use std::collections::BTreeMap;

use bencher_json::project::report::Adapter;
use serde::Deserialize;
use serde_json::Map;

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        finite::Finite,
        http::k6::{
            CHECK, Check, Contains, DROPPED_ITERATIONS, Kind, Metric, Values, insert_check,
            insert_no_drops,
        },
        leaf::LeafResults,
    },
    results::adapter_results::AdapterResults,
};

/// k6's machine-readable summary, schema version 1.0.0, from `--new-machine-readable-summary`.
///
/// It keeps only six Trend statistics and no Counter rate, and its checks are counted by name
/// across every group. Its `config` holds the script's source and is never read.
pub struct AdapterHttpK6Json;

impl Adaptable for AdapterHttpK6Json {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let MachineReadable { version, results } = serde_json::from_str(input).ok()?;
        if version != SCHEMA_VERSION {
            return None;
        }
        let Results { checks, metrics } = results;
        let mut leaf = LeafResults::default();
        for metric in &checks.metrics {
            if metric.name == CHECKS_SUCCEEDED {
                metric.insert("checks", &mut leaf);
            }
        }
        for check in checks.results {
            let keys = Map::from_iter([(CHECK.to_owned(), check.name.into())]);
            insert_check(&mut leaf, keys, check.passes, check.fails);
        }
        for metric in &metrics {
            metric.insert(&metric.name, &mut leaf);
        }
        if !metrics
            .iter()
            .any(|metric| metric.name == DROPPED_ITERATIONS)
        {
            insert_no_drops(&mut leaf);
        }
        leaf.finish(Adapter::HttpK6Json)
    }
}

/// The schema this leaf reads (`internal/lib/summary/machinereadable/schema_version_const.go`).
const SCHEMA_VERSION: &str = "1.0.0";
/// The share of checks that passed, which the other summaries name `checks`.
const CHECKS_SUCCEEDED: &str = "checks_succeeded";

#[derive(Deserialize)]
struct MachineReadable {
    version: String,
    results: Results,
}

#[derive(Deserialize)]
struct Results {
    #[serde(default)]
    checks: Checks,
    metrics: Vec<MachineMetric>,
}

#[derive(Default, Deserialize)]
struct Checks {
    #[serde(default)]
    metrics: Vec<MachineMetric>,
    #[serde(default)]
    results: Vec<Check>,
}

#[derive(Deserialize)]
struct MachineMetric {
    name: String,
    #[serde(rename = "type")]
    kind: Kind,
    contains: Contains,
    values: BTreeMap<String, Finite>,
}

impl MachineMetric {
    /// The metric under `name`, with its Trend columns spelled as the other summaries spell them
    /// (`p90` as `p(90)`), and a Rate's `matches` as its passes.
    fn insert(&self, name: &str, results: &mut LeafResults) {
        let value = |key: &str| self.values.get(key).copied().unwrap_or_default();
        let values = match self.kind {
            Kind::Trend => Values::Trend(
                self.values
                    .iter()
                    .map(|(column, value)| {
                        let column = column
                            .strip_prefix('p')
                            .map_or_else(|| column.clone(), |percent| format!("p({percent})"));
                        (column, *value)
                    })
                    .collect(),
            ),
            Kind::Counter => Values::Counter {
                count: value("count"),
                rate: None,
            },
            Kind::Rate => Values::Rate {
                rate: value("rate"),
                passes: value("matches"),
                samples: value("total"),
            },
            Kind::Gauge => Values::Gauge {
                value: value("value"),
                min: value("min"),
                max: value("max"),
            },
        };
        Metric {
            name,
            contains: Some(self.contains),
            values,
        }
        .insert(results);
    }
}

#[cfg(test)]
mod test_http_k6_json {
    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{BytesThroughput, Latency, Throughput},
                http::{
                    BlockedTime, CheckFailures, CheckPasses, CheckRatio, DroppedIterations,
                    FailedRequests, FailureRatio, IterationThroughput, Iterations, Requests,
                },
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::test_util::{
            assert_ignores_average, assert_value, assert_variant, convert_edited_v1, convert_v1,
            value,
        },
        results::adapter_results::{AdapterResults, Skipped},
    };

    const LEAF: Adapter = Adapter::HttpK6Json;

    fn named(suffix: &str) -> AdapterResults {
        let mut results = convert_v1(LEAF, &format!("http/k6/{suffix}"));
        results.name_nameless(None);
        results
    }

    /// Kills the schema's `p90` and `p95` misread, a rate the summary does not hold made up for a
    /// Counter, the share of checks read from `checks_failed`, a check given a group the summary
    /// does not hold, and a Rate's `matches` not read as its failed count.
    #[test]
    fn adapter_http_k6_json_measures() {
        let results = named("machine_readable_summary.json");
        assert_eq!(results.resolved_adapter, Some(LEAF));
        assert_eq!(results.skipped, Skipped::default());
        for (measure, metric, expected) in [
            (Latency::SLUG_STR, "mean", 54_874_152.841_121_5),
            (Latency::SLUG_STR, "p90", 91_383_178.100_000_01),
            (Latency::SLUG_STR, "p95", 91_619_415.75),
            (Requests::SLUG_STR, "value", 214.0),
            (FailureRatio::SLUG_STR, "value", 35.514_018_691_588_78),
            (FailedRequests::SLUG_STR, "value", 76.0),
            (Iterations::SLUG_STR, "value", 100.0),
            (CheckRatio::SLUG_STR, "value", 100.0),
            (DroppedIterations::SLUG_STR, "value", 239.0),
            ("responses-599", "value", 38.0),
            ("order_time", "max", 41_935_180.0),
        ] {
            assert_value(&results, ("k6", "{}"), (measure, metric), expected);
        }
        for (parameters, measure, metric) in [
            ("{}", Throughput::SLUG_STR, "mean"),
            ("{}", IterationThroughput::SLUG_STR, "mean"),
            (
                r#"{"@direction":"received"}"#,
                BytesThroughput::SLUG_STR,
                "mean",
            ),
            (
                r#"{"@check":"cart is 200","@group":"::checkout::cart"}"#,
                CheckPasses::SLUG_STR,
                "value",
            ),
        ] {
            assert_eq!(
                value(&results, "k6", parameters, measure, metric),
                None,
                "{parameters} {measure}"
            );
        }
        assert_variant(
            &results,
            "k6",
            r#"{"@check":"cart is 200"}"#,
            &[
                (CheckPasses::SLUG_STR, vec![("value", 38.0)]),
                (CheckFailures::SLUG_STR, vec![("value", 0.0)]),
            ],
        );
        let full = named("machine_readable_scenarios_full.json");
        assert_value(&full, ("k6", "{}"), (BlockedTime::SLUG_STR, "p95"), 3_726.0);
    }

    /// Kills a leaf that reads a schema version other than 1.0.0, whose shape it cannot know.
    #[test]
    fn adapter_http_k6_json_refuses_another_schema_version() {
        let input = std::fs::read_to_string("./tool_output/http/k6/machine_readable_simple.json")
            .expect("Failed to read a fixture");
        let mut json = serde_json::from_str::<serde_json::Value>(&input).unwrap();
        let settings = Settings::new(None, BmfVersion::V1);
        assert!(LEAF.convert(&input, settings).is_some());
        json["version"] = serde_json::json!("1.1.0");
        assert_eq!(LEAF.convert(&json.to_string(), settings), None);
    }

    /// Kills a claim that needs `metadata.k6_version`, which k6 may rename to `k6Version`: the
    /// summary with camelCase metadata, or with none, reads the same.
    #[test]
    fn adapter_http_k6_json_needs_no_metadata() {
        let results = convert_v1(LEAF, "http/k6/machine_readable_simple.json");
        for metadata in [
            Some(
                serde_json::json!({ "k6Version": "2.4.0", "generatedAt": "2026-01-01T00:00:00Z" }),
            ),
            None,
        ] {
            let edited = convert_edited_v1(LEAF, "http/k6/machine_readable_simple.json", |json| {
                let object = json.as_object_mut().unwrap();
                if let Some(metadata) = metadata.clone() {
                    object.insert("metadata".to_owned(), metadata);
                } else {
                    object.remove("metadata");
                }
            });
            assert_eq!(edited, results, "{metadata:?}");
        }
    }

    /// Kills a Rate's ratio with no samples (`total` 0) written as a measured 0, a user's Rate
    /// with none written, the failed count or the count by status dropped with them, and a ratio
    /// of 0 from real samples skipped.
    #[test]
    fn adapter_http_k6_json_skips_empty_rates() {
        let results = named("machine_readable_sub_metrics.json");
        assert_eq!(
            results.skipped,
            Skipped {
                variants: 2,
                ..Skipped::default()
            }
        );
        let status = r#"{"@responses":"500"}"#;
        assert_value(
            &results,
            ("k6", status),
            (FailedRequests::SLUG_STR, "value"),
            0.0,
        );
        for measure in [FailureRatio::SLUG_STR, "paid"] {
            assert_eq!(
                value(&results, "k6", status, measure, "value"),
                None,
                "{measure}"
            );
        }
        for measure in ["responses-500", FailureRatio::SLUG_STR] {
            assert_eq!(
                value(&results, "k6", "{}", measure, "value"),
                Some(0.0),
                "{measure}"
            );
        }
    }

    /// Kills a leaf that reads `config`, which holds the script's source: junk there, or none,
    /// leaves the results the same.
    #[test]
    fn adapter_http_k6_json_never_reads_config() {
        let results = convert_v1(LEAF, "http/k6/machine_readable_summary.json");
        for config in [
            Some(serde_json::json!({ "script": 1, "duration": "x", "execution": [] })),
            Some(serde_json::Value::Null),
            None,
        ] {
            let edited = convert_edited_v1(LEAF, "http/k6/machine_readable_summary.json", |json| {
                let object = json.as_object_mut().unwrap();
                if let Some(config) = config.clone() {
                    object.insert("config".to_owned(), config);
                } else {
                    object.remove("config");
                }
            });
            assert_eq!(edited, results, "{config:?}");
        }
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_http_k6_json_ignores_average() {
        assert_ignores_average(LEAF, "http/k6/machine_readable_summary.json");
    }
}
