use std::collections::{BTreeMap, HashSet};

use bencher_json::project::{
    measure::built_in::{
        BuiltInMeasure as _,
        default::{Allocations, BytesThroughput, CpuTime, Latency, Throughput},
    },
    report::Adapter,
};
use bencher_valid::BYTES;
use serde::{Deserialize, de::IgnoredAny};

use crate::{
    Adaptable, Settings,
    adapters::{
        bmf_v1::{
            finite::finite_json,
            leaf::{LeafResults, Variant},
            runs::insert_runs,
        },
        util::Units,
    },
    results::adapter_results::AdapterResults,
};

/// Google Benchmark's `--benchmark_format=json`: each run's times, counters, and memory.
///
/// With repetitions, a benchmark's times and counters come from its aggregates, and its memory,
/// which the aggregates do not carry, from the statistics of its repetitions.
pub struct AdapterCppGoogleV1;

impl Adaptable for AdapterCppGoogleV1 {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let Google { benchmarks } = serde_json::from_str(&finite_json(input)).ok()?;
        let aggregated = benchmarks
            .iter()
            .filter(|row| row.run_type == RunType::Aggregate && !row.is_complexity())
            .map(|row| row.run_name.as_str())
            .collect::<HashSet<_>>();
        let mut results = LeafResults::default();
        let mut memory = BTreeMap::<(&str, Option<u64>), BTreeMap<&str, Vec<f64>>>::new();
        for row in &benchmarks {
            if row.error_occurred || row.skipped || row.is_complexity() {
                results.skip_benchmark();
                continue;
            }
            let statistic = if row.run_type == RunType::Aggregate {
                // Releases before 1.6.0 write no unit, and every aggregate of theirs is a time.
                let (Some(name), None | Some(AggregateUnit::Time)) =
                    (&row.aggregate_name, row.aggregate_unit)
                else {
                    continue;
                };
                Some(if name == "stddev" { "std_dev" } else { name })
            } else {
                for (field, value) in row.memory() {
                    if let Some(value) = value {
                        memory
                            .entry((row.run_name.as_str(), row.threads))
                            .or_default()
                            .entry(field)
                            .or_default()
                            .push(value);
                    }
                }
                if aggregated.contains(row.run_name.as_str()) {
                    continue;
                }
                None
            };
            let (name, parameters, skipped) = name_and_parameters(&row.run_name, row.threads);
            let Ok(benchmark_name) = name.parse() else {
                results.skip_benchmark();
                continue;
            };
            let Ok(parameters) = serde_json::from_value(serde_json::Value::Object(parameters))
            else {
                results.skip_variant();
                continue;
            };
            for _ in 0..skipped {
                results.skip_variant();
            }
            row.insert(&mut results.variant(benchmark_name, parameters), statistic);
        }
        for ((run_name, threads), fields) in memory {
            let (name, parameters, _) = name_and_parameters(run_name, threads);
            let (Ok(benchmark_name), Ok(parameters)) = (
                name.parse(),
                serde_json::from_value(serde_json::Value::Object(parameters)),
            ) else {
                continue;
            };
            let mut variant = results.variant(benchmark_name, parameters);
            for (field, values) in fields {
                insert_memory(&mut variant, field, values);
            }
        }
        results.finish(Adapter::CppGoogle)
    }
}

/// Allocations per iteration are a mean, and each byte count one value, unless the benchmark
/// ran with repetitions.
fn insert_memory(variant: &mut Variant<'_>, field: &str, values: Vec<f64>) {
    if field == ALLOCS_PER_ITER {
        insert_runs(variant, Allocations::name_id(), "mean", values);
    } else if let Some(measure) = variant.named(field, BYTES) {
        insert_runs(variant, measure, "value", values);
    }
}

const ALLOCS_PER_ITER: &str = "allocs_per_iter";

/// The run name without its arguments and its `threads:` segment, which become parameters: a
/// named argument by its name, a positional one as `@arg` and its index, and `@threads`, with
/// the number of named arguments skipped. Adapter keys start with `@`, following JSON-LD's
/// convention for keywords, and a named argument with an adapter key is skipped. A last
/// `threads:` segment that differs from the row's `threads` is a named argument `threads`,
/// which Google Benchmark prints the same way.
fn name_and_parameters(
    run_name: &str,
    threads: Option<u64>,
) -> (String, serde_json::Map<String, serde_json::Value>, usize) {
    let mut parameters = serde_json::Map::new();
    let mut segments = run_name.split('/').collect::<Vec<_>>();
    if let Some(threads) = segments
        .last()
        .and_then(|segment| segment.strip_prefix("threads:"))
        .and_then(|segment| segment.parse::<u64>().ok())
        .filter(|number| Some(*number) == threads)
    {
        parameters.insert("@threads".to_owned(), threads.into());
        segments.pop();
    }
    let reserved = segments
        .iter()
        .skip(1)
        .rev()
        .take_while(|segment| is_reserved(segment))
        .count();
    let (named, reserved) = segments.split_at(segments.len() - reserved);
    let arguments = named
        .iter()
        .skip(1)
        .rev()
        .take_while(|segment| argument(segment).is_some())
        .count();
    let (function, arguments) = named.split_at(named.len() - arguments);
    let mut skipped = 0;
    for (index, segment) in arguments.iter().enumerate() {
        let Some((name, value)) = argument(segment) else {
            continue;
        };
        if let Some(name) = name {
            if is_adapter_key(name) {
                skipped += 1;
            } else {
                parameters.insert(name.to_owned(), value.into());
            }
        } else {
            parameters.insert(format!("@arg{index}"), value.into());
        }
    }
    let name = function
        .iter()
        .chain(reserved)
        .copied()
        .collect::<Vec<_>>()
        .join("/");
    (name, parameters, skipped)
}

/// `@threads`, or `@arg` and an index as the leaf writes it.
fn is_adapter_key(name: &str) -> bool {
    name == "@threads"
        || name.strip_prefix("@arg").is_some_and(|index| {
            index
                .parse::<usize>()
                .is_ok_and(|number| number.to_string() == index)
        })
}

/// The segments Google Benchmark appends after the arguments, which stay in the name.
fn is_reserved(segment: &str) -> bool {
    ["min_time:", "min_warmup_time:", "iterations:", "repeats:"]
        .iter()
        .any(|prefix| segment.starts_with(prefix))
        || ["process_time", "real_time", "manual_time"].contains(&segment)
}

/// An argument, `N` or `name:N`, with its name if it has one.
fn argument(segment: &str) -> Option<(Option<&str>, i64)> {
    if let Ok(value) = segment.parse() {
        return Some((None, value));
    }
    let (name, value) = segment.rsplit_once(':')?;
    let value = value.parse().ok()?;
    (!name.is_empty()).then_some((Some(name), value))
}

#[derive(Deserialize)]
struct Google {
    benchmarks: Vec<Row>,
}

#[derive(Deserialize)]
struct Row {
    run_name: String,
    run_type: RunType,
    aggregate_name: Option<String>,
    aggregate_unit: Option<AggregateUnit>,
    #[serde(default)]
    error_occurred: bool,
    #[serde(default)]
    skipped: bool,
    real_time: Option<f64>,
    cpu_time: Option<f64>,
    time_unit: Option<Units>,
    threads: Option<u64>,
    big_o: Option<IgnoredAny>,
    rms: Option<IgnoredAny>,
    allocs_per_iter: Option<f64>,
    max_bytes_used: Option<f64>,
    total_allocated_bytes: Option<f64>,
    net_heap_growth: Option<f64>,
    #[serde(flatten)]
    counters: BTreeMap<String, serde_json::Value>,
}

impl Row {
    /// A `BigO` or RMS row, which fits a curve across a benchmark's arguments.
    fn is_complexity(&self) -> bool {
        self.big_o.is_some() || self.rms.is_some()
    }

    /// Its time per iteration is a mean, and a counter one value, unless the row is an aggregate
    /// of repetitions, named `statistic`; a value that is not finite is skipped and counted.
    fn insert(&self, variant: &mut Variant<'_>, statistic: Option<&str>) {
        let time = statistic.unwrap_or("mean");
        if let Some(units) = self.time_unit {
            for (measure, value) in [
                (Latency::name_id(), self.real_time),
                (CpuTime::name_id(), self.cpu_time),
            ] {
                if let Some(value) = value {
                    variant.insert(measure, time, value * units.as_nanos());
                } else {
                    variant.skip_metric();
                }
            }
        }
        let value = statistic.unwrap_or("value");
        for (counter, number) in &self.counters {
            if FIELDS.contains(&counter.as_str()) {
                continue;
            }
            if number.is_null() {
                variant.skip_metric();
                continue;
            }
            let Some(number) = number.as_f64() else {
                continue;
            };
            let measure = match counter.as_str() {
                "bytes_per_second" => BytesThroughput::name_id(),
                "items_per_second" => Throughput::name_id(),
                _ => {
                    let Ok(measure) = counter.parse() else {
                        variant.skip_measure();
                        continue;
                    };
                    measure
                },
            };
            variant.insert(measure, value, number);
        }
    }

    /// The memory manager's fields, by Google Benchmark's labels.
    fn memory(&self) -> [(&'static str, Option<f64>); 4] {
        [
            (ALLOCS_PER_ITER, self.allocs_per_iter),
            ("max_bytes_used", self.max_bytes_used),
            ("total_allocated_bytes", self.total_allocated_bytes),
            ("net_heap_growth", self.net_heap_growth),
        ]
    }
}

/// The fields of a run that are neither read above nor user counters.
const FIELDS: [&str; 11] = [
    "name",
    "family_index",
    "per_family_instance_index",
    "repetitions",
    "repetition_index",
    "iterations",
    "error_message",
    "skip_message",
    "label",
    "cpu_coefficient",
    "real_coefficient",
];

#[derive(Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RunType {
    Iteration,
    Aggregate,
}

#[derive(Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum AggregateUnit {
    Time,
    Percentage,
}

#[cfg(test)]
mod test_cpp_google_v1 {
    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{Allocations, BytesThroughput, CpuTime, Latency, Throughput},
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::test_util::{
            Expected, assert_ignores_average, assert_units, assert_variant, convert_v1, variants,
        },
    };

    use super::name_and_parameters;

    /// One run's real and CPU time per iteration.
    fn times(real: f64, cpu: f64) -> [Expected; 2] {
        [
            (Latency::SLUG_STR, vec![("mean", real)]),
            (CpuTime::SLUG_STR, vec![("mean", cpu)]),
        ]
    }

    fn value(measure: &'static str, value: f64) -> Expected {
        (measure, vec![("value", value)])
    }

    /// Repetition aggregates, in the order mean, median, standard deviation.
    fn aggregates(measure: &'static str, [mean, median, std_dev]: [f64; 3]) -> Expected {
        (
            measure,
            vec![("mean", mean), ("median", median), ("std_dev", std_dev)],
        )
    }

    /// The statistics of per-repetition rows, in the order mean, median, minimum, maximum,
    /// standard deviation.
    fn repeated(measure: &'static str, [mean, median, min, max, std_dev]: [f64; 5]) -> Expected {
        (
            measure,
            vec![
                ("mean", mean),
                ("median", median),
                ("min", min),
                ("max", max),
                ("std_dev", std_dev),
            ],
        )
    }

    /// The expected measures, then the memory of the repetitions fixture.
    fn with_same_memory<const N: usize>(expected: [Expected; N]) -> Vec<Expected> {
        expected.into_iter().chain(same_memory()).collect()
    }

    /// The memory manager of the repetitions fixture, which reports the same every repetition.
    fn same_memory() -> [Expected; 4] {
        [
            repeated(Allocations::SLUG_STR, [3.0, 3.0, 3.0, 3.0, 0.0]),
            repeated("max_bytes_used", [1_024.0, 1_024.0, 1_024.0, 1_024.0, 0.0]),
            repeated(
                "total_allocated_bytes",
                [16_384.0, 16_384.0, 16_384.0, 16_384.0, 0.0],
            ),
            repeated("net_heap_growth", [0.0; 5]),
        ]
    }

    /// Kills a leaf that keeps an argument or `threads:` in the name, moves a reserved segment
    /// or a capture's name out of it, numbers a positional argument by anything but its index,
    /// reads `@threads` from the `threads` field instead of the name, drops CPU time or a
    /// counter, reads a counter as a statistic, fails the file or keeps a row for an error, a
    /// skip, or a `BigO` or RMS fit, skips one uncounted, or records no leaf.
    #[test]
    fn adapter_cpp_google_v1() {
        let results = convert_v1(Adapter::CppGoogle, "cpp/google/v1_10.json");
        assert_eq!(results.resolved_adapter, Some(Adapter::CppGoogle));
        assert_eq!(results.inner.len(), 8);
        assert_eq!(variants(&results), 13);
        assert_eq!(results.skipped.benchmarks, 4);
        assert_variant(
            &results,
            "BM_fib",
            r#"{"@arg0":10}"#,
            &times(25.140_565_706_647_94, 25.142_681_596_796_844),
        );
        assert_variant(
            &results,
            "BM_reserved/min_time:0.010/repeats:1/real_time",
            r#"{"@arg0":5}"#,
            &times(3.054_057_799_477_470_2, 3.054_136_569_353_148),
        );
        let [latency, cpu_time] = times(4.707_523_286_564_159, 4.707_502_241_764_658);
        assert_variant(
            &results,
            "BM_copy",
            r#"{"@arg1":2,"size":1024}"#,
            &[
                latency,
                cpu_time,
                value(BytesThroughput::SLUG_STR, 217_525_122_115.744_87),
                value(Throughput::SLUG_STR, 212_426_877.066_157_1),
                value("chunks", 16.0),
            ],
        );
        assert_variant(
            &results,
            "BM_sum/small_vec",
            "{}",
            &times(22.598_426_473_846_69, 22.598_052_455_283_273),
        );
        assert_variant(
            &results,
            "BM_threads/real_time",
            r#"{"@threads":2}"#,
            &times(9.855_643_061_483_738, 9.855_271_196_989_68),
        );
        assert_variant(
            &results,
            "BM_manual/manual_time",
            "{}",
            &times(999.999_999_999_726_2, 11.293_357_142_856_475),
        );
        assert_variant(
            &results,
            "BM_complexity",
            r#"{"@arg0":1024}"#,
            &times(194.929_051_841_718_77, 194.923_537_521_061_86),
        );
        assert_units(&results, &[]);
    }

    /// Kills a leaf that keeps a repetition's row, makes each aggregate its own benchmark, names
    /// an aggregate inaccurately, drops a user statistic, a counter's aggregates, or the memory
    /// of the repetitions, or keeps the coefficient of variation, a ratio, as a time.
    #[test]
    fn adapter_cpp_google_v1_repetitions() {
        let results = convert_v1(Adapter::CppGoogle, "cpp/google/repetitions.json");
        assert_eq!(results.inner.len(), 3);
        assert_eq!(variants(&results), 5);
        assert_eq!(results.skipped.benchmarks, 0);
        assert_variant(
            &results,
            "BM_fib",
            r#"{"@arg0":10}"#,
            &with_same_memory([
                (
                    Latency::SLUG_STR,
                    vec![
                        ("mean", 26.412_731_110_385_75),
                        ("median", 26.581_282_819_495_03),
                        ("std_dev", 0.481_040_585_876_116_6),
                        ("max", 26.786_814_052_274_778),
                    ],
                ),
                (
                    CpuTime::SLUG_STR,
                    vec![
                        ("mean", 26.402_012_775_814),
                        ("median", 26.549_686_920_114_947),
                        ("std_dev", 0.476_283_861_072_882_2),
                        ("max", 26.786_968_223_924_102),
                    ],
                ),
            ]),
        );
        assert_variant(
            &results,
            "BM_copy",
            r#"{"@arg1":2,"size":1024}"#,
            &with_same_memory([
                aggregates(
                    Latency::SLUG_STR,
                    [
                        5.405_347_907_895_383,
                        5.342_700_767_222_428,
                        0.139_430_143_240_129_03,
                    ],
                ),
                aggregates(
                    CpuTime::SLUG_STR,
                    [
                        5.405_533_605_910_203,
                        5.342_780_613_711_838,
                        0.139_813_858_202_301_16,
                    ],
                ),
                aggregates(
                    BytesThroughput::SLUG_STR,
                    [
                        189_518_875_233.881_62,
                        191_660_499_286.080_05,
                        4_834_884_434.222_083,
                    ],
                ),
                aggregates(
                    Throughput::SLUG_STR,
                    [
                        185_077_026.595_587_52,
                        187_168_456.334_062_55,
                        4_721_566.830_295_003,
                    ],
                ),
                aggregates("chunks", [16.0, 16.0, 0.0]),
            ]),
        );
        assert_variant(
            &results,
            "BM_alloc",
            r#"{"@arg0":256}"#,
            &with_same_memory([
                aggregates(
                    Latency::SLUG_STR,
                    [
                        12.700_185_888_242_943,
                        12.505_245_291_437_296,
                        0.592_582_174_358_187,
                    ],
                ),
                aggregates(
                    CpuTime::SLUG_STR,
                    [
                        12.699_968_215_696_15,
                        12.505_192_415_477_827,
                        0.592_542_797_748_923_8,
                    ],
                ),
            ]),
        );
    }

    /// Kills a leaf that drops the memory of the repetitions, reads one repetition's, or takes
    /// the population standard deviation.
    #[test]
    fn adapter_cpp_google_v1_memory_repetitions() {
        let results = convert_v1(Adapter::CppGoogle, "cpp/google/memory_repetitions.json");
        assert_eq!(results.inner.len(), 1);
        let std_dev = 1.527_525_231_651_946_8;
        assert_variant(
            &results,
            "BM_alloc",
            r#"{"@arg0":256}"#,
            &[
                aggregates(
                    Latency::SLUG_STR,
                    [
                        12.925_535_975_822_008,
                        12.831_098_545_441_597,
                        0.317_195_727_807_260_1,
                    ],
                ),
                aggregates(
                    CpuTime::SLUG_STR,
                    [
                        12.925_687_558_697_613,
                        12.830_741_592_285_117,
                        0.317_944_348_626_189_25,
                    ],
                ),
                repeated(Allocations::SLUG_STR, [7.0 / 3.0, 2.0, 1.0, 4.0, std_dev]),
                repeated(
                    "max_bytes_used",
                    [
                        1_024.0 * 7.0 / 3.0,
                        2_048.0,
                        1_024.0,
                        4_096.0,
                        1_024.0 * std_dev,
                    ],
                ),
                repeated(
                    "total_allocated_bytes",
                    [
                        4_096.0 * 7.0 / 3.0,
                        8_192.0,
                        4_096.0,
                        16_384.0,
                        4_096.0 * std_dev,
                    ],
                ),
                repeated(
                    "net_heap_growth",
                    [8.0 * 7.0 / 3.0, 16.0, 8.0, 32.0, 8.0 * std_dev],
                ),
            ],
        );
    }

    /// Kills a leaf that divides a time by its unit's nanoseconds, which only a unit other than
    /// `ns` shows, or that skips a user statistic whose name is too long for a metric uncounted:
    /// the run's times are in microseconds, and its statistic's name is 65 characters, one more
    /// than a metric name may be, so its real and CPU time are each skipped and counted.
    #[test]
    fn adapter_cpp_google_v1_microseconds() {
        let results = convert_v1(Adapter::CppGoogle, "cpp/google/microseconds.json");
        assert_eq!(results.inner.len(), 1);
        assert_eq!(results.skipped.metrics, 2);
        assert_variant(
            &results,
            "BM_fib/repeats:3",
            r#"{"@arg0":20}"#,
            &[
                aggregates(
                    Latency::SLUG_STR,
                    [
                        5_364.167_744_860_612,
                        5_356.170_699_065_86,
                        21.347_700_390_509_195,
                    ],
                ),
                aggregates(
                    CpuTime::SLUG_STR,
                    [
                        5_363.727_499_350_023,
                        5_355.955_878_079_74,
                        21.314_792_655_502_752,
                    ],
                ),
            ],
        );
    }

    /// Kills a leaf that needs `aggregate_unit`, which releases before 1.6.0 do not write.
    #[test]
    fn adapter_cpp_google_v1_before_1_6() {
        let results = convert_v1(Adapter::CppGoogle, "cpp/google/v1_5.json");
        assert_eq!(results.inner.len(), 2);
        assert_variant(
            &results,
            "BM_fib",
            r#"{"@arg0":10}"#,
            &[
                aggregates(
                    Latency::SLUG_STR,
                    [
                        34.807_170_792_077_78,
                        36.600_793_223_990_934,
                        3.779_160_295_451_218,
                    ],
                ),
                aggregates(
                    CpuTime::SLUG_STR,
                    [
                        34.809_458_023_108_526,
                        36.606_388_103_679_88,
                        3.780_681_212_562_200_7,
                    ],
                ),
            ],
        );
        assert_variant(
            &results,
            "BM_copy",
            r#"{"@arg0":1024}"#,
            &[
                aggregates(
                    Latency::SLUG_STR,
                    [
                        6.008_782_283_795_614,
                        6.077_157_734_668_482,
                        0.156_950_009_758_641_23,
                    ],
                ),
                aggregates(
                    CpuTime::SLUG_STR,
                    [
                        6.008_980_750_208_841,
                        6.077_298_750_270_935,
                        0.156_935_679_422_926_47,
                    ],
                ),
                aggregates(
                    BytesThroughput::SLUG_STR,
                    [
                        170_490_211_917.190_3,
                        168_495_912_753.071_17,
                        4_515_319_803.518_496_5,
                    ],
                ),
                aggregates("chunks", [16.0, 16.0, 0.0]),
            ],
        );
    }

    /// Kills a leaf that fails the file on Google Benchmark's bare `NaN` and `Infinity`, drops a
    /// counter beside one, or skips one uncounted.
    #[test]
    fn adapter_cpp_google_v1_non_finite() {
        let results = convert_v1(Adapter::CppGoogle, "cpp/google/non_finite.json");
        assert_eq!(results.inner.len(), 1);
        assert_eq!(results.skipped.metrics, 3);
        let [latency, cpu_time] = times(0.251_494_183_585_065_8, 0.251_521_888_449_664_06);
        assert_variant(
            &results,
            "BM_ratio",
            "{}",
            &[latency, cpu_time, value("hits", 7.0)],
        );
    }

    /// Kills a leaf that drops a memory manager field, reads allocations per iteration as
    /// anything but a mean, or gives a field's measure no unit.
    #[test]
    fn adapter_cpp_google_v1_memory() {
        let results = convert_v1(Adapter::CppGoogle, "cpp/google/memory.json");
        assert_eq!(results.inner.len(), 1);
        assert_variant(
            &results,
            "BM_alloc",
            r#"{"@arg0":256}"#,
            &with_memory(
                times(11.769_136_874_126_971, 11.758_176_390_499_395),
                [3.0, 1_024.0, 16_384.0, 0.0],
            ),
        );
        assert_units(
            &results,
            &[
                ("max_bytes_used", "bytes (B)"),
                ("total_allocated_bytes", "bytes (B)"),
                ("net_heap_growth", "bytes (B)"),
            ],
        );
    }

    /// Kills a leaf that needs a field a 2019 release does not write.
    #[test]
    fn adapter_cpp_google_v1_older() {
        let results = convert_v1(Adapter::CppGoogle, "cpp/google/two.txt");
        assert_eq!(results.inner.len(), 2);
        assert_variant(
            &results,
            "fib_20",
            "{}",
            &times(27_455.600_415_007_055, 27_364.903_204_272_36),
        );
    }

    /// Kills a leaf that reads a `NaN` inside a string as a value, which renames `BM_IsNaN`, or
    /// takes a label's escaped quote for its end, which leaves the next row's `NaN` unread and
    /// fails the file, or skips a time that is not finite uncounted.
    #[test]
    fn adapter_cpp_google_v1_non_finite_strings() {
        let results = convert_v1(Adapter::CppGoogle, "cpp/google/nan_strings.json");
        assert_eq!(results.inner.len(), 2);
        assert_eq!(results.skipped.metrics, 2);
        assert_variant(
            &results,
            "BM_IsNaN/iterations:10/manual_time",
            "{}",
            &[(CpuTime::SLUG_STR, vec![("mean", 68.999_999_999_980_53)])],
        );
        let [latency, cpu_time] = times(19.000_000_861_524_313, 18.100_000_000_013_94);
        assert_variant(
            &results,
            "BM_after/iterations:10",
            "{}",
            &[latency, cpu_time, value("hits", 3.0)],
        );
    }

    /// Kills a `finish` that, as `finish_with_skips` does, returns results for an input whose
    /// every benchmark was skipped, which would let a leaf claim another harness's output.
    #[test]
    fn adapter_cpp_google_v1_only_skipped() {
        let errored = r#"{"benchmarks": [{"name": "BM_error", "run_name": "BM_error", "run_type": "iteration", "error_occurred": true, "error_message": "no input", "iterations": 1, "real_time": 0.0, "cpu_time": 0.0, "time_unit": "ns"}]}"#;
        assert_eq!(
            Adapter::CppGoogle.convert(errored, Settings::new(None, BmfVersion::V1)),
            None
        );
    }

    /// Kills a leaf that takes `threads:` for a named argument, which leaves the arguments
    /// before a reserved segment in the name.
    #[test]
    fn adapter_cpp_google_v1_run_name() {
        for (run_name, threads, name, parameters) in [
            (
                "BM_copy/8/real_time/threads:2",
                2,
                "BM_copy/real_time",
                serde_json::json!({ "@arg0": 8, "@threads": 2 }),
            ),
            (
                "BM_sum/small_vec/size:-1/16/repeats:3/process_time/manual_time/threads:4",
                4,
                "BM_sum/small_vec/repeats:3/process_time/manual_time",
                serde_json::json!({ "size": -1, "@arg1": 16, "@threads": 4 }),
            ),
        ] {
            assert_eq!(
                name_and_parameters(run_name, Some(threads)),
                (
                    name.to_owned(),
                    parameters.as_object().cloned().unwrap_or_default(),
                    0
                ),
                "{run_name}"
            );
        }
    }

    /// Ruling 90: a named argument with an adapter key is skipped and counted, whether or not the
    /// run name holds that key, and a named argument with a bare adapter name stays the user's.
    /// Kills a leaf that lets the named argument win, skips it uncounted, keeps a named argument
    /// with an adapter key the run name does not hold, skips an `@arg` index the leaf never
    /// writes (`@arg01`, `@arg+1`), or keeps the bare keys.
    #[test]
    fn adapter_cpp_google_v1_adapter_keys_win() {
        let input = r#"{"benchmarks": [
            {"name": "BM_keys/threads:9/@arg2:7/5/@threads:3/threads:2", "run_name": "BM_keys/threads:9/@arg2:7/5/@threads:3/threads:2", "run_type": "iteration", "threads": 2, "iterations": 1, "real_time": 1.0, "cpu_time": 2.0, "time_unit": "ns"},
            {"name": "BM_unheld/@arg9:1/@threads:3/@arg01:5/@arg+1:6/@arg:4", "run_name": "BM_unheld/@arg9:1/@threads:3/@arg01:5/@arg+1:6/@arg:4", "run_type": "iteration", "threads": 1, "iterations": 1, "real_time": 3.0, "cpu_time": 4.0, "time_unit": "ns"}
        ]}"#;
        let results = Adapter::CppGoogle
            .convert(input, Settings::new(None, BmfVersion::V1))
            .expect("Failed to parse");
        assert_eq!(results.skipped.variants, 4);
        assert_variant(
            &results,
            "BM_keys",
            r#"{"@arg2":5,"@threads":2,"threads":9}"#,
            &times(1.0, 2.0),
        );
        assert_variant(
            &results,
            "BM_unheld",
            r#"{"@arg":4,"@arg+1":6,"@arg01":5}"#,
            &times(3.0, 4.0),
        );
    }

    /// Kills a leaf that counts a clashing named argument only on rows that are not aggregates,
    /// again in the memory pass, on the repetitions its aggregates replace, or on the
    /// coefficient of variation it does not keep.
    #[test]
    fn adapter_cpp_google_v1_adapter_key_per_row() {
        let results = convert_v1(Adapter::CppGoogle, "cpp/google/memory_keys.json");
        assert_eq!(results.inner.len(), 1);
        assert_eq!(variants(&results), 1);
        assert_eq!(results.skipped.variants, 3);
        assert_variant(
            &results,
            "BM_rep/repeats:3",
            "{}",
            &with_same_memory([
                aggregates(
                    Latency::SLUG_STR,
                    [
                        6.966_682_332_565_515,
                        6.956_058_379_715_62,
                        0.024_076_894_081_769_693,
                    ],
                ),
                aggregates(
                    CpuTime::SLUG_STR,
                    [
                        6.966_797_990_457_286_5,
                        6.955_942_038_493_06,
                        0.024_224_273_933_183_01,
                    ],
                ),
            ]),
        );
    }

    /// Call 100: a last `threads:` segment is `@threads` only when it equals the row's `threads`,
    /// and otherwise a named argument. Kills a leaf that takes every last `threads:` segment for
    /// `@threads`, takes none, or reads `@threads` from the field alone, or a memory pass that
    /// drops the row's `threads`, which moves a `Threads(2)` run's memory to `{"threads": 2}`.
    #[test]
    fn adapter_cpp_google_v1_threads_argument() {
        let results = convert_v1(Adapter::CppGoogle, "cpp/google/threads_argument.json");
        assert_eq!(results.inner.len(), 1);
        assert_eq!(variants(&results), 3);
        assert_eq!(results.skipped.variants, 0);
        assert_variant(
            &results,
            "BM_work",
            r#"{"threads":5}"#,
            &with_memory(
                times(3.750_996_117_790_054_4, 3.750_834_663_106_820_3),
                [5.0, 5_120.0, 81_920.0, 1_280.0],
            ),
        );
        assert_variant(
            &results,
            "BM_work",
            r#"{"@threads":2,"threads":5}"#,
            &with_memory(
                times(11.080_132_452_611_3, 11.079_541_132_968_18),
                [10.0, 10_240.0, 163_840.0, 2_560.0],
            ),
        );
        assert_variant(
            &results,
            "BM_work",
            r#"{"@threads":1,"n":4}"#,
            &with_memory(
                times(4.039_312_472_185_44, 4.039_111_426_072_43),
                [4.0, 4_096.0, 65_536.0, 1_024.0],
            ),
        );
    }

    /// The expected measures, then one run's memory manager fields in Google Benchmark's order.
    fn with_memory<const N: usize>(expected: [Expected; N], memory: [f64; 4]) -> Vec<Expected> {
        let [
            allocations,
            max_bytes_used,
            total_allocated_bytes,
            net_heap_growth,
        ] = memory;
        expected
            .into_iter()
            .chain([
                (Allocations::SLUG_STR, vec![("mean", allocations)]),
                value("max_bytes_used", max_bytes_used),
                value("total_allocated_bytes", total_allocated_bytes),
                value("net_heap_growth", net_heap_growth),
            ])
            .collect()
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_cpp_google_v1_ignores_average() {
        for suffix in ["cpp/google/v1_10.json", "cpp/google/repetitions.json"] {
            assert_ignores_average(Adapter::CppGoogle, suffix);
        }
    }
}
