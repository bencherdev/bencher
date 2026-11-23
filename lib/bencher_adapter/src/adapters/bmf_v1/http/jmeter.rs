use std::collections::BTreeMap;

use bencher_json::{
    BenchmarkName, MeasureNameId,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{BytesThroughput, Latency, Throughput},
            http::{FailedRequests, FailureRatio, Requests},
        },
        report::Adapter,
    },
};
use rust_decimal::Decimal;
use serde::Deserialize;
use serde_json::Map;

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        finite::Finite,
        leaf::{LeafResults, Variant, parameter_set},
    },
    results::adapter_results::AdapterResults,
};

/// The HTML dashboard's `statistics.json` at version 1.
///
/// Each label is a benchmark, Transaction Controllers included, and the whole test's `Total`
/// entry is the benchmark the server names by the run's benchmark prefix, else `jmeter`.
pub struct AdapterHttpJmeter;

impl Adaptable for AdapterHttpJmeter {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let statistics = serde_json::from_str::<BTreeMap<String, Statistic>>(input).ok()?;
        let mut results = LeafResults::default();
        for (label, statistic) in statistics {
            statistic.insert(&label, &mut results);
        }
        results.finish_with_skips(Adapter::HttpJmeter)
    }
}

/// One label's statistics (`JsonExporter.java:123-142` and `StatisticsSummaryConsumer.java:135-157`
/// at 5.6.3), each time in whole milliseconds underneath and each rate over the label's span from
/// its first start to its last end. The dashboard writes a value that is not finite as a string.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Statistic {
    sample_count: Finite,
    error_count: Finite,
    error_pct: Finite,
    mean_res_time: Finite,
    /// Written since 5.3.
    median_res_time: Option<Finite>,
    min_res_time: Finite,
    max_res_time: Finite,
    /// The percentiles at `aggregate_rpt_pct1` to `3`, which the file does not record, read at
    /// the defaults of 90, 95, and 99.
    pct1_res_time: Finite,
    pct2_res_time: Finite,
    pct3_res_time: Finite,
    throughput: Finite,
    /// In KiB per second.
    received_k_bytes_per_sec: Finite,
    sent_k_bytes_per_sec: Finite,
}

/// The whole test's key in English, `reportgenerator_summary_total`.
const TOTAL: &str = "Total";
/// The name of the whole test when the run sets no prefix.
const JMETER: &str = "jmeter";
const VALUE: &str = "value";
const MEAN: &str = "mean";
const DIRECTION: &str = "@direction";
const NANOS_PER_MILLI: u32 = 1_000_000;
const BYTES_PER_KIB: u32 = 1_024;

impl Statistic {
    fn insert(self, label: &str, results: &mut LeafResults) {
        let benchmark = if label == TOTAL {
            None
        } else if let Ok(name) = label.parse::<BenchmarkName>() {
            Some(name)
        } else {
            results.skip_benchmark();
            return;
        };
        let Self {
            sample_count,
            error_count,
            error_pct,
            mean_res_time,
            median_res_time,
            min_res_time,
            max_res_time,
            pct1_res_time,
            pct2_res_time,
            pct3_res_time,
            throughput,
            received_k_bytes_per_sec,
            sent_k_bytes_per_sec,
        } = self;
        let Some(mut variant) = label_variant(results, benchmark.clone(), Map::new()) else {
            return;
        };
        sample_count.insert(&mut variant, &Requests::name_id(), VALUE, 1.0);
        error_count.insert(&mut variant, &FailedRequests::name_id(), VALUE, 1.0);
        // An empty Transaction Controller has no sample: JMeter writes `NaN` for its times and
        // ratio, and its unset `Long.MAX_VALUE` and `Long.MIN_VALUE` as its `min` and `max`.
        if sample_count.get() == Some(0.0) {
            return;
        }
        error_pct.insert(&mut variant, &FailureRatio::name_id(), VALUE, 1.0);
        let latency = Latency::name_id();
        for (metric, time) in [
            ("mean", Some(mean_res_time)),
            ("median", median_res_time),
            ("min", Some(min_res_time)),
            ("max", Some(max_res_time)),
            ("p90", Some(pct1_res_time)),
            ("p95", Some(pct2_res_time)),
            ("p99", Some(pct3_res_time)),
        ] {
            if let Some(time) = time {
                insert_scaled(&mut variant, &latency, metric, time, NANOS_PER_MILLI);
            }
        }
        // A label whose span is 0 ms has a rate that is not finite, a value of JMeter's 1 ms
        // resolution, so it is left out uncounted.
        if let Some(throughput) = throughput.get() {
            variant.insert(Throughput::name_id(), MEAN, throughput);
        }
        for (direction, rate) in [
            ("received", received_k_bytes_per_sec),
            ("sent", sent_k_bytes_per_sec),
        ] {
            let keys = Map::from_iter([(DIRECTION.to_owned(), direction.into())]);
            if let Some(rate) = rate.get()
                && let Some(mut variant) = label_variant(results, benchmark.clone(), keys)
            {
                variant.insert(BytesThroughput::name_id(), MEAN, scale(rate, BYTES_PER_KIB));
            }
        }
    }
}

/// The variant of `keys` of the label's benchmark, or of the whole test's for `None`, or `None`,
/// counted as skipped, past the key cap.
fn label_variant(
    results: &mut LeafResults,
    benchmark: Option<BenchmarkName>,
    keys: Map<String, serde_json::Value>,
) -> Option<Variant<'_>> {
    let Some(parameters) = parameter_set(keys) else {
        results.skip_variant();
        return None;
    };
    Some(if let Some(benchmark) = benchmark {
        results.variant(benchmark, parameters)
    } else {
        results.nameless_variant(JMETER, parameters)
    })
}

/// `value` times `times` as `metric`, counted as skipped when it is not finite.
fn insert_scaled(
    variant: &mut Variant<'_>,
    measure: &MeasureNameId,
    metric: &str,
    value: Finite,
    times: u32,
) {
    Finite::from(value.get().map(|value| scale(value, times)))
        .insert(variant, measure, metric, 1.0);
}

/// `value` times `times`, in decimal arithmetic on the shortest text that reads back as `value`,
/// so `0.024793388429752084` ms is `24793.388429752084` ns, else in floating point past the
/// decimal range.
fn scale(value: f64, times: u32) -> f64 {
    value
        .to_string()
        .parse::<Decimal>()
        .ok()
        .and_then(|decimal| decimal.checked_mul(Decimal::from(times)))
        .and_then(|decimal| decimal.to_string().parse().ok())
        .unwrap_or_else(|| value * f64::from(times))
}

#[cfg(test)]
mod test_http_jmeter {
    use bencher_json::{
        BenchmarkPrefix, BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{BytesThroughput, Latency, Throughput},
                http::{FailedRequests, FailureRatio, Requests},
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::test_util::{
            Expected, assert_ignores_average, assert_variant, convert_v1, value, variants,
        },
        results::adapter_results::{AdapterResults, Skipped},
    };

    const LEAF: Adapter = Adapter::HttpJmeter;
    const RECEIVED: &str = r#"{"@direction":"received"}"#;
    const SENT: &str = r#"{"@direction":"sent"}"#;

    fn read(suffix: &str) -> AdapterResults {
        convert_v1(LEAF, &format!("http/jmeter/{suffix}"))
    }

    fn named(suffix: &str, prefix: Option<&str>) -> AdapterResults {
        let mut results = read(suffix);
        let prefix = prefix.map(|prefix| prefix.parse::<BenchmarkPrefix>().unwrap());
        results.name_nameless(prefix.as_ref());
        results
    }

    fn fixture(suffix: &str) -> String {
        std::fs::read_to_string(format!("./tool_output/http/jmeter/{suffix}"))
            .expect("Failed to read a fixture")
    }

    fn count(measure: &'static str, value: f64) -> Expected {
        (measure, vec![("value", value)])
    }

    /// Kills a time left in milliseconds, `pct1` to `pct3` read as other percentiles or each
    /// other, `median` read from the mean, a rate in KiB/s, the two directions swapped or merged
    /// into one variant, `requests` or `failed-requests` read from the other count, the ratio
    /// written as a fraction, and a Transaction Controller left out.
    #[test]
    fn adapter_http_jmeter_measures() {
        let results = read("multi.statistics.json");
        assert_eq!(results.resolved_adapter, Some(LEAF));
        assert_eq!(results.version, BmfVersion::V1);
        assert_eq!(results.skipped, Skipped::default());
        assert_eq!(variants(&results), 8 * 3);
        assert_variant(
            &results,
            "slow",
            "{}",
            &[
                (
                    Latency::SLUG_STR,
                    vec![
                        ("mean", 50_152_030.217_186_05),
                        ("median", 50_000_000.0),
                        ("min", 50_000_000.0),
                        ("max", 52_000_000.0),
                        ("p90", 51_000_000.0),
                        ("p95", 51_000_000.0),
                        ("p99", 51_000_000.0),
                    ],
                ),
                (Throughput::SLUG_STR, vec![("mean", 106.059_088_632_949_43)]),
                count(Requests::SLUG_STR, 1_059.0),
                count(FailedRequests::SLUG_STR, 0.0),
                count(FailureRatio::SLUG_STR, 0.0),
            ],
        );
        assert_variant(
            &results,
            "slow",
            RECEIVED,
            &[(
                BytesThroughput::SLUG_STR,
                vec![("mean", 1_752_308.262_393_590_5)],
            )],
        );
        assert_variant(
            &results,
            "slow",
            SENT,
            &[(
                BytesThroughput::SLUG_STR,
                vec![("mean", 13_257.386_079_118_678)],
            )],
        );
        assert_variant(
            &results,
            "mixed",
            "{}",
            &[
                (
                    Latency::SLUG_STR,
                    vec![
                        ("mean", 41_861_480.075_901_33),
                        ("median", 0.0),
                        ("min", 0.0),
                        ("max", 1_002_000_000.0),
                        ("p90", 1_000_000.0),
                        ("p95", 1_000_000.0),
                        ("p99", 1_001_000_000.0),
                    ],
                ),
                (Throughput::SLUG_STR, vec![("mean", 99.565_463_820_139_81)]),
                count(Requests::SLUG_STR, 1_054.0),
                count(FailedRequests::SLUG_STR, 321.0),
                count(FailureRatio::SLUG_STR, 30.455_408),
            ],
        );
        for (metric, expected) in [("p90", 0.0), ("p95", 1_000_000.0), ("p99", 1_000_000.0)] {
            assert_eq!(
                value(&results, "drop", "{}", Latency::SLUG_STR, metric),
                Some(expected),
                "{metric}"
            );
        }
        assert_eq!(
            value(&results, "checkout", "{}", Requests::SLUG_STR, "value"),
            Some(149_705.0)
        );
    }

    /// Kills the `Total` entry read as a benchmark named `Total`, named by the prefix untrimmed
    /// or not at all, or given another entry's values.
    #[test]
    fn adapter_http_jmeter_total_is_the_whole_test() {
        for (prefix, name) in [(None, "jmeter"), (Some("api "), "api")] {
            let results = named("multi.statistics.json", prefix);
            assert!(results.get("Total").is_none(), "{prefix:?}");
            for (measure, expected) in [
                (Requests::SLUG_STR, 601_994.0),
                (FailedRequests::SLUG_STR, 299_731.0),
                (FailureRatio::SLUG_STR, 49.7897),
            ] {
                assert_eq!(
                    value(&results, name, "{}", measure, "value"),
                    Some(expected),
                    "{prefix:?} {measure}"
                );
            }
            assert_eq!(
                value(&results, name, RECEIVED, BytesThroughput::SLUG_STR, "mean"),
                Some(36_209_758.682_185_1),
                "{prefix:?}"
            );
        }
    }

    /// Kills a leaf that needs a `Total` entry: in a Korean locale the dashboard names it `총계`, an
    /// ordinary benchmark.
    #[test]
    fn adapter_http_jmeter_reads_another_locale() {
        let results = read("locale_ko.statistics.json");
        assert!(results.nameless.is_none());
        assert_eq!(
            value(&results, "총계", "{}", Requests::SLUG_STR, "value"),
            Some(300.0)
        );
    }

    /// Kills `medianResTime` required, which refuses the files of 5.1 and 5.2, which have none,
    /// and a median made up for them.
    #[test]
    fn adapter_http_jmeter_reads_5_1_and_5_2() {
        for (suffix, mean) in [
            ("old_5_1.statistics.json", 41_416_000.000_000_01),
            ("old_5_2_1.statistics.json", 41_472_000.000_000_03),
        ] {
            let results = read(suffix);
            assert_eq!(results.skipped, Skipped::default(), "{suffix}");
            assert_eq!(variants(&results), 8 * 3, "{suffix}");
            assert_eq!(
                value(&results, "confirm", "{}", Latency::SLUG_STR, "median"),
                None,
                "{suffix}"
            );
            assert_eq!(
                value(&results, "confirm", "{}", Latency::SLUG_STR, "mean"),
                Some(mean),
                "{suffix}"
            );
        }
    }

    /// Kills the placeholders of an empty Transaction Controller written, so `Long.MAX_VALUE` ms
    /// becomes its `min`, its `NaN` counted as skipped, or its counts dropped with them.
    #[test]
    fn adapter_http_jmeter_empty_controller_writes_counts_only() {
        let results = read("empty_controller.statistics.json");
        assert_eq!(results.skipped, Skipped::default());
        assert_variant(
            &results,
            "empty",
            "{}",
            &[
                count(Requests::SLUG_STR, 0.0),
                count(FailedRequests::SLUG_STR, 0.0),
            ],
        );
        assert!(
            value(
                &results,
                "empty",
                RECEIVED,
                BytesThroughput::SLUG_STR,
                "mean"
            )
            .is_none()
        );
    }

    /// Kills the gate of an entry with no sample kept to the labels: the overall entry leaves out
    /// controller samples (`StatisticsSummaryConsumer.java:72-74` at 5.6.3), so with subresults
    /// off it has none, and its placeholders would be written or counted as skipped.
    #[test]
    fn adapter_http_jmeter_empty_total_writes_counts_only() {
        let results = named("total_empty.statistics.json", None);
        assert_eq!(results.skipped, Skipped::default());
        assert_variant(
            &results,
            "jmeter",
            "{}",
            &[
                count(Requests::SLUG_STR, 0.0),
                count(FailedRequests::SLUG_STR, 0.0),
            ],
        );
        for direction in [RECEIVED, SENT] {
            assert!(
                value(
                    &results,
                    "jmeter",
                    direction,
                    BytesThroughput::SLUG_STR,
                    "mean"
                )
                .is_none(),
                "{direction}"
            );
        }
    }

    /// Kills a rate the dashboard divides by a span of 0 ms written as infinite or as 0, or
    /// counted as skipped, a byte rate's variant built with no metric, and the latency of a 0 ms
    /// sample skipped with it.
    #[test]
    fn adapter_http_jmeter_skips_a_rate_that_is_not_finite_uncounted() {
        let results = named("instant.statistics.json", None);
        assert_eq!(results.skipped, Skipped::default());
        assert_eq!(variants(&results), 2);
        for benchmark in ["debug", "jmeter"] {
            assert_eq!(
                value(&results, benchmark, "{}", Throughput::SLUG_STR, "mean"),
                None,
                "{benchmark}"
            );
            assert_eq!(
                value(&results, benchmark, "{}", Latency::SLUG_STR, "max"),
                Some(0.0),
                "{benchmark}"
            );
        }
    }

    /// Kills a label past the benchmark name limit read, cut, or skipped uncounted.
    #[test]
    fn adapter_http_jmeter_long_label_is_skipped_and_counted() {
        let results = read("long_label.statistics.json");
        assert_eq!(
            results.skipped,
            Skipped {
                benchmarks: 1,
                ..Skipped::default()
            }
        );
        assert_eq!(variants(&results), 3 * 3);
    }

    /// Kills a reader that takes the first of 2 files run together.
    #[test]
    fn adapter_http_jmeter_refuses_several_documents() {
        let one = fixture("fast.statistics.json");
        let settings = Settings::new(None, BmfVersion::V1);
        assert!(LEAF.convert(&one, settings).is_some());
        let two = format!("{one}\n{}", fixture("error.statistics.json"));
        assert_eq!(LEAF.convert(&two, settings), None);
    }

    /// Kills a time scaled in floating point alone, which reads 0.06528824896547775 ms as
    /// 65288.248965477746 ns, and a decimal product read back with `to_f64`, which reads it the
    /// same and 3627.1188162765106 KiB/s as 3714169.6678671464 B/s. A rate in KiB/s scales
    /// exactly in floating point, since 1024 is a power of 2.
    #[test]
    fn adapter_http_jmeter_scales_in_decimal() {
        for (suffix, benchmark, parameters, measure, expected) in [
            (
                "large.statistics.json",
                "item 032",
                "{}",
                Latency::SLUG_STR,
                65_288.248_965_477_75,
            ),
            (
                "multi.statistics.json",
                "checkout",
                RECEIVED,
                BytesThroughput::SLUG_STR,
                3_714_169.667_867_147,
            ),
        ] {
            assert_eq!(
                value(&read(suffix), benchmark, parameters, measure, "mean"),
                Some(expected),
                "{benchmark} {measure}"
            );
        }
    }

    /// Kills a statistic picked by the run's average setting.
    #[test]
    fn adapter_http_jmeter_ignores_average() {
        assert_ignores_average(LEAF, "http/jmeter/multi.statistics.json");
    }
}
