use std::collections::BTreeMap;

use bencher_json::{
    BenchmarkName, MeasureNameId,
    project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{Latency, Throughput},
            jmh::{SampleTime, SingleShotTime},
        },
        report::Adapter,
    },
};
use serde::Deserialize;

use crate::{
    Adaptable, Settings,
    adapters::bmf_v1::{
        finite::Finite,
        leaf::{LeafResults, Variant},
        percentile::percentile_name,
    },
    results::adapter_results::AdapterResults,
};

/// JMH's `-rf json`: each benchmark on a variant of its parameters and `@mode`.
///
/// The primary metric is in the measure of its mode, each secondary result in the primary's unit
/// (an operations counter or a group member) in that measure on a variant that also names its
/// `@label`, and each other secondary result in a measure of its own label and unit. Adapter keys
/// start with `@`, following JSON-LD's convention for keywords, and a parameter spelled like one
/// is skipped.
pub struct AdapterJavaJmhV1;

impl Adaptable for AdapterJavaJmhV1 {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let benchmarks = serde_json::from_str::<Vec<Benchmark>>(input).ok()?;
        let mut results = LeafResults::default();
        for benchmark in benchmarks {
            let Benchmark {
                name,
                mode,
                mut params,
                primary_metric,
                secondary_metrics,
            } = benchmark;
            let (Some((measure, factor)), Ok(benchmark_name)) = (
                primary_measure(&mode, &primary_metric.score_unit),
                name.parse::<BenchmarkName>(),
            ) else {
                results.skip_benchmark();
                continue;
            };
            for key in [MODE, LABEL] {
                if params.remove(key).is_some() {
                    results.skip_variant();
                }
            }
            let mut labelled = Vec::new();
            let mut others = Vec::new();
            for (label, metric) in secondary_metrics {
                if (mode == SAMPLE && is_sample_percentile(&label)) || !metric.has_finite_value() {
                    continue;
                }
                let label = if let Some(label) = label.strip_prefix('·') {
                    label.to_owned()
                } else {
                    label
                };
                if let Some(primary) = primary_measure(&mode, &metric.score_unit) {
                    labelled.push((label, metric, primary));
                } else {
                    others.push((label, metric));
                }
            }
            if let Some(mut variant) =
                jmh_variant(&mut results, &benchmark_name, &params, &mode, None)
            {
                primary_metric.insert(&mut variant, &measure, factor);
                for (label, metric) in others {
                    if is_primary_measure(&label) {
                        variant.skip_measure();
                    } else if let Some(measure) = variant.named(&label, &metric.score_unit) {
                        metric.insert(&mut variant, &measure, 1.0);
                    }
                }
            }
            for (label, metric, (measure, factor)) in labelled {
                if let Some(mut variant) =
                    jmh_variant(&mut results, &benchmark_name, &params, &mode, Some(&label))
                {
                    metric.insert(&mut variant, &measure, factor);
                }
            }
        }
        results.finish_with_skips(Adapter::JavaJmh)
    }
}

const SAMPLE: &str = "sample";
const MODE: &str = "@mode";
const LABEL: &str = "@label";

/// The measure of a mode's primary metric and the factor from its unit to the measure's: a
/// throughput per second, or a time in nanoseconds.
fn primary_measure(mode: &str, unit: &str) -> Option<(MeasureNameId, f64)> {
    if mode == "thrpt" {
        let nanos = nanos(unit.strip_prefix("ops/")?)?;
        return Some((Throughput::name_id(), NANOS_PER_SECOND / nanos));
    }
    let measure = match mode {
        "avgt" => Latency::name_id(),
        SAMPLE => SampleTime::name_id(),
        "ss" => SingleShotTime::name_id(),
        _ => return None,
    };
    Some((measure, nanos(unit.strip_suffix("/op")?)?))
}

/// The nanoseconds in one of JMH's time units.
fn nanos(unit: &str) -> Option<f64> {
    Some(match unit {
        "ns" => 1.0,
        "us" => 1_000.0,
        "ms" => 1_000_000.0,
        "s" => NANOS_PER_SECOND,
        "min" => 60.0 * NANOS_PER_SECOND,
        "hr" => 3_600.0 * NANOS_PER_SECOND,
        "day" => 86_400.0 * NANOS_PER_SECOND,
        _ => return None,
    })
}

const NANOS_PER_SECOND: f64 = 1_000_000_000.0;

/// Whether a secondary result's label names a measure the primary metric of some mode writes.
fn is_primary_measure(label: &str) -> bool {
    Throughput::from_str(label)
        .or_else(|| Latency::from_str(label))
        .or_else(|| SampleTime::from_str(label))
        .or_else(|| SingleShotTime::from_str(label))
        .is_some()
}

/// A percentile that sample mode also writes as a secondary metric: since JMH 1.37 alone or after
/// a secondary result's label and `:`, and before it after the benchmark's label and `·`.
fn is_sample_percentile(label: &str) -> bool {
    [
        "p0.00", "p0.50", "p0.90", "p0.95", "p0.99", "p0.999", "p0.9999", "p1.00",
    ]
    .iter()
    .any(|percentile| {
        label
            .strip_suffix(percentile)
            .is_some_and(|rest| rest.is_empty() || rest.ends_with('·') || rest.ends_with(':'))
    })
}

/// The variant of a benchmark's parameters, its mode, and the label of the secondary result it
/// reports, if any, or `None`, counted as skipped, if they are not a valid parameter set.
fn jmh_variant<'r>(
    results: &'r mut LeafResults,
    benchmark_name: &BenchmarkName,
    params: &BTreeMap<String, String>,
    mode: &str,
    label: Option<&str>,
) -> Option<Variant<'r>> {
    let mut parameters = params
        .iter()
        .map(|(key, value)| (key.clone(), value.as_str().into()))
        .collect::<serde_json::Map<_, _>>();
    parameters.insert(MODE.to_owned(), mode.into());
    if let Some(label) = label {
        parameters.insert(LABEL.to_owned(), label.into());
    }
    let Ok(parameters) = serde_json::from_value(serde_json::Value::Object(parameters)) else {
        results.skip_variant();
        return None;
    };
    Some(results.variant(benchmark_name.clone(), parameters))
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Benchmark {
    #[serde(rename = "benchmark")]
    name: String,
    mode: String,
    #[serde(default)]
    params: BTreeMap<String, String>,
    primary_metric: Metric,
    #[serde(default)]
    secondary_metrics: BTreeMap<String, Metric>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Metric {
    score: Finite,
    #[serde(default)]
    score_error: Finite,
    #[serde(default)]
    score_confidence: [Finite; 2],
    #[serde(default)]
    score_percentiles: BTreeMap<String, Finite>,
    score_unit: String,
    #[serde(default)]
    raw_data: Vec<Vec<Finite>>,
}

impl Metric {
    /// The percentiles of the samples, and the score.
    ///
    /// The score is a mean with JMH's fixed 99.9% interval, unless JMH summed the samples or took
    /// their maximum or minimum, where it writes no error and the score itself as both bounds. A
    /// sum is the total of the raw data's finite values; a maximum or a minimum is already a
    /// percentile, also where some of the raw data is not finite.
    fn insert(&self, variant: &mut Variant<'_>, measure: &MeasureNameId, factor: f64) {
        let Self {
            score,
            score_error,
            score_confidence: [lower, upper],
            score_percentiles,
            ..
        } = self;
        for (percent, value) in score_percentiles {
            if let Some(name) = percentile_name(percent) {
                value.insert(variant, measure, &name, factor);
            } else {
                variant.skip_metric();
            }
        }
        let is_score = |value: Option<f64>| {
            value
                .zip(score.get())
                .is_some_and(|(value, score)| value.total_cmp(&score).is_eq())
        };
        if !(score_error.get().is_none() && is_score(lower.get()) && is_score(upper.get())) {
            score.insert(variant, measure, "mean", factor);
            lower.insert_spread(*score, variant, measure, "mean_ci99_9_lower", factor);
            upper.insert_spread(*score, variant, measure, "mean_ci99_9_upper", factor);
            return;
        }
        if self.raw_data.iter().all(Vec::is_empty) {
            variant.skip_metric();
        } else if score
            .get()
            .is_some_and(|score| is_close(self.total(), score))
        {
            score.insert(variant, measure, "sum", factor);
        } else if !score_percentiles.iter().any(|(percent, value)| {
            matches!(percentile_name(percent).as_deref(), Some("min" | "max"))
                && is_score(value.get())
        }) {
            variant.skip_metric();
        }
    }

    /// The total of the raw data's finite values in JMH's order, where JMH writes an iteration in
    /// which a profiler saw nothing as not a number.
    fn total(&self) -> f64 {
        self.raw_data
            .iter()
            .flatten()
            .filter_map(|value| value.get())
            .fold(0.0, |total, value| total + value)
    }

    /// Whether any value of the result is finite, where JMH writes every value of a profiler's
    /// result that saw nothing as not a number.
    fn has_finite_value(&self) -> bool {
        let Self {
            score,
            score_error,
            score_confidence,
            score_percentiles,
            raw_data,
            ..
        } = self;
        [score, score_error]
            .into_iter()
            .chain(score_confidence)
            .chain(score_percentiles.values())
            .chain(raw_data.iter().flatten())
            .any(|value| value.get().is_some())
    }
}

/// Whether a score and the total of its raw data agree within rounding, where JMH may add the
/// same samples in another order.
fn is_close(total: f64, score: f64) -> bool {
    (total - score).abs() <= SUM_TOLERANCE * total.abs().max(score.abs())
}

const SUM_TOLERANCE: f64 = 1e-12;

#[cfg(test)]
mod test_java_jmh_v1 {
    use bencher_json::{
        BmfVersion,
        project::{
            measure::built_in::{
                BuiltInMeasure as _,
                default::{Latency, Throughput},
                jmh::{SampleTime, SingleShotTime},
            },
            report::Adapter,
        },
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::test_util::{
            Expected, assert_ignores_average, assert_units, assert_variant, convert_edited_v1,
            convert_v1, variants,
        },
        results::adapter_results::{AdapterResults, Skipped},
    };

    /// The percentiles JMH writes, in its order.
    const PERCENTILES: [&str; 10] = [
        "min", "median", "p90", "p95", "p99", "p99_9", "p99_99", "p99_999", "p99_9999", "max",
    ];

    /// A mean with its 99.9% interval, in the order mean, lower, upper, and the percentiles.
    fn scored(
        measure: &'static str,
        [mean, lower, upper]: [f64; 3],
        percentiles: [f64; 10],
    ) -> Expected {
        let (measure, mut metrics) = unscored(measure, percentiles);
        metrics.extend([
            ("mean", mean),
            ("mean_ci99_9_lower", lower),
            ("mean_ci99_9_upper", upper),
        ]);
        (measure, metrics)
    }

    /// A sum over the iterations, and the percentiles of the iterations.
    fn summed(measure: &'static str, sum: f64, percentiles: [f64; 10]) -> Expected {
        let (measure, mut metrics) = unscored(measure, percentiles);
        metrics.push(("sum", sum));
        (measure, metrics)
    }

    /// The percentiles alone.
    fn unscored(measure: &'static str, percentiles: [f64; 10]) -> Expected {
        (measure, PERCENTILES.into_iter().zip(percentiles).collect())
    }

    /// The minimum and the median, then one value for every higher percentile.
    fn rising([min, median]: [f64; 2], rest: f64) -> [f64; 10] {
        let mut percentiles = [rest; 10];
        percentiles[0] = min;
        percentiles[1] = median;
        percentiles
    }

    /// The metric `name` of `measure` on the variant of `parameters`, of any benchmark.
    fn metric(
        results: &AdapterResults,
        parameters: &str,
        measure: &str,
        name: &str,
    ) -> Option<f64> {
        results
            .inner
            .values()
            .flat_map(|variants| variants.iter())
            .filter(|(variant, _)| variant.canonical() == parameters)
            .flat_map(|(_, metrics)| metrics.inner.iter())
            .filter(|(id, _)| id.to_string() == measure)
            .flat_map(|(_, metric)| metric.inner.iter())
            .find(|(metric, _)| metric.to_string() == name)
            .map(|(_, value)| value.into_inner())
    }

    /// Kills a leaf that picks a measure from the unit instead of the mode, leaves a unit
    /// unnormalized, names a percentile or the interval inaccurately, keeps the percentiles sample
    /// mode repeats as secondary metrics, puts the modes of one benchmark in one variant, or
    /// records no leaf.
    #[test]
    fn adapter_java_jmh_v1_modes() {
        let results = convert_v1(Adapter::JavaJmh, "java/jmh/modes.json");
        assert_eq!(results.resolved_adapter, Some(Adapter::JavaJmh));
        assert_eq!(results.inner.len(), 1);
        assert_eq!(variants(&results), 4);
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        for (mode, expected) in [
            (
                "thrpt",
                scored(
                    Throughput::SLUG_STR,
                    [
                        59_845_123.389_306_59,
                        54_301_671.384_435_85,
                        65_388_575.394_177_34,
                    ],
                    rising(
                        [59_494_414.244_504_72, 60_011_512.963_901_654],
                        60_029_442.959_513_426,
                    ),
                ),
            ),
            (
                "avgt",
                scored(
                    Latency::SLUG_STR,
                    [
                        16.882_661_911_473_39,
                        14.268_669_258_418_548,
                        19.496_654_564_528_23,
                    ],
                    rising(
                        [16.718_134_219_556_823, 16.949_837_978_491_793],
                        16.980_013_536_371_555,
                    ),
                ),
            ),
            (
                "sample",
                scored(
                    SampleTime::SLUG_STR,
                    [
                        40.674_535_439_743_33,
                        35.089_147_212_355_96,
                        46.259_923_667_130_7,
                    ],
                    [
                        29.0,
                        31.0,
                        40.0,
                        40.0,
                        78.020_000_000_005_9,
                        3_007.095_999_999_888_6,
                        11_932.953_599_995_468,
                        18_528.0,
                        18_528.0,
                        18_528.0,
                    ],
                ),
            ),
            (
                "ss",
                scored(
                    SingleShotTime::SLUG_STR,
                    [
                        1_024.666_666_666_666_5,
                        -5_364.032_170_766_504,
                        7_413.365_504_099_837,
                    ],
                    rising([761.0, 891.0], 1422.0),
                ),
            ),
        ] {
            assert_variant(
                &results,
                "bench.Hash.hash",
                &format!(r#"{{"@mode":"{mode}","algorithm":"fnv","size":"16"}}"#),
                &[expected],
            );
        }
        assert_units(&results, &[]);
    }

    /// Kills a leaf that drops a score in milliseconds, minutes, hours, or days, or scales one
    /// from the wrong number of nanoseconds.
    #[test]
    fn adapter_java_jmh_v1_time_units() {
        for (suffix, benchmark, mode, measure, interval, [min, median, rest]) in TIME_UNITS {
            let results = convert_v1(Adapter::JavaJmh, suffix);
            assert!(results.skipped.is_empty(), "{suffix} {:?}", results.skipped);
            assert_variant(
                &results,
                benchmark,
                &format!(r#"{{"@mode":"{mode}"}}"#),
                &[scored(measure, interval, rising([min, median], rest))],
            );
        }
    }

    /// The scores of each time unit the other fixtures do not use.
    const TIME_UNITS: [Scaled; 8] = [
        (
            "java/jmh/milliseconds.json",
            "bench.Units.hash",
            "thrpt",
            Throughput::SLUG_STR,
            [
                3_919_612_215.523_421_3,
                -5_644_730_291.135_116,
                13_483_954_722.181_96,
            ],
            [
                3_317_862_972.819_984_4,
                4_163_342_074.260_297,
                4_277_631_599.489_982,
            ],
        ),
        (
            "java/jmh/milliseconds.json",
            "bench.Units.hash",
            "avgt",
            Latency::SLUG_STR,
            [
                0.260_817_739_548_509_56,
                -0.262_638_957_725_956_2,
                0.784_274_436_822_975_4,
            ],
            [
                0.227_798_956_135_380_4,
                0.274_965_907_967_511_14,
                0.279_688_354_542_637_13,
            ],
        ),
        (
            "java/jmh/minutes.json",
            "bench.Units.hash",
            "thrpt",
            Throughput::SLUG_STR,
            [
                4_501_606_006.032_665,
                4_244_421_647.336_49,
                4_758_790_364.728_84,
            ],
            [
                4_492_117_858.385_467,
                4_494_895_355.146_492,
                4_517_804_804.566_036,
            ],
        ),
        (
            "java/jmh/minutes.json",
            "bench.Units.hash",
            "avgt",
            Latency::SLUG_STR,
            [
                0.223_185_944_889_129_02,
                0.195_378_344_762_099_7,
                0.250_993_545_016_158_34,
            ],
            [
                0.221_514_690_107_060_68,
                0.223_543_604_925_913_74,
                0.224_499_539_634_412_74,
            ],
        ),
        (
            "java/jmh/hours_days.json",
            "bench.Units.hours",
            "thrpt",
            Throughput::SLUG_STR,
            [
                4_675_004_717.308_434,
                4_169_385_502.121_331,
                5_180_623_932.495_535,
            ],
            [
                4_650_265_446.215_224,
                4_669_793_662.140_09,
                4_704_955_043.569_984,
            ],
        ),
        (
            "java/jmh/hours_days.json",
            "bench.Units.hours",
            "avgt",
            Latency::SLUG_STR,
            [
                0.228_568_119_341_220_54,
                0.118_564_435_553_413_95,
                0.338_571_803_129_027_07,
            ],
            [
                0.221_611_873_188_207_58,
                0.231_791_414_480_838_18,
                0.232_301_070_354_615_92,
            ],
        ),
        (
            "java/jmh/hours_days.json",
            "bench.Units.days",
            "thrpt",
            Throughput::SLUG_STR,
            [
                4_570_741_356.718_307,
                1_357_253_400.938_744,
                7_784_229_312.497_869,
            ],
            [
                4_401_385_058.275_224,
                4_557_875_264.862_159,
                4_752_963_747.017_534,
            ],
        ),
        (
            "java/jmh/hours_days.json",
            "bench.Units.days",
            "avgt",
            Latency::SLUG_STR,
            [
                0.208_674_578_273_748_55,
                0.156_257_280_480_221_64,
                0.261_091_876_067_275_46,
            ],
            [
                0.205_357_715_255_800_63,
                0.210_270_442_226_365_76,
                0.210_395_577_339_079_3,
            ],
        ),
    ];

    /// A fixture, a benchmark and mode in it, its measure, its mean and interval, and its minimum,
    /// median, and every higher percentile, in the measure's unit.
    type Scaled = (
        &'static str,
        &'static str,
        &'static str,
        &'static str,
        [f64; 3],
        [f64; 3],
    );

    /// Kills a leaf that drops a mode's secondary metrics, or reads a summed event counter as a
    /// mean.
    #[test]
    fn adapter_java_jmh_v1_every_mode_keeps_its_secondaries() {
        let results = convert_v1(Adapter::JavaJmh, "java/jmh/modes_gc_aux.json");
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        assert_eq!(variants(&results), 8);
        assert_variant(
            &results,
            "bench.Counters.lookup",
            r#"{"@mode":"thrpt"}"#,
            &[
                scored(
                    Throughput::SLUG_STR,
                    [
                        465_860_058.806_990_4,
                        -1_362_686_650.574_045_2,
                        2_294_406_768.188_026,
                    ],
                    rising(
                        [352_893_625.161_672_7, 500_554_423.717_013_66],
                        544_132_127.542_284_8,
                    ),
                ),
                summed(
                    "evictions",
                    4_370_533.0,
                    rising([1_103_431.0, 1_565_442.0], 1_701_660.0),
                ),
                scored(
                    "gc.alloc.rate",
                    [
                        6_212.527_581_754_672,
                        -18_175.503_875_830_32,
                        30_600.559_039_339_663,
                    ],
                    rising(
                        [4_706.151_031_345_094, 6_673.944_427_779_327],
                        7_257.487_286_139_592_5,
                    ),
                ),
                scored(
                    "gc.alloc.rate.norm",
                    [
                        14.000_079_366_019_81,
                        13.999_867_883_645_988,
                        14.000_290_848_393_63,
                    ],
                    rising(
                        [14.000_071_405_536_948, 14.000_074_026_982_936],
                        14.000_092_665_539_539,
                    ),
                ),
                summed("gc.count", 14.0, rising([4.0, 5.0], 5.0)),
                summed("gc.time", 16.0, rising([4.0, 5.0], 7.0)),
            ],
        );
        assert_units(
            &results,
            &[
                ("evictions", "#"),
                ("gc.alloc.rate", "MB/sec"),
                ("gc.alloc.rate.norm", "B/op"),
                ("gc.count", "counts"),
                ("gc.time", "ms"),
            ],
        );
    }

    /// Kills a leaf that puts every mode of a benchmark on one variant, where the last mode's
    /// secondary metrics replace the others', or checks for the maximum before the sum, which
    /// drops a single shot's count.
    #[test]
    fn adapter_java_jmh_v1_each_mode_has_its_own_secondaries() {
        let results = convert_v1(Adapter::JavaJmh, "java/jmh/modes_gc_aux.json");
        assert_eq!(
            ["thrpt", "avgt", "sample", "ss"].map(|mode| metric(
                &results,
                &format!(r#"{{"@mode":"{mode}"}}"#),
                "gc.alloc.rate",
                "mean"
            )),
            [
                Some(6_212.527_581_754_672),
                Some(5_692.572_466_675_389_5),
                Some(2_861.108_333_679_017_5),
                Some(70.226_669_355_849_13),
            ]
        );
        assert_eq!(
            metric(&results, r#"{"@mode":"ss"}"#, "evictions", "sum"),
            Some(0.0)
        );
    }

    /// Kills a leaf that keeps an operations counter as a measure of its own, leaves its unit
    /// unnormalized, puts it in a measure other than its mode's, gives it fewer statistics than
    /// the primary score, or puts a `@label` key on the primary score's variant.
    #[test]
    fn adapter_java_jmh_v1_operations_counters() {
        let results = convert_v1(Adapter::JavaJmh, "java/jmh/modes_gc_aux.json");
        assert_variant(
            &results,
            "bench.Counters.lookup",
            r#"{"@label":"hits","@mode":"thrpt"}"#,
            &[scored(
                Throughput::SLUG_STR,
                [
                    349_395_044.105_226_2,
                    -1_022_014_982.976_497_3,
                    1_720_805_071.186_949_7,
                ],
                rising(
                    [264_670_218.871_254_53, 375_415_819.036_794_25],
                    408_099_094.407_629_8,
                ),
            )],
        );
        assert_variant(
            &results,
            "bench.Counters.lookup",
            r#"{"@label":"misses","@mode":"avgt"}"#,
            &[scored(
                Latency::SLUG_STR,
                [
                    9.712_386_531_364_022,
                    -32.389_520_541_503_984,
                    51.814_293_604_232_02,
                ],
                rising(
                    [7.722_373_478_845_27, 9.172_601_543_455_855],
                    12.242_184_571_790_94,
                ),
            )],
        );

        let results = convert_v1(Adapter::JavaJmh, "java/jmh/aux_counters.json");
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        assert_eq!(variants(&results), 3);
        assert_variant(
            &results,
            "bench.Counters.lookup",
            r#"{"@mode":"thrpt"}"#,
            &[
                scored(
                    Throughput::SLUG_STR,
                    [
                        2_129_660_507.662_81,
                        -44_791_013.513_335_47,
                        4_304_112_028.838_956,
                    ],
                    rising(
                        [1_999_434_773.959_776_2, 2_156_212_157.634_065_6],
                        2_233_334_591.394_588,
                    ),
                ),
                summed(
                    "evictions",
                    1_248_765.0,
                    rising([390_769.0, 421_530.0], 436_466.0),
                ),
            ],
        );
        assert_variant(
            &results,
            "bench.Counters.lookup",
            r#"{"@label":"misses","@mode":"thrpt"}"#,
            &[scored(
                Throughput::SLUG_STR,
                [
                    532_415_127.332_097_35,
                    -11_197_740.511_902_869,
                    1_076_027_995.176_097_6,
                ],
                rising(
                    [499_858_694.739_128_6, 539_053_039.408_516_4],
                    558_333_647.848_647,
                ),
            )],
        );
        assert_units(&results, &[("evictions", "#")]);
    }

    /// Kills a leaf that keeps the parameters of a counter's variant past the limit of 8 keys,
    /// skips one uncounted, or skips the primary score's variant with it.
    #[test]
    fn adapter_java_jmh_v1_counter_past_the_parameter_limit() {
        let results = convert_v1(Adapter::JavaJmh, "java/jmh/wide_params.json");
        assert_eq!(results.skipped.variants, 1);
        assert_eq!(variants(&results), 1);
        assert_variant(
            &results,
            "bench.Wide.sum",
            r#"{"@mode":"thrpt","a":"1","b":"2","c":"3","d":"4","e":"5","f":"6","g":"7"}"#,
            &[scored(
                Throughput::SLUG_STR,
                [
                    1_664_020_322.597_768,
                    1_085_898_771.310_108,
                    2_242_141_873.885_428_4,
                ],
                rising(
                    [1_629_579_720.359_719_5, 1_670_537_199.473_437],
                    1_691_944_047.960_147_4,
                ),
            )],
        );
    }

    /// Kills a leaf that keeps a variant with a parameter value past the limit, skips it
    /// uncounted, or fails a payload whose only variant it skips, which refuses a report that
    /// should carry a warning.
    #[test]
    fn adapter_java_jmh_v1_parameter_past_the_limit() {
        let results = convert_v1(Adapter::JavaJmh, "java/jmh/long_param.json");
        assert_eq!(results.skipped.variants, 1);
        assert_eq!(variants(&results), 1);
        assert_variant(
            &results,
            "bench.Labels.length",
            r#"{"@mode":"avgt","label":"short"}"#,
            &[scored(
                Latency::SLUG_STR,
                [
                    0.399_679_572_192_287_67,
                    0.035_614_404_258_703_55,
                    0.763_744_740_125_871_8,
                ],
                rising(
                    [0.382_507_316_435_910_7, 0.394_959_205_210_788_3],
                    0.421_572_194_930_163_96,
                ),
            )],
        );

        let results = convert_v1(Adapter::JavaJmh, "java/jmh/long_param_only.json");
        assert!(results.inner.is_empty());
        assert_eq!(results.skipped.variants, 1);
    }

    /// Kills a leaf that reads a score JMH summed as a mean, keeps the score of a maximum or a
    /// minimum or counts it, drops a score uncounted where the raw data is missing, takes
    /// missing raw data for a sum of zero, takes a score that misses the total by more than
    /// rounding for a sum, or counts a result with no finite value.
    #[test]
    fn adapter_java_jmh_v1_sums_and_extremes() {
        let results = convert_v1(Adapter::JavaJmh, "java/jmh/profilers.json");
        assert_eq!(
            results.skipped,
            Skipped {
                metrics: 9,
                ..Skipped::default()
            }
        );
        let only = |results: &AdapterResults, measure: &str| {
            results
                .inner
                .values()
                .flat_map(|variants| variants.values())
                .flat_map(|metrics| metrics.inner.iter())
                .filter(|(name, _)| name.to_string() == measure)
                .flat_map(|(_, metric)| metric.inner.keys().map(ToString::to_string))
                .collect::<Vec<_>>()
        };
        let percentiles = PERCENTILES.map(str::to_owned).to_vec();
        let mut sum = percentiles.clone();
        sum.push("sum".to_owned());
        sum.sort();
        let mut sorted = percentiles;
        sorted.sort();
        assert_eq!(only(&results, "compiler.time.profiled"), sum);
        assert_eq!(only(&results, "compiler.time.total"), sorted);
        assert_eq!(only(&results, "mempool.total.used"), sorted);
        assert_eq!(only(&results, "safepoints.pause.count"), sorted);

        let minimum = convert_edited_v1(Adapter::JavaJmh, "java/jmh/profilers.json", |json| {
            let total = &mut json[0]["secondaryMetrics"]["compiler.time.total"];
            total["score"] = 143.0.into();
            total["scoreConfidence"] = serde_json::json!([143.0, 143.0]);
        });
        assert_eq!(minimum.skipped.metrics, 9);
        assert_eq!(only(&minimum, "compiler.time.total"), sorted);

        let zero = convert_edited_v1(Adapter::JavaJmh, "java/jmh/profilers.json", |json| {
            let count = &mut json[0]["secondaryMetrics"]["safepoints.pause.count"];
            count["score"] = 0.0.into();
            count["scoreConfidence"] = serde_json::json!([0.0, 0.0]);
        });
        assert_eq!(zero.skipped.metrics, 9);
        assert_eq!(only(&zero, "safepoints.pause.count"), sorted);

        let off = convert_edited_v1(Adapter::JavaJmh, "java/jmh/profilers.json", |json| {
            let profiled = &mut json[0]["secondaryMetrics"]["compiler.time.profiled"];
            let score = profiled["score"].as_f64().unwrap_or_default() * (1.0 + 1e-9);
            profiled["score"] = score.into();
            profiled["scoreConfidence"] = serde_json::json!([score, score]);
        });
        assert_eq!(off.skipped.metrics, 10);
        assert_eq!(only(&off, "compiler.time.profiled"), sorted);
    }

    /// Kills a leaf that reads an average whose samples are all equal, with an error of 0 and its
    /// score as both bounds, as a sum, as the class loading profiler writes it when no class loads.
    #[test]
    fn adapter_java_jmh_v1_average_of_equal_samples() {
        let results = convert_v1(Adapter::JavaJmh, "java/jmh/zero_error.json");
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        let mut expected = vec![scored(
            Latency::SLUG_STR,
            [
                0.974_792_415_641_570_4,
                0.828_584_466_652_997_2,
                1.121_000_364_630_143_6,
            ],
            rising(
                [0.969_438_922_384_446_2, 0.970_932_213_489_359_9],
                0.984_006_111_050_905_1,
            ),
        )];
        let classes = [
            ("class.load", "classes/sec"),
            ("class.load.norm", "classes/op"),
            ("class.unload", "classes/sec"),
            ("class.unload.norm", "classes/op"),
        ];
        expected.extend(classes.map(|(label, _)| scored(label, [0.0; 3], [0.0; 10])));
        assert_variant(
            &results,
            "bench.Steady.lookup",
            r#"{"@mode":"avgt"}"#,
            &expected,
        );
        assert_units(&results, &classes);
    }

    /// Kills a leaf that drops a parameter the user named like an adapter key without `@`, keeps
    /// one spelled like an adapter key, which then means the user's value, skips it uncounted, or
    /// reserves `@label` only on the variants that set it.
    #[test]
    fn adapter_java_jmh_v1_adapter_keys() {
        let input = r#"[{"benchmark": "bench.Named.run", "mode": "avgt", "params": {"@label": "theirs", "@mode": "fast", "counter": "mine", "label": "x", "mode": "slow"}, "primaryMetric": {"score": 2.0, "scoreError": 0.5, "scoreConfidence": [1.5, 2.5], "scorePercentiles": {}, "scoreUnit": "ns/op"}, "secondaryMetrics": {"hits": {"score": 3.0, "scoreError": 0.5, "scoreConfidence": [2.5, 3.5], "scorePercentiles": {}, "scoreUnit": "ns/op"}}}]"#;
        let results = Adapter::JavaJmh
            .convert(input, Settings::new(None, BmfVersion::V1))
            .expect("expected the leaf to parse the payload");
        assert_eq!(
            results.skipped,
            Skipped {
                variants: 2,
                ..Skipped::default()
            }
        );
        assert_eq!(variants(&results), 2);
        for (parameters, [mean, lower, upper]) in [
            (
                r#"{"@mode":"avgt","counter":"mine","label":"x","mode":"slow"}"#,
                [2.0, 1.5, 2.5],
            ),
            (
                r#"{"@label":"hits","@mode":"avgt","counter":"mine","label":"x","mode":"slow"}"#,
                [3.0, 2.5, 3.5],
            ),
        ] {
            assert_variant(
                &results,
                "bench.Named.run",
                parameters,
                &[(
                    Latency::SLUG_STR,
                    vec![
                        ("mean", mean),
                        ("mean_ci99_9_lower", lower),
                        ("mean_ci99_9_upper", upper),
                    ],
                )],
            );
        }
    }

    /// Kills a leaf that keeps a group's members as measures of their own, names them under
    /// another key, leaves them unnormalized, or merges them with the group's own score.
    #[test]
    fn adapter_java_jmh_v1_group() {
        let results = convert_v1(Adapter::JavaJmh, "java/jmh/group.json");
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        assert_eq!(results.inner.len(), 1);
        assert_eq!(variants(&results), 6);
        for (parameters, expected) in [
            (
                r#"{"@mode":"thrpt"}"#,
                scored(
                    Throughput::SLUG_STR,
                    [
                        493_383_359.501_753_03,
                        -2_386_556_373.085_597,
                        3_373_323_092.089_102_7,
                    ],
                    rising(
                        [390_201_614.636_447, 414_840_926.053_859],
                        675_107_537.814_953,
                    ),
                ),
            ),
            (
                r#"{"@label":"get","@mode":"thrpt"}"#,
                scored(
                    Throughput::SLUG_STR,
                    [
                        406_026_470.366_356_1,
                        -2_528_991_524.692_511_6,
                        3_341_044_465.425_223_4,
                    ],
                    rising(
                        [301_301_472.445_517_3, 325_511_904.938_948_4],
                        591_266_033.714_602_6,
                    ),
                ),
            ),
            (
                r#"{"@label":"put","@mode":"thrpt"}"#,
                scored(
                    Throughput::SLUG_STR,
                    [
                        87_356_889.135_396_88,
                        31_677_856.812_500_782,
                        143_035_921.458_293,
                    ],
                    rising(
                        [83_841_504.100_350_34, 88_900_142.190_929_7],
                        89_329_021.114_910_59,
                    ),
                ),
            ),
            (
                r#"{"@label":"put","@mode":"avgt"}"#,
                scored(
                    Latency::SLUG_STR,
                    [
                        10.545_470_454_506_777,
                        -4.948_456_954_040_86,
                        26.039_397_863_054_413,
                    ],
                    rising(
                        [10.023_648_039_128_847, 10.087_324_567_839_786],
                        11.525_438_756_551_698,
                    ),
                ),
            ),
        ] {
            assert_variant(&results, "bench.Group.cell", parameters, &[expected]);
        }
        assert_units(&results, &[]);
    }

    /// Kills a leaf that writes a secondary result named like a measure it writes for any mode, by
    /// its slug or its name, into that measure, which overwrites the primary score's statistics,
    /// or drops one uncounted.
    #[test]
    fn adapter_java_jmh_v1_secondary_named_like_its_measures() {
        let results = convert_v1(Adapter::JavaJmh, "java/jmh/clash.json");
        assert_eq!(
            results.skipped,
            Skipped {
                measures: 2,
                ..Skipped::default()
            }
        );
        assert_variant(
            &results,
            "bench.Clash.count",
            r#"{"@mode":"thrpt"}"#,
            &[scored(
                Throughput::SLUG_STR,
                [
                    2_801_779_764.258_327_5,
                    -1_722_457_898.205_703_7,
                    7_326_017_426.722_359,
                ],
                rising(
                    [2_657_044_171.021_331_3, 2_660_168_315.266_184],
                    3_088_126_806.487_467,
                ),
            )],
        );
        assert_units(&results, &[]);

        let renamed = convert_edited_v1(Adapter::JavaJmh, "java/jmh/clash.json", |json| {
            let secondaries = &mut json[0]["secondaryMetrics"];
            for (from, to) in [
                ("latency", SampleTime::SLUG_STR),
                ("throughput", SingleShotTime::SLUG_STR),
            ] {
                if let Some(secondaries) = secondaries.as_object_mut()
                    && let Some(secondary) = secondaries.remove(from)
                {
                    secondaries.insert(to.to_owned(), secondary);
                }
            }
        });
        assert_eq!(renamed.skipped.measures, 2);
        assert_units(&renamed, &[]);

        let named = convert_v1(Adapter::JavaJmh, "java/jmh/clash_names.json");
        assert_eq!(
            named.skipped,
            Skipped {
                measures: 2,
                ..Skipped::default()
            }
        );
        assert_units(&named, &[]);
    }

    /// Kills a leaf that skips an operations counter named like a measure it writes, which is a
    /// variant of its own `@label` and leaves the primary score as it is.
    #[test]
    fn adapter_java_jmh_v1_counter_named_like_its_measure() {
        let results = convert_v1(Adapter::JavaJmh, "java/jmh/clash_names.json");
        assert_eq!(variants(&results), 3);
        assert_eq!(
            metric(
                &results,
                r#"{"@label":"throughput","@mode":"thrpt"}"#,
                Throughput::SLUG_STR,
                "mean"
            ),
            Some(2_766_687_683.355_669)
        );
    }

    /// Kills a leaf that compares a sum with the total of its raw data bit for bit or within an
    /// absolute tolerance, which counts the total of the `pauses` profiler, whose sum adds its
    /// samples in another order.
    #[test]
    fn adapter_java_jmh_v1_sum_within_tolerance() {
        let results = convert_v1(Adapter::JavaJmh, "java/jmh/pauses_all.json");
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        assert_eq!(
            metric(&results, r#"{"@mode":"thrpt"}"#, "pauses", "sum"),
            Some(21.194_752_000_000_012)
        );
        assert_eq!(
            metric(&results, r#"{"@mode":"thrpt"}"#, "pauses.count", "sum"),
            Some(125.0)
        );

        let long = convert_v1(Adapter::JavaJmh, "java/jmh/pauses_long.json");
        assert!(long.skipped.is_empty(), "{:?}", long.skipped);
        assert_eq!(
            metric(&long, r#"{"@mode":"thrpt"}"#, "pauses", "sum"),
            Some(14_706.296_448)
        );
    }

    /// Kills a leaf that totals the raw data in another order than JMH, from the last value, fork
    /// by fork, or with compensation, which on values that cancel misses JMH's sum by far more
    /// than rounding.
    #[test]
    fn adapter_java_jmh_v1_sum_in_order() {
        let results = convert_v1(Adapter::JavaJmh, "java/jmh/frac.json");
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        assert_eq!(
            metric(&results, r#"{"@mode":"thrpt"}"#, "drift", "sum"),
            Some(2.0)
        );
    }

    /// Kills a leaf that drops a secondary result whose score is not a number though some of its
    /// values are, or keeps its interval beside the mean it could not keep uncounted.
    #[test]
    fn adapter_java_jmh_v1_partly_finite_secondary() {
        let results = convert_v1(Adapter::JavaJmh, "java/jmh/partial.json");
        assert_eq!(
            results.skipped,
            Skipped {
                metrics: 4,
                ..Skipped::default()
            }
        );
        let (_, mut drift) = unscored("drift", [1.0; 10]);
        drift.remove(0);
        let mut expected = vec![("drift", drift)];
        expected.extend([scored(
            Throughput::SLUG_STR,
            [
                3_033_006_957.903_909_7,
                -10_730_996_510.957_483,
                16_797_010_426.765_303,
            ],
            rising(
                [2_162_331_162.380_112, 3_443_051_141.817_722_3],
                3_493_638_569.513_894_6,
            ),
        )]);
        assert_variant(
            &results,
            "bench.Partial.run",
            r#"{"@mode":"thrpt"}"#,
            &expected,
        );
        assert_units(&results, &[("drift", "#")]);
    }

    /// Kills a leaf that counts a minimum or a maximum whose raw data is partly not finite though
    /// it equals the kept `min` or `max`, drops a mean beside it, or counts a sum whose raw data is
    /// partly not a number, where JMH's score is the total of the finite values.
    #[test]
    fn adapter_java_jmh_v1_extremes_with_partial_raw_data() {
        let results = convert_v1(Adapter::JavaJmh, "java/jmh/pauses_some.json");
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        let parameters = r#"{"@mode":"thrpt"}"#;
        for (label, value) in [
            ("pauses.p0.00", 0.137_471_999_999_999_98),
            ("pauses.p1.00", 0.165_887_999_999_999_98),
        ] {
            assert_eq!(metric(&results, parameters, label, "min"), Some(value));
            assert_eq!(metric(&results, parameters, label, "sum"), None);
        }
        assert_eq!(
            metric(&results, parameters, "pauses", "sum"),
            Some(1.063_424)
        );
        assert_eq!(
            metric(&results, parameters, "pauses.avg", "mean"),
            Some(0.151_917_714_285_714_27)
        );
    }

    /// Kills a leaf that keeps JMH's `·` before a label, gives a secondary metric's measure no unit
    /// or a normalized one, or reads a score JMH summed as a mean.
    #[test]
    fn adapter_java_jmh_v1_secondary() {
        let results = convert_v1(Adapter::JavaJmh, "java/jmh/gc_1_36.json");
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        assert_variant(
            &results,
            "bench.Hash.allocate",
            r#"{"@mode":"avgt","algorithm":"fnv","size":"16"}"#,
            &[
                scored(
                    Latency::SLUG_STR,
                    [
                        1.684_420_752_177_346_9,
                        -6.365_783_263_925_682,
                        9.734_624_768_280_376,
                    ],
                    rising(
                        [1.300_232_642_274_620_9, 1.586_671_760_615_341],
                        2.166_357_853_642_079,
                    ),
                ),
                scored(
                    "gc.alloc.rate",
                    [
                        18_904.735_875_878_98,
                        -66_617.880_891_156_92,
                        104_427.352_642_914_88,
                    ],
                    rising(
                        [14_071.481_833_502_976, 19_210.663_267_093_94],
                        23_432.062_527_040_038,
                    ),
                ),
                scored(
                    "gc.alloc.rate.norm",
                    [
                        32.000_003_433_822_75,
                        31.999_987_018_599_978,
                        32.000_019_849_045_52,
                    ],
                    rising(
                        [32.000_002_650_409_3, 32.000_003_234_521_58],
                        32.000_004_416_537_365,
                    ),
                ),
                summed("gc.count", 16.0, rising([5.0, 5.0], 6.0)),
                summed("gc.time", 15.0, rising([4.0, 4.0], 7.0)),
            ],
        );
        assert_units(
            &results,
            &[
                ("gc.alloc.rate", "MB/sec"),
                ("gc.alloc.rate.norm", "B/op"),
                ("gc.count", "counts"),
                ("gc.time", "ms"),
            ],
        );
    }

    /// Kills a leaf that reads a parameter's value as a number, drops a parameter, or merges the
    /// variants of one benchmark.
    #[test]
    fn adapter_java_jmh_v1_params() {
        let results = convert_v1(Adapter::JavaJmh, "java/jmh/params_gc.json");
        assert_eq!(results.inner.len(), 2);
        assert_eq!(variants(&results), 8);
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        assert_variant(
            &results,
            "bench.Hash.allocate",
            r#"{"@mode":"avgt","algorithm":"fnv","size":"256"}"#,
            &[
                scored(
                    Latency::SLUG_STR,
                    [
                        11.438_102_873_063_926,
                        -45.423_318_347_388_87,
                        68.299_524_093_516_72,
                    ],
                    rising(
                        [9.564_057_802_402_617, 9.714_261_095_133_498],
                        15.035_989_721_655_659,
                    ),
                ),
                scored(
                    "gc.alloc.rate",
                    [
                        23_660.982_346_388_18,
                        -77_948.689_969_420_7,
                        125_270.654_662_197_05,
                    ],
                    rising(
                        [17_234.596_352_571_487, 26_659.067_910_369_32],
                        27_089.282_776_223_73,
                    ),
                ),
                scored(
                    "gc.alloc.rate.norm",
                    [
                        272.000_395_490_863,
                        271.998_438_872_355_05,
                        272.002_352_109_371,
                    ],
                    rising(
                        [272.000_331_805_565_57, 272.000_335_352_776_1],
                        272.000_519_314_247_36,
                    ),
                ),
                summed("gc.count", 20.0, rising([6.0, 7.0], 7.0)),
                summed("gc.time", 17.0, rising([4.0, 4.0], 9.0)),
            ],
        );
    }

    /// Kills a leaf that fails the payload on JMH's quoted `NaN`, `+INF`, or `-INF`, drops a
    /// metric beside one, counts the interval JMH cannot compute from one sample beside its mean,
    /// or drops uncounted an interval whose mean it could not keep.
    #[test]
    fn adapter_java_jmh_v1_non_finite() {
        let results = convert_v1(Adapter::JavaJmh, "java/jmh/single_shot.json");
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        let (measure, mut metrics) = unscored(SingleShotTime::SLUG_STR, [12_649.0; 10]);
        metrics.push(("mean", 12_649.0));
        assert_variant(
            &results,
            "bench.Hash.hash",
            r#"{"@mode":"ss","algorithm":"fnv","size":"16"}"#,
            &[(measure, metrics)],
        );

        let infinite = r#"[{"benchmark": "bench.Infinite.run", "mode": "thrpt", "primaryMetric": {"score": "+INF", "scoreError": "NaN", "scoreConfidence": ["-INF", "+INF"], "scorePercentiles": {"0.0": 2.0, "100.0": "+INF"}, "scoreUnit": "ops/s"}, "secondaryMetrics": {}}]"#;
        let results = Adapter::JavaJmh
            .convert(infinite, Settings::new(None, BmfVersion::V1))
            .expect("expected the leaf to parse the payload");
        assert_eq!(results.skipped.metrics, 4);
        assert_variant(
            &results,
            "bench.Infinite.run",
            r#"{"@mode":"thrpt"}"#,
            &[(Throughput::SLUG_STR, vec![("min", 2.0)])],
        );
    }

    /// Kills a leaf that keeps the percentiles sample mode repeats after the benchmark's label and
    /// `·`, as JMH wrote them before 1.37, as variants of their own label.
    #[test]
    fn adapter_java_jmh_v1_sample_before_1_37() {
        let results = convert_v1(Adapter::JavaJmh, "java/jmh/sample_1_36.json");
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        assert_eq!(variants(&results), 1);
        assert_variant(
            &results,
            "bench.Hash.hash",
            r#"{"@mode":"sample","algorithm":"fnv","size":"16"}"#,
            &[scored(
                SampleTime::SLUG_STR,
                [
                    47.202_416_107_382_53,
                    39.050_819_511_882_445,
                    55.354_012_702_882_61,
                ],
                [
                    29.0,
                    40.0,
                    40.0,
                    41.0,
                    80.0,
                    2_219.396_000_001_987,
                    20_160.470_399_995_625,
                    22_560.0,
                    22_560.0,
                    22_560.0,
                ],
            )],
        );
        assert_units(&results, &[]);
    }

    /// Kills a leaf that keeps the percentiles sample mode repeats after a group member's label
    /// and `:`, as JMH 1.37 writes them, as variants of their own label.
    #[test]
    fn adapter_java_jmh_v1_group_sample() {
        let results = convert_v1(Adapter::JavaJmh, "java/jmh/group_sample.json");
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        assert_eq!(variants(&results), 3);
        for parameters in [
            r#"{"@mode":"sample"}"#,
            r#"{"@label":"get","@mode":"sample"}"#,
            r#"{"@label":"put","@mode":"sample"}"#,
        ] {
            assert!(
                metric(&results, parameters, SampleTime::SLUG_STR, "p99_9999").is_some(),
                "{parameters}"
            );
        }
        assert_units(&results, &[]);
    }

    /// Kills a leaf that drops a perf counter or normalizes its unit, or counts the interval JMH
    /// cannot compute from one sample.
    #[test]
    fn adapter_java_jmh_v1_perfnorm() {
        let results = convert_v1(Adapter::JavaJmh, "java/jmh/perfnorm.json");
        assert!(results.skipped.is_empty(), "{:?}", results.skipped);
        let per_op = [
            "L1-dcache-load-misses",
            "L1-dcache-loads",
            "L1-icache-load-misses",
            "L1-icache-loads",
            "branch-misses",
            "branches",
            "cycles",
            "dTLB-load-misses",
            "dTLB-loads",
            "iTLB-load-misses",
            "iTLB-loads",
            "instructions",
            "stalled-cycles-frontend",
        ]
        .map(|counter| (counter, "#/op"));
        let mut units = vec![("CPI", "clks/insn"), ("IPC", "insns/clk")];
        units.extend(per_op);
        assert_units(&results, &units);
    }

    /// Kills a leaf that reads the measure from the unit when it contradicts the mode.
    #[test]
    fn adapter_java_jmh_v1_unit_of_another_mode() {
        let results = convert_v1(Adapter::JavaJmh, "java/jmh/latency.json");
        assert_eq!(results.inner.len(), 0);
        assert_eq!(results.skipped.benchmarks, 1);
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_java_jmh_v1_ignores_average() {
        for suffix in [
            "java/jmh/modes.json",
            "java/jmh/modes_gc_aux.json",
            "java/jmh/params_gc.json",
            "java/jmh/profilers.json",
            "java/jmh/sample_no_raw.json",
        ] {
            assert_ignores_average(Adapter::JavaJmh, suffix);
        }
    }
}
