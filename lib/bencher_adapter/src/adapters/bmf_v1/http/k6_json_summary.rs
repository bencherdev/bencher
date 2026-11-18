use std::collections::BTreeMap;

use bencher_json::project::report::Adapter;
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        finite::Finite,
        http::k6::{
            CHECK, Check, Contains, DROPPED_ITERATIONS, GROUP, Kind, Metric, Values, insert_check,
            insert_no_drops,
        },
        leaf::LeafResults,
    },
    results::adapter_results::AdapterResults,
};

/// k6's end-of-test summary data as JSON: the legacy export that `--summary-export` writes, or
/// the `data` a `handleSummary` callback writes, which also gives each metric's type and what its
/// values hold.
pub struct AdapterHttpK6JsonSummary;

impl Adaptable for AdapterHttpK6JsonSummary {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let mut results = LeafResults::default();
        let dropped = match serde_json::from_str(input).ok()? {
            Summary::Data(Data {
                root_group,
                metrics,
            }) => {
                root_group.insert(&mut results);
                let dropped = metrics.contains_key(DROPPED_ITERATIONS);
                for (name, metric) in &metrics {
                    metric.insert(name, &mut results);
                }
                dropped
            },
            Summary::Export(Export {
                root_group,
                metrics,
            }) => {
                root_group.insert(&mut results);
                let dropped = metrics.contains_key(DROPPED_ITERATIONS);
                for (name, values) in metrics {
                    Metric {
                        name: &name,
                        contains: None,
                        values: export_values(&values),
                    }
                    .insert(&mut results);
                }
                dropped
            },
        };
        if !dropped {
            insert_no_drops(&mut results);
        }
        results.finish(Adapter::HttpK6JsonSummary)
    }
}

/// The summary data, whose groups and checks are arrays, or the legacy export, whose groups and
/// checks are objects keyed by name.
#[derive(Deserialize)]
#[serde(untagged)]
enum Summary {
    Data(Data),
    Export(Export),
}

#[derive(Deserialize)]
struct Data {
    root_group: DataGroup,
    metrics: BTreeMap<String, DataMetric>,
}

#[derive(Deserialize)]
struct DataGroup {
    path: String,
    groups: Vec<Self>,
    checks: Vec<Check>,
}

impl DataGroup {
    fn insert(&self, results: &mut LeafResults) {
        for check in &self.checks {
            insert_group_check(results, check, &self.path);
        }
        for group in &self.groups {
            group.insert(results);
        }
    }
}

#[derive(Deserialize)]
struct DataMetric {
    #[serde(rename = "type")]
    kind: Kind,
    contains: Contains,
    values: BTreeMap<String, Finite>,
}

impl DataMetric {
    fn insert(&self, name: &str, results: &mut LeafResults) {
        let value = |key: &str| self.values.get(key).copied().unwrap_or_default();
        let values = match self.kind {
            Kind::Trend => Values::Trend(
                self.values
                    .iter()
                    .map(|(column, value)| (column.clone(), *value))
                    .collect(),
            ),
            Kind::Counter => Values::Counter {
                count: value("count"),
                rate: Some(value("rate")),
            },
            Kind::Rate => Values::Rate {
                rate: value("rate"),
                passes: value("passes"),
                samples: samples(value("passes"), value("fails")),
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

#[derive(Deserialize)]
struct Export {
    root_group: ExportGroup,
    metrics: BTreeMap<String, Map<String, Value>>,
}

#[derive(Deserialize)]
struct ExportGroup {
    path: String,
    groups: BTreeMap<String, Self>,
    checks: BTreeMap<String, Check>,
}

impl ExportGroup {
    fn insert(&self, results: &mut LeafResults) {
        for check in self.checks.values() {
            insert_group_check(results, check, &self.path);
        }
        for group in self.groups.values() {
            group.insert(results);
        }
    }
}

/// A metric's values in the legacy export, which names no type, so the type is read from the
/// keys k6 writes for each (`internal/js/summary-wrapper.js`): a Counter's `rate`, a Rate's
/// `passes`, a Gauge's `value`, and otherwise a Trend's columns. Its thresholds are not read.
fn export_values(values: &Map<String, Value>) -> Values {
    let value = |key: &str| Finite::from(values.get(key).and_then(Value::as_f64));
    if values.contains_key("rate") {
        Values::Counter {
            count: value("count"),
            rate: Some(value("rate")),
        }
    } else if values.contains_key("passes") {
        Values::Rate {
            rate: value("value"),
            passes: value("passes"),
            samples: samples(value("passes"), value("fails")),
        }
    } else if values.contains_key("value") {
        Values::Gauge {
            value: value("value"),
            min: value("min"),
            max: value("max"),
        }
    } else {
        Values::Trend(
            values
                .iter()
                .filter(|(column, _)| *column != "thresholds")
                .map(|(column, value)| (column.clone(), Finite::from(value.as_f64())))
                .collect(),
        )
    }
}

/// A Rate's sample count, its passes and its fails.
fn samples(passes: Finite, fails: Finite) -> Finite {
    Finite::from(
        passes
            .get()
            .zip(fails.get())
            .map(|(passes, fails)| passes + fails),
    )
}

/// A check's passes and failures by its name and its group's path.
fn insert_group_check(results: &mut LeafResults, check: &Check, group: &str) {
    let keys = Map::from_iter([
        (CHECK.to_owned(), check.name.clone().into()),
        (GROUP.to_owned(), group.into()),
    ]);
    insert_check(results, keys, check.passes, check.fails);
}

#[cfg(test)]
mod test_http_k6_json_summary {
    use bencher_json::{
        BmfVersion, ParameterSet,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{BytesThroughput, Latency, Throughput},
                http::{
                    BlockedTime, CheckFailures, CheckPasses, CheckRatio, ConnectingTime,
                    DroppedIterations, FailedRequests, FailureRatio, IterationThroughput,
                    IterationTime, Iterations, ReceivingTime, Requests, SendingTime,
                    TlsHandshakingTime, WaitingTime,
                },
            },
            report::Adapter,
        },
    };
    use bencher_valid::{NANOSECONDS, PERCENTAGE};
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::test_util::{
            Expected, assert_ignores_average, assert_units, assert_value, assert_variant,
            convert_edited_v1, convert_v1, value, variants,
        },
        results::adapter_results::{AdapterResults, Skipped},
    };

    const LEAF: Adapter = Adapter::HttpK6JsonSummary;

    /// The fixture through the leaf, with its one benchmark named as the server names it when
    /// the run sets no prefix.
    fn named(suffix: &str) -> AdapterResults {
        let mut results = convert_v1(LEAF, &format!("http/k6/{suffix}"));
        results.name_nameless(None);
        results
    }

    fn named_edited<F: FnOnce(&mut serde_json::Value)>(suffix: &str, edit: F) -> AdapterResults {
        let mut results = convert_edited_v1(LEAF, &format!("http/k6/{suffix}"), edit);
        results.name_nameless(None);
        results
    }

    fn trend(measure: &'static str, [mean, median, min, max, p90, p95]: [f64; 6]) -> Expected {
        (
            measure,
            vec![
                ("mean", mean),
                ("median", median),
                ("min", min),
                ("max", max),
                ("p90", p90),
                ("p95", p95),
            ],
        )
    }

    /// Kills a leaf that names the benchmark itself, which a prefix would then go in front of,
    /// one that records another leaf, and a skip counted for the virtual users, a Trend's sample
    /// count, or a byte total.
    #[test]
    fn adapter_http_k6_json_summary_is_one_nameless_benchmark() {
        for suffix in ["export_simple.json", "data_simple.json"] {
            let results = convert_v1(LEAF, &format!("http/k6/{suffix}"));
            assert_eq!(results.resolved_adapter, Some(LEAF), "{suffix}");
            assert!(results.inner.is_empty(), "{suffix}");
            assert_eq!(
                results.nameless.as_ref().map(|nameless| nameless.harness),
                Some("k6"),
                "{suffix}"
            );
            assert_eq!(results.skipped, Skipped::default(), "{suffix}");
        }
    }

    /// Kills a time left in milliseconds, `avg` or `med` misnamed, a sample count, a byte total,
    /// or the virtual users written, a rate written as anything but `mean`, the failure ratio
    /// left a fraction or read from the successes, the failed count read from `fails`, and no 0
    /// for the dropped iterations k6 writes only once one drops.
    #[test]
    fn adapter_http_k6_json_summary_export_measures() {
        let results = named("export_simple.json");
        assert_eq!(variants(&results), 5);
        assert_variant(
            &results,
            "k6",
            "{}",
            &[
                trend(
                    Latency::SLUG_STR,
                    [
                        156_900.720_459_952_4,
                        85_963.0,
                        17_878.0,
                        7_269_975.0,
                        325_733.000_000_000_1,
                        478_958.899_999_999_8,
                    ],
                ),
                trend(
                    BlockedTime::SLUG_STR,
                    [
                        2_113.435_714_904_278,
                        1_252.0,
                        320.0,
                        3_645_869.0,
                        2_514.0,
                        3_406.0,
                    ],
                ),
                trend(
                    ConnectingTime::SLUG_STR,
                    [13.131_861_469_361_832, 0.0, 0.0, 1_764_726.0, 0.0, 0.0],
                ),
                trend(TlsHandshakingTime::SLUG_STR, [0.0; 6]),
                trend(
                    SendingTime::SLUG_STR,
                    [
                        5_769.478_971_702_261,
                        3_816.0,
                        911.0,
                        2_453_605.0,
                        6_930.0,
                        8_585.699_999_999_837,
                    ],
                ),
                trend(
                    WaitingTime::SLUG_STR,
                    [
                        131_932.995_850_846_14,
                        67_104.0,
                        10_156.0,
                        6_653_788.0,
                        285_250.200_000_000_1,
                        424_497.999_999_998_8,
                    ],
                ),
                trend(
                    ReceivingTime::SLUG_STR,
                    [
                        19_198.245_637_406_547,
                        12_369.0,
                        2_563.0,
                        7_088_446.0,
                        25_589.0,
                        31_589.899_999_999_79,
                    ],
                ),
                trend(
                    IterationTime::SLUG_STR,
                    [
                        199_586.359_083_391_56,
                        120_305.0,
                        34_392.0,
                        9_161_066.0,
                        389_048.0,
                        567_306.999_999_998_8,
                    ],
                ),
                (Requests::SLUG_STR, vec![("value", 242_459.0)]),
                (Throughput::SLUG_STR, vec![("mean", 48_490.567_903_160_15)]),
                (FailureRatio::SLUG_STR, vec![("value", 0.0)]),
                (FailedRequests::SLUG_STR, vec![("value", 0.0)]),
                (Iterations::SLUG_STR, vec![("value", 242_459.0)]),
                (
                    IterationThroughput::SLUG_STR,
                    vec![("mean", 48_490.567_903_160_15)],
                ),
                (CheckRatio::SLUG_STR, vec![("value", 100.0)]),
                (DroppedIterations::SLUG_STR, vec![("value", 0.0)]),
            ],
        );
    }

    /// Kills a byte rate without its direction or put in another measure, the total byte count
    /// written, `expected_response` written as k6 spells it, and a check's passes and failures
    /// swapped or put on the check's own path.
    #[test]
    fn adapter_http_k6_json_summary_export_variants() {
        let results = named("export_simple.json");
        for (direction, rate) in [
            ("received", 5_770_377.580_476_058),
            ("sent", 3_588_302.024_833_851),
        ] {
            assert_variant(
                &results,
                "k6",
                &format!(r#"{{"@direction":"{direction}"}}"#),
                &[(BytesThroughput::SLUG_STR, vec![("mean", rate)])],
            );
        }
        assert_value(
            &results,
            ("k6", r#"{"@responses":"expected"}"#),
            (Latency::SLUG_STR, "p95"),
            478_958.899_999_999_8,
        );
        assert_variant(
            &results,
            "k6",
            r#"{"@check":"status is 200","@group":""}"#,
            &[
                (CheckPasses::SLUG_STR, vec![("value", 242_459.0)]),
                (CheckFailures::SLUG_STR, vec![("value", 0.0)]),
            ],
        );
    }

    /// Kills a status code on `http_reqs` kept as a variant of the count, a count by status
    /// written to `requests`, a code IANA does not assign dropped or given a built-in, its rate
    /// left out of `throughput`, `expected_response` or a system tag written as k6 spells it, a
    /// sub-metric of the bytes without its direction, and the 0 for no dropped iterations written
    /// over the count k6 gives.
    #[test]
    fn adapter_http_k6_json_summary_sub_metrics() {
        let results = named("export_summary.json");
        for (parameters, measure, metric, expected) in [
            (
                r#"{"@scenario":"browse"}"#,
                Latency::SLUG_STR,
                "median",
                91_537_996.499_999_99,
            ),
            (
                r#"{"@group":"::checkout::pay"}"#,
                Latency::SLUG_STR,
                "mean",
                42_143_024.351_351_34,
            ),
            (
                r#"{"@responses":"500"}"#,
                Latency::SLUG_STR,
                "max",
                67_397_459.0,
            ),
            ("{}", "responses-200", "value", 99.0),
            ("{}", "responses-599", "value", 37.0),
            (
                r#"{"@responses":"200"}"#,
                Throughput::SLUG_STR,
                "mean",
                31.610_346_263_146_244,
            ),
            (
                r#"{"@responses":"599"}"#,
                Throughput::SLUG_STR,
                "mean",
                11.813_967_795_317_284,
            ),
            (
                r#"{"@responses":"unexpected"}"#,
                Requests::SLUG_STR,
                "value",
                74.0,
            ),
            (
                r#"{"@responses":"unexpected"}"#,
                Throughput::SLUG_STR,
                "mean",
                23.627_935_590_634_568,
            ),
            (
                r#"{"@name":"fast"}"#,
                FailedRequests::SLUG_STR,
                "value",
                0.0,
            ),
            (
                r#"{"@check":"home is 200"}"#,
                CheckRatio::SLUG_STR,
                "value",
                100.0,
            ),
            (
                r#"{"@direction":"received","@scenario":"browse"}"#,
                BytesThroughput::SLUG_STR,
                "mean",
                326_620.448_340_71,
            ),
            ("{}", DroppedIterations::SLUG_STR, "value", 239.0),
        ] {
            assert_value(&results, ("k6", parameters), (measure, metric), expected);
        }
        for (parameters, measure) in [
            (r#"{"@responses":"200"}"#, "responses-200"),
            (r#"{"@responses":"200"}"#, Requests::SLUG_STR),
            ("{}", "responses-500"),
        ] {
            assert_eq!(
                value(&results, "k6", parameters, measure, "value"),
                None,
                "{parameters} {measure}"
            );
        }
        assert_units(
            &convert_v1(LEAF, "http/k6/export_summary.json"),
            &[("responses-599", "responses"), ("paid", PERCENTAGE)],
        );
    }

    /// Kills a tag split at its last colon, a sub-metric of several tags read as one, a checks
    /// sub-metric not read as `check-ratio`, and a decimal percentile misnamed.
    #[test]
    fn adapter_http_k6_json_summary_tags() {
        let results = named("export_tags.json");
        for (parameters, measure, metric, expected) in [
            (
                r#"{"@method":"GET","@responses":"200"}"#,
                Latency::SLUG_STR,
                "median",
                25_534_907.0,
            ),
            (
                r#"{"@url":"http://localhost:8080/slow"}"#,
                Latency::SLUG_STR,
                "p99_99",
                51_136_208.533,
            ),
            (
                r#"{"@check":"fast is 200"}"#,
                CheckRatio::SLUG_STR,
                "value",
                100.0,
            ),
        ] {
            assert_value(&results, ("k6", parameters), (measure, metric), expected);
        }
    }

    /// Kills a group's path read from the check's path, a nested group or the root group's
    /// checks left unread, and the export's objects or the summary data's arrays of groups and
    /// checks refused.
    #[test]
    fn adapter_http_k6_json_summary_checks_by_group() {
        for (suffix, [home, checkout]) in [
            ("export_scenarios.json", [1_001.0, 490.0]),
            ("data_summary.json", [62.0, 37.0]),
        ] {
            let results = named(suffix);
            for (check, group, passes) in [
                ("home is 200", "::home", home),
                ("cart is 200", "::checkout::cart", checkout),
                ("pay is 201", "::checkout::pay", checkout),
                ("error is 500", "", checkout),
            ] {
                let parameters = format!(r#"{{"@check":"{check}","@group":"{group}"}}"#);
                assert_value(
                    &results,
                    ("k6", &parameters),
                    (CheckPasses::SLUG_STR, "value"),
                    passes,
                );
                assert_value(
                    &results,
                    ("k6", &parameters),
                    (CheckFailures::SLUG_STR, "value"),
                    0.0,
                );
            }
        }
    }

    /// Kills a user's Trend, Gauge, or time-valued metric read without the unit the summary data
    /// gives, a Trend that holds no times scaled as one, a Counter's count, a Gauge, or a Rate
    /// dropped, and a Rate left a fraction.
    #[test]
    fn adapter_http_k6_json_summary_data_reads_custom_metrics() {
        let results = named("data_summary.json");
        assert_eq!(results.skipped, Skipped::default());
        for (measure, metric, expected) in [
            ("order_time", "mean", 42_143_024.351_351_34),
            ("order_time", "p99_9", 59_705_956.420_000_03),
            ("queue_depth", "median", 5.0),
            ("orders_placed", "value", 37.0),
            ("cart_size", "value", 3.0),
            ("cart_size", "max", 3.0),
            ("paid", "value", 100.0),
        ] {
            assert_value(&results, ("k6", "{}"), (measure, metric), expected);
        }
        assert_value(
            &results,
            ("k6", r#"{"flow":"checkout"}"#),
            ("order_time", "max"),
            59_928_298.0,
        );
        assert_units(
            &convert_v1(LEAF, "http/k6/data_summary.json"),
            &[
                ("order_time", NANOSECONDS),
                ("paid", PERCENTAGE),
                ("responses-599", "responses"),
            ],
        );
    }

    /// Kills a user's Counter of bytes written as its total or with its rate dropped, and a
    /// user's Counter of times written as its sum or counted as skipped.
    #[test]
    fn adapter_http_k6_json_summary_data_reads_custom_byte_rates() {
        let mut results = convert_edited_v1(LEAF, "http/k6/data_summary.json", |json| {
            json["metrics"]["bytes_out"] = serde_json::json!({
                "type": "counter",
                "contains": "data",
                "values": { "count": 2048, "rate": 512.5 },
            });
            json["metrics"]["busy_time"] = serde_json::json!({
                "type": "counter",
                "contains": "time",
                "values": { "count": 5, "rate": 1 },
            });
        });
        assert_eq!(
            results.units[&"bytes_out".parse().unwrap()].as_ref(),
            BytesThroughput::UNITS_STR
        );
        results.name_nameless(None);
        assert_eq!(results.skipped, Skipped::default());
        assert_variant_has(&results, "bytes_out", &[("mean", 512.5)]);
        assert_eq!(value(&results, "k6", "{}", "busy_time", "value"), None);
        assert_eq!(value(&results, "k6", "{}", "busy_time", "mean"), None);
    }

    /// The `{}` variant's metrics of `measure` are exactly `expected`.
    fn assert_variant_has(results: &AdapterResults, measure: &str, expected: &[(&str, f64)]) {
        let metrics = results
            .get("k6")
            .and_then(|metrics| metrics.inner.get(&measure.parse().unwrap()))
            .unwrap_or_else(|| panic!("no {measure}"));
        assert_eq!(
            metrics
                .inner
                .keys()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            expected
                .iter()
                .map(|(metric, _)| (*metric).to_owned())
                .collect::<Vec<_>>(),
            "{measure}"
        );
        for (metric, expected) in expected {
            assert_value(results, ("k6", "{}"), (measure, metric), *expected);
        }
    }

    /// Kills a user's Trend or Gauge in the legacy export read as if it held milliseconds, or
    /// skipped uncounted, and its Counters and Rates, which hold no times, dropped.
    #[test]
    fn adapter_http_k6_json_summary_export_skips_custom_trends_and_gauges() {
        let results = named("export_summary.json");
        assert_eq!(
            results.skipped,
            Skipped {
                measures: 4,
                ..Skipped::default()
            }
        );
        for measure in ["order_time", "queue_depth", "cart_size"] {
            assert_eq!(
                value(&results, "k6", "{}", measure, "max"),
                None,
                "{measure}"
            );
        }
        assert_value(&results, ("k6", "{}"), ("orders_placed", "value"), 37.0);
        assert_value(&results, ("k6", "{}"), ("paid", "value"), 100.0);
    }

    /// Kills a sub-metric with no samples written as measured zeros (a Trend whose `count` is 0,
    /// a Rate's ratio with no passes and no fails, a user's Trend or Rate, a check's ratio), the
    /// failed count or the count by status dropped with them, the skip counted, an all-zero Trend
    /// with samples skipped, and a ratio of 0 from real samples skipped.
    #[test]
    fn adapter_http_k6_json_summary_skips_empty_sub_metrics() {
        for (suffix, measures) in [("export_sub_metrics.json", 1), ("data_sub_metrics.json", 0)] {
            let results = named(suffix);
            assert_eq!(
                results.skipped,
                Skipped {
                    variants: 2,
                    measures,
                    ..Skipped::default()
                },
                "{suffix}"
            );
            assert_variant(
                &results,
                "k6",
                r#"{"@responses":"500"}"#,
                &[
                    (Throughput::SLUG_STR, vec![("mean", 0.0)]),
                    (FailedRequests::SLUG_STR, vec![("value", 0.0)]),
                ],
            );
            let check = r#"{"@check":"never runs"}"#.parse::<ParameterSet>().unwrap();
            assert!(
                results
                    .inner
                    .values()
                    .all(|variants| !variants.contains_key(&check)),
                "{suffix}"
            );
            for (measure, metric) in [
                ("responses-500", "value"),
                (TlsHandshakingTime::SLUG_STR, "max"),
                (FailureRatio::SLUG_STR, "value"),
            ] {
                assert_eq!(
                    value(&results, "k6", "{}", measure, metric),
                    Some(0.0),
                    "{suffix} {measure}"
                );
            }
        }
    }

    /// Kills thresholds, `options`, or `state` read: each is junk here, and the results stay the
    /// same.
    #[test]
    fn adapter_http_k6_json_summary_ignores_thresholds_and_settings() {
        let results = named("export_summary.json");
        let edited = named_edited("export_summary.json", |json| {
            json["metrics"]["http_req_duration"]["thresholds"] = serde_json::json!("crossed");
            json["metrics"]["checks"]["thresholds"] = serde_json::json!([1]);
        });
        assert_eq!(edited, results);
        let results = named("data_summary.json");
        let edited = named_edited("data_summary.json", |json| {
            json["metrics"]["http_req_duration"]["thresholds"] = serde_json::json!(7);
            json["options"] = serde_json::json!("junk");
            json["state"] = serde_json::json!([]);
        });
        assert_eq!(edited, results);
    }

    /// Kills a ratio or a time scaled in floating point, which gives `28.999999999999996` for a
    /// fraction of `0.29` and `497.00000000000006` for `0.000497` ms.
    #[test]
    fn adapter_http_k6_json_summary_scales_in_decimal() {
        let results = named_edited("export_simple.json", |json| {
            json["metrics"]["http_req_failed"]["value"] = serde_json::json!(0.29);
            json["metrics"]["http_req_duration"]["min"] = serde_json::json!(0.000_497);
        });
        for (measure, metric, expected) in [
            (FailureRatio::SLUG_STR, "value", 29.0),
            (Latency::SLUG_STR, "min", 497.0),
        ] {
            let actual = value(&results, "k6", "{}", measure, metric).expect("no metric");
            assert_eq!(
                actual.to_bits(),
                f64::to_bits(expected),
                "{measure} {metric}"
            );
        }
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_http_k6_json_summary_ignores_average() {
        for suffix in ["export_summary.json", "data_summary.json"] {
            assert_ignores_average(LEAF, &format!("http/k6/{suffix}"));
        }
    }

    /// Kills a node that tries only one of its leaves, or tries the summary export before the
    /// machine-readable summary, which a document both leaves read tells apart.
    #[test]
    fn adapter_http_k6_node_reads_both_summaries_json_first() {
        let read = |suffix: &str| {
            std::fs::read_to_string(format!("./tool_output/http/k6/{suffix}"))
                .expect("Failed to read a fixture")
        };
        let mut both =
            serde_json::from_str::<serde_json::Value>(&read("machine_readable_simple.json"))
                .unwrap();
        let export =
            serde_json::from_str::<serde_json::Value>(&read("export_simple.json")).unwrap();
        for key in ["root_group", "metrics"] {
            both[key] = export[key].clone();
        }
        let settings = Settings::new(None, BmfVersion::V1);
        assert!(LEAF.convert(&both.to_string(), settings).is_some());
        for (input, leaf) in [
            (read("export_simple.json"), Adapter::HttpK6JsonSummary),
            (read("data_simple.json"), Adapter::HttpK6JsonSummary),
            (read("machine_readable_simple.json"), Adapter::HttpK6Json),
            (both.to_string(), Adapter::HttpK6Json),
        ] {
            let results = Adapter::HttpK6
                .convert(&input, settings)
                .expect("expected http_k6 to parse");
            assert_eq!(results.resolved_adapter, Some(leaf));
        }
    }
}
