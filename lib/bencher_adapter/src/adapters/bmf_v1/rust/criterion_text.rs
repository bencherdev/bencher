use bencher_json::{BenchmarkName, ParameterSet, project::report::Adapter};
use nom::{
    IResult, Parser as _,
    branch::alt,
    bytes::complete::{tag, take_till1},
    character::complete::space1,
    combinator::{eof, map, value},
    sequence::delimited,
};

use crate::{
    Adaptable, Settings,
    adapters::{
        bmf_v1::{
            leaf::{LeafResults, Variant},
            rust::criterion_json::measure_of,
        },
        rust::criterion::{parse_criterion_interval, parse_criterion_time},
        util::parse_f64,
    },
    results::adapter_results::AdapterResults,
};

/// criterion's terminal output.
///
/// Each `time:` line is criterion's typical estimate with its interval, followed by
/// the `thrpt:` line under it and the statistics `--verbose` prints. A change
/// against a saved baseline and the outlier counts are local comparisons, so
/// neither is read.
pub struct AdapterRustCriterionText;

impl Adaptable for AdapterRustCriterionText {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let mut results = LeafResults::default();
        let mut benchmark: Option<BenchmarkName> = None;
        let mut prior_line = None;
        for line in input.lines() {
            if let Ok((_, (benchmark_name, interval))) =
                parse_criterion_time(prior_line, line, parse_value)
            {
                let mut variant = results.variant(benchmark_name.clone(), ParameterSet::default());
                insert_interval(&mut variant, "typical", interval);
                benchmark = Some(benchmark_name);
            } else if let Some(benchmark_name) = &benchmark {
                if let Ok((_, interval)) = parse_throughput(line) {
                    let mut variant =
                        results.variant(benchmark_name.clone(), ParameterSet::default());
                    insert_interval(&mut variant, "typical", interval);
                } else if let Ok((_, statistics)) = parse_verbose(line) {
                    let mut variant =
                        results.variant(benchmark_name.clone(), ParameterSet::default());
                    for (statistic, (first, second)) in statistics {
                        insert_bounds(&mut variant, statistic, first, second);
                    }
                }
            }
            prior_line = Some(line);
        }
        results.finish(Adapter::RustCriterionText)
    }
}

fn insert_interval(
    variant: &mut Variant<'_>,
    statistic: &str,
    (first, estimate, second): (Value<'_>, Value<'_>, Value<'_>),
) {
    let Some((measure, estimate)) = measure_of(variant, estimate.1, estimate.0) else {
        return;
    };
    variant.insert(measure, statistic, estimate);
    insert_bounds(variant, statistic, first, second);
}

/// An interval's bounds, under the measure their unit picks; criterion prints
/// some rates from the high bound down, so the lesser is the lower.
fn insert_bounds(variant: &mut Variant<'_>, statistic: &str, first: Value<'_>, second: Value<'_>) {
    let Some((measure, first)) = measure_of(variant, first.1, first.0) else {
        return;
    };
    let Some((second_measure, second)) = measure_of(variant, second.1, second.0) else {
        return;
    };
    if measure != second_measure {
        variant.skip_measure();
        return;
    }
    variant.insert(
        measure.clone(),
        &format!("{statistic}_ci_lower"),
        first.min(second),
    );
    variant.insert(measure, &format!("{statistic}_ci_upper"), first.max(second));
}

/// A number and its unit as criterion prints it.
type Value<'i> = (f64, &'i str);

/// An interval with no estimate, as `--verbose` prints it.
type Bounds<'i> = (Value<'i>, Value<'i>);

fn parse_value(input: &str) -> IResult<&str, Value<'_>> {
    map(
        (
            parse_f64,
            space1,
            alt((
                tag("cpb (decimal)"),
                tag("cycles per bit"),
                take_till1(|c| c == ' ' || c == ']'),
            )),
        ),
        |(number, _, unit)| (number, unit),
    )
    .parse(input)
}

fn parse_throughput(input: &str) -> IResult<&str, (Value<'_>, Value<'_>, Value<'_>)> {
    map(
        (
            (space1, tag("thrpt:"), space1),
            parse_criterion_interval(parse_value),
            eof,
        ),
        |(_, interval, _)| interval,
    )
    .parse(input)
}

/// A `--verbose` line: two statistics, each with only its interval, such as
/// `mean [960.83 ns 971.97 ns] std. dev. [25.454 ns 30.823 ns]`. The slope's
/// R² shares no unit with the slope, so it is not read.
fn parse_verbose(input: &str) -> IResult<&str, Vec<(&'static str, Bounds<'_>)>> {
    map(
        (
            alt((
                value("slope", tag("slope")),
                value("mean", tag("mean")),
                value("median", tag("median")),
            )),
            space1,
            parse_bounds,
            space1,
            alt((
                map(
                    (
                        tag("R^2"),
                        space1,
                        delimited(tag("["), (parse_f64, space1, parse_f64), tag("]")),
                    ),
                    |_| None,
                ),
                map(
                    (
                        alt((
                            value("std_dev", tag("std. dev.")),
                            value("median_abs_dev", tag("med. abs. dev.")),
                        )),
                        space1,
                        parse_bounds,
                    ),
                    |(statistic, _, bounds)| Some((statistic, bounds)),
                ),
            )),
            eof,
        ),
        |(statistic, _, bounds, _, other, _)| {
            std::iter::once((statistic, bounds)).chain(other).collect()
        },
    )
    .parse(input)
}

fn parse_bounds(input: &str) -> IResult<&str, Bounds<'_>> {
    map(
        delimited(tag("["), (parse_value, space1, parse_value), tag("]")),
        |(first, _, second)| (first, second),
    )
    .parse(input)
}

#[cfg(test)]
mod test_rust_criterion_text {
    use bencher_json::project::{
        measure::built_in::{
            BuiltInMeasure as _,
            default::{BytesThroughput, Cycles, Latency, Throughput},
        },
        report::Adapter,
    };
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, Settings,
        adapters::bmf_v1::test_util::{
            assert_ignores_average, assert_units, assert_variant, convert_v1, typical,
        },
    };

    use super::AdapterRustCriterionText;

    /// Kills a leaf that reads `time:` as anything but the typical estimate, drops
    /// `thrpt:`, folds bytes and elements into one measure, scales a binary,
    /// decimal, or bit rate wrong, or records no leaf.
    #[test]
    fn adapter_rust_criterion_text_throughput() {
        for adapter in [Adapter::RustCriterion, Adapter::RustCriterionText] {
            let results = convert_v1(adapter, "rust/criterion/throughput.txt");
            assert_eq!(
                results.resolved_adapter,
                Some(Adapter::RustCriterionText),
                "{adapter}"
            );
            assert_eq!(results.inner.len(), 18, "{adapter}");
            assert_variant(
                &results,
                "fib 15",
                "{}",
                &[typical(Latency::SLUG_STR, 972.23, 979.82, 988.69)],
            );
            assert_variant(
                &results,
                "checksum/fold/1024",
                "{}",
                &[
                    typical(Latency::SLUG_STR, 44.124, 45.5, 47.693),
                    typical(
                        BytesThroughput::SLUG_STR,
                        19.996 * GIB,
                        20.960 * GIB,
                        21.614 * GIB,
                    ),
                ],
            );
            assert_variant(
                &results,
                "checksum/sort/elements",
                "{}",
                &[
                    typical(Latency::SLUG_STR, 1_028.5, 1_035.1, 1_044.0),
                    typical(Throughput::SLUG_STR, 3.9233e9, 3.9571e9, 3.9827e9),
                ],
            );
            assert_variant(
                &results,
                "checksum/decimal",
                "{}",
                &[
                    typical(Latency::SLUG_STR, 50.461, 51.48, 52.459),
                    typical(BytesThroughput::SLUG_STR, 19.063e9, 19.425e9, 19.817e9),
                ],
            );
            assert_variant(
                &results,
                "checksum/bits",
                "{}",
                &[
                    typical(Latency::SLUG_STR, 49.149, 49.751, 50.316),
                    typical(
                        BytesThroughput::SLUG_STR,
                        158.99e9 / 8.0,
                        160.80e9 / 8.0,
                        162.77e9 / 8.0,
                    ),
                ],
            );
            assert_variant(
                &results,
                "slow/fib 30",
                "{}",
                &[typical(Latency::SLUG_STR, 1.356e6, 1.3902e6, 1.4592e6)],
            );
        }
    }

    /// Every rate unit criterion prints, each from its own benchmark, so each unit's
    /// measure and scale is checked.
    ///
    /// Kills a leaf that sends a rate of elements, bytes, decimal bytes, or bits at any
    /// prefix to the wrong measure, or scales a prefix as binary that is decimal or the
    /// other way around.
    #[test]
    fn adapter_rust_criterion_text_rates() {
        let results = convert_v1(Adapter::RustCriterionText, "rust/criterion/throughput.txt");
        for (benchmark, latency, measure, rates) in [
            (
                "rate/elements",
                [2.1026, 2.2222, 2.3055].map(|time| time * 1e6),
                Throughput::SLUG_STR,
                [433.75, 450.01, 475.6],
            ),
            (
                "rate/kilo_elements",
                [1.8908, 1.9259, 1.9694].map(|time| time * 1e6),
                Throughput::SLUG_STR,
                [50.776, 51.923, 52.887].map(|rate| rate * KILO),
            ),
            (
                "rate/mega_elements",
                [1.9097, 1.9315, 1.964].map(|time| time * 1e3),
                Throughput::SLUG_STR,
                [50.916, 51.773, 52.365].map(|rate| rate * MEGA),
            ),
            (
                "rate/bytes",
                [1.8735, 1.9075, 1.9325].map(|time| time * 1e6),
                BytesThroughput::SLUG_STR,
                [517.47, 524.25, 533.76],
            ),
            (
                "rate/kibibytes",
                [1.9396, 2.0692, 2.1685].map(|time| time * 1e6),
                BytesThroughput::SLUG_STR,
                [450.35, 471.96, 503.49].map(|rate| rate * KIB),
            ),
            (
                "rate/mebibytes",
                [1.8892, 1.9451, 2.0166].map(|time| time * 1e3),
                BytesThroughput::SLUG_STR,
                [47.291, 49.029, 50.482].map(|rate| rate * MIB),
            ),
            (
                "rate/kilobytes",
                [1.9595, 1.9919, 2.0464].map(|time| time * 1e6),
                BytesThroughput::SLUG_STR,
                [488.65, 502.02, 510.33].map(|rate| rate * KILO),
            ),
            (
                "rate/megabytes",
                [1.959, 2.0318, 2.1554].map(|time| time * 1e3),
                BytesThroughput::SLUG_STR,
                [46.395, 49.217, 51.045].map(|rate| rate * MEGA),
            ),
            (
                "rate/bits",
                [1.9501, 2.0338, 2.1201].map(|time| time * 1e6),
                BytesThroughput::SLUG_STR,
                [471.67, 491.68, 512.79].map(|rate| rate / 8.0),
            ),
            (
                "rate/kilobits",
                [2.1792, 2.3324, 2.4925].map(|time| time * 1e6),
                BytesThroughput::SLUG_STR,
                [40.12, 42.875, 45.889].map(|rate| rate * KILO / 8.0),
            ),
            (
                "rate/megabits",
                [1.9783, 2.0608, 2.1394].map(|time| time * 1e3),
                BytesThroughput::SLUG_STR,
                [46.742, 48.525, 50.549].map(|rate| rate * MEGA / 8.0),
            ),
        ] {
            let [time_lower, time, time_upper] = latency;
            let [rate_lower, rate, rate_upper] = rates;
            assert_variant(
                &results,
                benchmark,
                "{}",
                &[
                    typical(Latency::SLUG_STR, time_lower, time, time_upper),
                    typical(measure, rate_lower, rate, rate_upper),
                ],
            );
        }
    }

    /// Kills a leaf that ingests a change against the baseline, or lets the
    /// change block's `time:` and `thrpt:` lines overwrite the benchmark's own.
    #[test]
    fn adapter_rust_criterion_text_change() {
        let results = convert_v1(Adapter::RustCriterionText, "rust/criterion/change.txt");
        assert_eq!(results.inner.len(), 7);
        assert_variant(
            &results,
            "fib 15",
            "{}",
            &[typical(Latency::SLUG_STR, 986.31, 992.52, 999.29)],
        );
        assert_variant(
            &results,
            "checksum/fold/1024",
            "{}",
            &[
                typical(Latency::SLUG_STR, 43.095, 43.352, 43.644),
                typical(
                    BytesThroughput::SLUG_STR,
                    21.851 * GIB,
                    21.998 * GIB,
                    22.130 * GIB,
                ),
            ],
        );
    }

    /// Kills a leaf that drops a statistic `--verbose` prints, names one inexactly,
    /// reads R² as a time, or puts a level in an interval's name.
    #[test]
    fn adapter_rust_criterion_text_verbose() {
        let results = convert_v1(Adapter::RustCriterionText, "rust/criterion/verbose.txt");
        assert_eq!(results.inner.len(), 7);
        assert_variant(
            &results,
            "fib 15",
            "{}",
            &[(
                Latency::SLUG_STR,
                vec![
                    ("typical", 1_071.6),
                    ("typical_ci_lower", 1_047.6),
                    ("typical_ci_upper", 1_098.5),
                    ("slope_ci_lower", 1_047.6),
                    ("slope_ci_upper", 1_098.5),
                    ("mean_ci_lower", 1_105.8),
                    ("mean_ci_upper", 1_135.9),
                    ("std_dev_ci_lower", 63.439),
                    ("std_dev_ci_upper", 88.528),
                    ("median_ci_lower", 1_131.5),
                    ("median_ci_upper", 1_148.5),
                    ("median_abs_dev_ci_lower", 20.023),
                    ("median_abs_dev_ci_upper", 45.178),
                ],
            )],
        );
        assert_variant(
            &results,
            "checksum/fold/1024",
            "{}",
            &[
                (
                    Latency::SLUG_STR,
                    vec![
                        ("typical", 43.933),
                        ("typical_ci_lower", 43.38),
                        ("typical_ci_upper", 44.518),
                        ("slope_ci_lower", 43.38),
                        ("slope_ci_upper", 44.518),
                        ("mean_ci_lower", 43.011),
                        ("mean_ci_upper", 43.632),
                        ("std_dev_ci_lower", 1.073),
                        ("std_dev_ci_upper", 2.0231),
                        ("median_ci_lower", 42.579),
                        ("median_ci_upper", 42.711),
                        ("median_abs_dev_ci_lower", 0.165_21),
                        ("median_abs_dev_ci_upper", 0.354_25),
                    ],
                ),
                typical(
                    BytesThroughput::SLUG_STR,
                    21.422 * GIB,
                    21.707 * GIB,
                    21.984 * GIB,
                ),
            ],
        );
    }

    const KILO: f64 = 1e3;
    const MEGA: f64 = 1e6;
    const KIB: f64 = 1024.0;
    const MIB: f64 = 1024.0 * 1024.0;
    const GIB: f64 = 1024.0 * 1024.0 * 1024.0;

    /// Kills a leaf that reads a cycle count as a time, refuses a rate in a unit it
    /// does not know rather than keeping it under the unit's own name, or creates that
    /// measure in the generic unit.
    #[test]
    fn adapter_rust_criterion_text_cycles_per_byte() {
        let results = convert_v1(
            Adapter::RustCriterionText,
            "rust/criterion/cycles_per_byte.txt",
        );
        assert_eq!(results.inner.len(), 2);
        assert_variant(
            &results,
            "fib 15",
            "{}",
            &[typical(
                Cycles::SLUG_STR,
                3_964.871_1,
                3_987.403_4,
                4_012.827_8,
            )],
        );
        assert_variant(
            &results,
            "checksum/fold",
            "{}",
            &[
                typical(Cycles::SLUG_STR, 822.886_5, 839.048_2, 858.028_1),
                typical("cpb", 0.2009, 0.2048, 0.2095),
            ],
        );
        assert_units(&results, &[("cpb", "cpb")]);
    }

    /// criterion-perf-events labels every count `cycles`, whatever the event.
    #[test]
    fn adapter_rust_criterion_text_perf_events() {
        let results = convert_v1(Adapter::RustCriterionText, "rust/criterion/perf_events.txt");
        assert_eq!(results.inner.len(), 2);
        assert_variant(
            &results,
            "fib 15",
            "{}",
            &[typical(
                Cycles::SLUG_STR,
                19_980.222,
                19_980.298_7,
                19_980.380_3,
            )],
        );
        assert_variant(
            &results,
            "fib 15 branches",
            "{}",
            &[typical(
                Cycles::SLUG_STR,
                3_948.991_6,
                3_948.997_1,
                3_949.002_4,
            )],
        );
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_rust_criterion_text_ignores_average() {
        for suffix in [
            "rust/criterion/throughput.txt",
            "rust/criterion/verbose.txt",
            "rust/criterion/cycles_per_byte.txt",
        ] {
            assert_ignores_average(Adapter::RustCriterionText, suffix);
        }
    }

    /// Issue 376: a mebibyte rate is a bytes throughput, in bytes per second.
    #[test]
    fn adapter_rust_criterion_text_issue_376() {
        let results = AdapterRustCriterionText::parse(
            "parse/json              time:   [5.9905 ms 6.0185 ms 6.0629 ms]\n                        thrpt:  [164.95 MiB/s 166.14 MiB/s 167.10 MiB/s]\n",
            Settings::default(),
        )
        .expect("Failed to parse");
        let mib = 1024.0 * 1024.0;
        assert_variant(
            &results,
            "parse/json",
            "{}",
            &[
                typical(Latency::SLUG_STR, 5.9905e6, 6.0185e6, 6.0629e6),
                typical(
                    BytesThroughput::SLUG_STR,
                    164.95 * mib,
                    166.14 * mib,
                    167.10 * mib,
                ),
            ],
        );
    }
}
