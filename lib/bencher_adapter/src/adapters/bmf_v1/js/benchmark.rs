use bencher_json::{BenchmarkName, ParameterSet, project::report::Adapter};
use nom::{
    IResult, Parser as _,
    branch::alt,
    bytes::complete::tag,
    character::complete::{anychar, space1},
    combinator::eof,
    multi::many_till,
    sequence::delimited,
};

use crate::{
    Adaptable, Settings,
    adapters::{
        bmf_v1::{finite::Finite, js::rate::insert_rate, leaf::LeafResults},
        util::{parse_f64, parse_number_as_f64, parse_u64},
    },
    results::adapter_results::AdapterResults,
};

/// Benchmark.js at version 1: each line a benchmark as Benchmark.js prints it,
/// `name x 1,431,759 ops/sec ±0.74% (93 runs sampled)`.
pub struct AdapterJsBenchmarkV1;

impl Adaptable for AdapterJsBenchmarkV1 {
    fn parse(input: &str, _settings: Settings) -> Option<AdapterResults> {
        let mut results = LeafResults::default();
        for line in input.lines() {
            let Some((name, Rate { hz, rme, runs })) = parse_line(line) else {
                continue;
            };
            let Ok(benchmark_name) = name.parse::<BenchmarkName>() else {
                results.skip_benchmark();
                continue;
            };
            let Some(mut variant) = results.new_variant(benchmark_name, ParameterSet::default())
            else {
                continue;
            };
            // Benchmark.js prints the margin of a single run, which is not a number, as 0.
            let rme = (runs > 1).then_some(rme);
            insert_rate(&mut variant, Finite::from(Some(hz)), Finite::from(rme));
        }
        results.finish(Adapter::JsBenchmark)
    }
}

/// A line's name, the shortest that the rest of the line follows as its rate, and its rate.
fn parse_line(line: &str) -> Option<(String, Rate)> {
    let (_, (name, rate)) = many_till(anychar, parse_rate).parse(line).ok()?;
    Some((name.into_iter().collect(), rate))
}

/// The rate of operations per second, its margin as a percentage, and the number of runs.
struct Rate {
    hz: f64,
    rme: f64,
    runs: u64,
}

fn parse_rate(input: &str) -> IResult<&str, Rate> {
    (
        (space1, tag("x"), space1),
        parse_number_as_f64,
        (space1, tag("ops/sec"), space1, tag("\u{b1}")),
        parse_f64,
        (tag("%"), space1),
        delimited(
            tag("("),
            (
                parse_u64,
                space1,
                alt((tag("runs"), tag("run"))),
                space1,
                tag("sampled"),
            ),
            tag(")"),
        ),
        eof,
    )
        .map(|(_, hz, _, rme, _, (runs, ..), _)| Rate { hz, rme, runs })
        .parse(input)
}

#[cfg(test)]
mod test_js_benchmark_v1 {
    use bencher_json::project::{
        measure::built_in::{BuiltInMeasure as _, default::Throughput},
        report::Adapter,
    };
    use pretty_assertions::assert_eq;

    use crate::adapters::bmf_v1::test_util::{assert_ignores_average, assert_variant, convert_v1};

    /// Kills the rate stored as `value` or any name but `mean`, an interval not taken from `rme`
    /// at its level, a name cut at the first ` x `, a single run's margin of 0 kept as an
    /// interval, `(1 run sampled)` refused, and a leaf that records no leaf.
    #[test]
    fn adapter_js_benchmark_v1_suite() {
        let results = convert_v1(Adapter::JsBenchmark, "js/benchmark/suite.txt");
        assert_eq!(results.resolved_adapter, Some(Adapter::JsBenchmark));
        assert!(results.skipped.is_empty());
        assert_eq!(results.inner.len(), 4);
        for (name, expected) in [
            (
                "fib(10)",
                (
                    Throughput::SLUG_STR,
                    vec![
                        ("mean", 3_624_585.0),
                        ("mean_ci95_lower", 3_580_002.604_5),
                        ("mean_ci95_upper", 3_669_167.395_5),
                    ],
                ),
            ),
            (
                "fib(25)",
                (
                    Throughput::SLUG_STR,
                    vec![
                        ("mean", 2_761.0),
                        ("mean_ci95_lower", 2_755.201_9),
                        ("mean_ci95_upper", 2_766.798_1),
                    ],
                ),
            ),
            (
                "RegExp#test x 2",
                (
                    Throughput::SLUG_STR,
                    vec![
                        ("mean", 63_441_315.0),
                        ("mean_ci95_lower", 62_375_500.908),
                        ("mean_ci95_upper", 64_507_129.092),
                    ],
                ),
            ),
            ("slow", (Throughput::SLUG_STR, vec![("mean", 85.37)])),
        ] {
            assert_variant(&results, name, "{}", &[expected]);
        }
    }

    /// Kills a leaf that reads the average setting.
    #[test]
    fn adapter_js_benchmark_v1_ignores_average() {
        for suffix in [
            "js/benchmark/four.txt",
            "js/benchmark/issue_506.txt",
            "js/benchmark/suite.txt",
        ] {
            assert_ignores_average(Adapter::JsBenchmark, suffix);
        }
    }
}
