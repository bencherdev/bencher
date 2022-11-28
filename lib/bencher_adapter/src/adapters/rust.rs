use std::{collections::HashMap, str::FromStr, time::Duration};

use bencher_json::{project::metric_kind::LATENCY_SLUG_STR, JsonMetric};
use literally::hmap;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_until1},
    character::complete::{anychar, digit1, line_ending, space1},
    combinator::{eof, map, map_res, peek, success},
    multi::{fold_many1, many0, many1, many_m_n, many_till},
    sequence::{delimited, tuple},
    IResult,
};
use ordered_float::OrderedFloat;

use crate::{
    results::{adapter_metrics::AdapterMetrics, adapter_results::AdapterResults},
    Adapter, AdapterError, Settings,
};

use super::print_ln;

pub struct AdapterRust;

impl Adapter for AdapterRust {
    fn parse(input: &str, settings: Settings) -> Result<AdapterResults, AdapterError> {
        parse_rust(input, settings)
            .map(|(_, benchmarks)| benchmarks)
            .map_err(|err| AdapterError::Nom(err.map_input(Into::into)))
    }
}

#[derive(Debug, PartialEq, Eq)]
enum Test {
    Ignored,
    Failed,
    Bench(JsonMetric),
    Ok(JsonMetric),
}

pub fn parse_rust(input: &str, settings: Settings) -> IResult<&str, AdapterResults> {
    tuple((
        fold_many1(
            |i| parse_running(i, settings),
            HashMap::new,
            |benchmarks, new_benchmarks| benchmarks.into_iter().chain(new_benchmarks).collect(),
        ),
        eof,
    ))(input)
    .map(|(remainder, (benchmarks, _))| {
        debug_assert!(remainder.is_empty(), "{remainder}");
        (remainder, benchmarks.into())
    })
}

fn parse_running(
    input: &str,
    settings: Settings,
) -> IResult<&str, HashMap<String, AdapterMetrics>> {
    map_res(
        tuple((
            tuple((
                many0(line_ending),
                alt((
                    // A non-initial multi-target run
                    parse_multi_mod,
                    // The start of a run
                    map(success(""), |_| ()),
                )),
                many0(line_ending),
            )),
            alt((
                |input| parse_cargo(input, settings),
                |input| parse_criterion(input, settings),
            )),
        )),
        |(_, benchmarks)| -> Result<_, AdapterError> {
            let mut results = HashMap::new();
            for benchmark in benchmarks {
                if let Some((benchmark_name, metric)) = benchmark? {
                    results.insert(
                        benchmark_name,
                        AdapterMetrics {
                            inner: hmap! {
                                LATENCY_SLUG_STR => metric
                            },
                        },
                    );
                }
            }
            Ok(results)
        },
    )(input)
}

fn parse_cargo(
    input: &str,
    settings: Settings,
) -> IResult<&str, Vec<Result<Option<(String, JsonMetric)>, AdapterError>>> {
    map(
        tuple((
            parse_running_x_tests,
            many0(|input| parse_cargo_bench(input, settings)),
            tuple((
                many0(line_ending),
                parse_test_failures_and_result,
                many0(line_ending),
            )),
        )),
        |(_, benchmarks, _)| benchmarks,
    )(input)
}

fn parse_criterion(
    input: &str,
    settings: Settings,
) -> IResult<&str, Vec<Result<Option<(String, JsonMetric)>, AdapterError>>> {
    map(
        many1(tuple((
            parse_criterion_benchmarking_file,
            many0(line_ending),
            |input| parse_criterion_bench(input),
            parse_criterion_change,
            many0(line_ending),
        ))),
        |benchmarks| {
            benchmarks
                .into_iter()
                .map(|(_, _, benchmarks, _, _)| benchmarks)
                .collect()
        },
    )(input)
}

fn parse_criterion_benchmarking_file(input: &str) -> IResult<&str, ()> {
    map(
        many_m_n(
            4,
            4,
            tuple((tag("Benchmarking"), many_till(anychar, line_ending))),
        ),
        |_| (),
    )(input)
}

fn parse_criterion_bench(
    input: &str,
) -> IResult<&str, Result<Option<(String, JsonMetric)>, AdapterError>> {
    map(
        tuple((
            take_until1(" "),
            tuple((space1, tag("time:"), space1)),
            parse_criterion_metric,
            line_ending,
        )),
        |(key, _, metric, _)| Ok(Some((key.into(), metric))),
    )(input)
}

fn parse_criterion_metric(input: &str) -> IResult<&str, JsonMetric> {
    map(
        delimited(
            tag("["),
            tuple((
                parse_criterion_duration,
                space1,
                parse_criterion_duration,
                space1,
                parse_criterion_duration,
            )),
            tag("]"),
        ),
        |(lower_bound, _, value, _, upper_bound)| JsonMetric {
            value,
            lower_bound: Some(lower_bound),
            upper_bound: Some(upper_bound),
        },
    )(input)
}

fn parse_criterion_duration(input: &str) -> IResult<&str, OrderedFloat<f64>> {
    map(
        tuple((parse_float, space1, parse_units)),
        |(duration, _, units)| (to_f64(duration) * units.as_nanos()).into(),
    )(input)
}

fn parse_criterion_change(input: &str) -> IResult<&str, ()> {
    map(
        tuple((
            space1,
            tag("change:"),
            many_till(anychar, line_ending),
            space1,
            alt((tag("No change"), tag("Performance has"))),
            many_till(anychar, line_ending),
            tag("Found"),
            many_till(anychar, line_ending),
            many1(tuple((space1, digit1, many_till(anychar, line_ending)))),
        )),
        |_| (),
    )(input)
}

// Doc-tests ...
// Running ...
fn parse_multi_mod(input: &str) -> IResult<&str, ()> {
    map(
        tuple((
            space1,
            alt((tag("Running"), tag("Doc-tests"))),
            many_till(anychar, line_ending),
        )),
        |_| (),
    )(input)
}

// running X test(s)
fn parse_running_x_tests(input: &str) -> IResult<&str, ()> {
    map(
        tuple((
            tag("running"),
            space1,
            digit1,
            space1,
            alt((tag("tests"), tag("test"))),
            line_ending,
        )),
        |_| (),
    )(input)
}

// test rust::mod::path::to_test ... ignored/Y ns/iter (+/- Z)
fn parse_cargo_bench(
    input: &str,
    settings: Settings,
) -> IResult<&str, Result<Option<(String, JsonMetric)>, AdapterError>> {
    map(
        tuple((|input| parse_test(input), success(""))),
        |((key, test), _)| match test {
            Test::Ignored => Ok(None),
            Test::Failed => {
                if settings.allow_failure {
                    Ok(None)
                } else {
                    Err(AdapterError::BenchmarkFailed(key.into()))
                }
            },
            Test::Ok(metric) | Test::Bench(metric) => Ok(Some((key.into(), metric))),
        },
    )(input)
}

fn parse_test(input: &str) -> IResult<&str, (String, Test)> {
    map(
        tuple((
            tag("test"),
            space1,
            take_until1(" "),
            space1,
            tag("..."),
            space1,
            alt((
                map(tag("ignored"), |_| Test::Ignored),
                map(
                    tuple((
                        tag("FAILED"),
                        // Strip trailing report time
                        many_till(anychar, peek(line_ending)),
                    )),
                    |_| Test::Failed,
                ),
                map(parse_bench, Test::Bench),
                map(parse_ok, Test::Ok),
            )),
            line_ending,
        )),
        |(_, _, key, _, _, _, test, _)| (key.into(), test),
    )(input)
}

fn parse_test_failures_and_result(input: &str) -> IResult<&str, ()> {
    alt((
        map(
            tuple((
                parse_test_failures,
                many0(line_ending),
                parse_test_result,
                many0(line_ending),
                tuple((tag("error:"), many_till(anychar, alt((line_ending, eof))))),
            )),
            |_| (),
        ),
        parse_test_result,
    ))(input)
}

fn parse_test_failures(input: &str) -> IResult<&str, ()> {
    map(
        tuple((
            tag("failures:"),
            many_till(anychar, line_ending),
            line_ending,
            many_m_n(3, 4, tag("-")),
            many_till(anychar, line_ending),
            tag("thread"),
            many_till(anychar, line_ending),
            tag("note:"),
            many_till(anychar, line_ending),
            line_ending,
            line_ending,
            tag("failures:"),
            line_ending,
            many1(tuple((space1, many_till(anychar, line_ending)))),
        )),
        |_| (),
    )(input)
}

fn parse_test_result(input: &str) -> IResult<&str, ()> {
    map(
        tuple((tag("test result:"), many_till(anychar, line_ending))),
        |_| (),
    )(input)
}

pub enum Units {
    Pico,
    Nano,
    Micro,
    Milli,
    Sec,
}

fn parse_units(input: &str) -> IResult<&str, Units> {
    alt((
        map(tag("ps"), |_| Units::Pico),
        map(tag("ns"), |_| Units::Nano),
        map(tag("μs"), |_| Units::Micro),
        map(tag("µs"), |_| Units::Micro),
        map(tag("ms"), |_| Units::Milli),
        map(tag("s"), |_| Units::Sec),
    ))(input)
}

impl Units {
    fn as_nanos(&self) -> f64 {
        match self {
            Self::Pico => 1.0 / 1_000.0,
            Self::Nano => 1.0,
            Self::Micro => 1_000.0,
            Self::Milli => 1_000_000.0,
            Self::Sec => 1_000_000_000.0,
        }
    }
}

// cargo bench
// TODO cargo test -- -Z unstable-options --format json
fn parse_bench(input: &str) -> IResult<&str, JsonMetric> {
    map(
        tuple((
            tag("bench:"),
            space1,
            parse_int,
            space1,
            parse_units,
            tag("/iter"),
            space1,
            delimited(tag("("), tuple((tag("+/-"), space1, parse_int)), tag(")")),
        )),
        |(_, _, duration, _, units, _, _, (_, _, variance))| {
            let value = (to_duration(to_u64(duration), &units).as_nanos() as f64).into();
            let variance = Some((to_duration(to_u64(variance), &units).as_nanos() as f64).into());
            JsonMetric {
                value,
                lower_bound: variance,
                upper_bound: variance,
            }
        },
    )(input)
}

fn parse_int(input: &str) -> IResult<&str, Vec<(&str, &str)>> {
    many1(tuple((digit1, alt((tag(","), success(" "))))))(input)
}

fn to_u64(input: Vec<(&str, &str)>) -> u64 {
    let mut number = String::new();
    for (digit, _) in input {
        number.push_str(digit);
    }
    u64::from_str(&number).unwrap()
}

fn to_duration(time: u64, units: &Units) -> Duration {
    match units {
        Units::Pico => Duration::from_nanos((time as f64 / units.as_nanos()) as u64),
        Units::Nano => Duration::from_nanos(time),
        Units::Micro => Duration::from_micros(time),
        Units::Milli => Duration::from_millis(time),
        Units::Sec => Duration::from_secs(time),
    }
}

// cargo test -- -Z unstable-options --report-time
// TODO cargo test -- -Z unstable-options --format json --report-time
fn parse_ok(input: &str) -> IResult<&str, JsonMetric> {
    map(
        tuple((
            tag("ok"),
            space1,
            delimited(tag("<"), tuple((parse_float, parse_units)), tag(">")),
        )),
        |(_, _, (duration, units))| {
            let value = to_f64(duration) * units.as_nanos() as f64;
            JsonMetric {
                value: value.into(),
                lower_bound: None,
                upper_bound: None,
            }
        },
    )(input)
}

fn parse_float(input: &str) -> IResult<&str, Vec<&str>> {
    fold_many1(
        alt((digit1, tag("."), tag(","))),
        Vec::new,
        |mut float_chars, float_char| {
            if float_char == "," {
                float_chars
            } else {
                float_chars.push(float_char);
                float_chars
            }
        },
    )(input)
}

fn to_f64(input: Vec<&str>) -> f64 {
    let mut number = String::new();
    for floating_point in input {
        number.push_str(floating_point);
    }
    f64::from_str(&number).unwrap()
}

#[cfg(test)]
pub(crate) mod test_rust {
    use bencher_json::JsonMetric;
    use nom::IResult;
    use pretty_assertions::assert_eq;

    use super::{
        parse_bench, parse_criterion_benchmarking_file, parse_criterion_change,
        parse_criterion_metric, parse_multi_mod, parse_running_x_tests, parse_test,
        parse_test_failures, parse_test_result, AdapterRust, Test,
    };
    use crate::{
        adapters::test_util::{convert_file_path, validate_metrics},
        results::adapter_results::AdapterResults,
        Adapter, Settings,
    };

    const UNIT_RESULT: IResult<&str, ()> = Ok(("", ()));

    fn convert_rust_bench(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/rust/cargo_bench_{}.txt", suffix);
        convert_file_path::<AdapterRust>(&file_path, Settings::default())
    }

    fn convert_rust_test(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/rust/cargo_test_{}.txt", suffix);
        convert_file_path::<AdapterRust>(&file_path, Settings::default())
    }

    fn validate_bench_metrics(results: &AdapterResults, key: &str) {
        let metrics = results.inner.get(key).unwrap();
        validate_metrics(metrics, 3_161.0, Some(975.0), Some(975.0));
    }

    fn parse_assert<T>(
        parse_fn: &impl Fn(&str) -> IResult<&str, T>,
        input: &str,
        expected: IResult<&str, T>,
        context: impl std::fmt::Display,
    ) where
        T: std::fmt::Debug + PartialEq + Eq,
    {
        assert_eq!(expected, parse_fn(input), "{context}");
    }

    #[test]
    fn test_parse_multi_mod() {
        for (index, input) in [
            "        Running benches/name.rs (/oath/to/target/release/deps/name-hash)\n",
            "     Running unittests (target/release/deps/log4rs_bench-f36c88332bd25d23)\n",
            "   Doc-tests bencher_adapter\n",
        ]
        .iter()
        .enumerate()
        {
            assert_eq!(UNIT_RESULT, parse_multi_mod(input), "#{index}: {input}")
        }

        for (index, input) in [
            "",
            "Running benches/name.rs (/oath/to/target/release/deps/name-hash)\n",
            "Doc-tests bencher_adapter\n",
            "   Doc-tests bencher_adapter",
            "prefix   Doc-tests bencher_adapter",
            "   prefix Doc-tests bencher_adapter",
        ]
        .iter()
        .enumerate()
        {
            assert_eq!(true, parse_multi_mod(input).is_err(), "#{index}: {input}")
        }
    }

    #[test]
    fn test_parse_running_x_tests() {
        for (index, input) in ["running 0 tests\n", "running 1 test\n", "running 2 tests\n"]
            .iter()
            .enumerate()
        {
            assert_eq!(
                UNIT_RESULT,
                parse_running_x_tests(input),
                "#{index}: {input}"
            )
        }

        for (index, input) in [
            "",
            "running 0 tests",
            " running 0 tests\n",
            "prefix running 0 tests\n",
            "Courage the Cowardly Dog\nrunning 0 tests\n",
        ]
        .iter()
        .enumerate()
        {
            assert_eq!(
                true,
                parse_running_x_tests(input).is_err(),
                "#{index}: {input}"
            )
        }
    }

    #[test]
    fn test_parse_test() {
        for (index, (expected, input)) in [
            (
                Ok(("", ("tests::is_ignored".into(), Test::Ignored))),
                "test tests::is_ignored ... ignored\n",
            ),
            (
                Ok(("", ("tests::is_failed".into(), Test::Failed))),
                "test tests::is_failed ... FAILED\n",
            ),
            (
                Ok((
                    "",
                    (
                        "tests::is_unit_test".into(),
                        Test::Ok(JsonMetric {
                            value: 1_000_000_000.0.into(),
                            lower_bound: None,
                            upper_bound: None,
                        }),
                    ),
                )),
                "test tests::is_unit_test ... ok <1.000s>\n",
            ),
            (
                Ok((
                    "",
                    (
                        "tests::is_bench_test".into(),
                        Test::Bench(JsonMetric {
                            value: 5_280.0.into(),
                            lower_bound: Some(333.0.into()),
                            upper_bound: Some(333.0.into()),
                        }),
                    ),
                )),
                "test tests::is_bench_test ... bench:             5,280 ns/iter (+/- 333)\n",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(expected, parse_test(input), "#{index}: {input}")
        }

        for (index, input) in [
            "",
            "tests::is_ignored",
            "test tests::is_ignored ... ignored",
            " test tests::is_ignored ... ignored\n",
            "prefix test tests::is_ignored ... ignored\n",
        ]
        .iter()
        .enumerate()
        {
            assert_eq!(true, parse_test(input).is_err(), "#{index}: {input}")
        }
    }

    #[test]
    fn test_parse_test_result() {
        for (index, input) in [
            "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n",
        ]
            .iter()
            .enumerate()
        {
            assert_eq!(UNIT_RESULT, parse_test_result(input), "#{index}: {input}")
        }

        for (index, input) in [
            "",
            "test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s",
            " test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n",
            "prefix test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s\n"
        ]
        .iter()
        .enumerate()
        {
            assert_eq!(true, parse_test_result(input).is_err(), "#{index}: {input}")
        }
    }

    #[test]
    fn test_parse_test_failures() {
        for (index, input) in [
            "failures:\n\n--- tests::benchmark_failure stdout ----\nthread 'main' panicked at 'explicit panic', src/tests.rs:28:9\nnote: run with `RUST_BACKTRACE=1` environment variable to display a backtrace\n\n\nfailures:\n     tests::benchmark_failure\n",
        ]
            .iter()
            .enumerate()
        {
            assert_eq!(UNIT_RESULT, parse_test_failures(input), "#{index}: {input}")
        }

        for (index, input) in [
            "",
            "failures:\n\n--- tests::benchmark_failure stdout ----\nthread 'main' panicked at 'explicit panic', src/tests.rs:28:9\nnote: run with `RUST_BACKTRACE=1` environment variable to display a backtrace\n\n\nfailures:\ntests::benchmark_failure\n",
        ]
        .iter()
        .enumerate()
        {
            assert_eq!(true, parse_test_failures(input).is_err(), "#{index}: {input}")
        }
    }

    #[test]
    fn test_parse_criterion_benchmarking_file() {
        for (index, input) in [
            "Benchmarking crit_test\nBenchmarking crit_test: Warming up for 3.0000 s\nBenchmarking crit_test: Collecting 100 samples in estimated 5.0000 s (15B iterations)\nBenchmarking crit_test: Analyzing\n",
        ]
            .iter()
            .enumerate()
        {
            assert_eq!(UNIT_RESULT, parse_criterion_benchmarking_file(input), "#{index}: {input}")
        }

        for (index, input) in [
            "",
            "Benchmarking crit_test\nBenchmarking crit_test: Warming up for 3.0000 s\nBenchmarking crit_test: Collecting 100 samples in estimated 5.0000 s (15B iterations)\nBenchmarking crit_test: Analyzing",
            " Benchmarking crit_test\nBenchmarking crit_test: Warming up for 3.0000 s\nBenchmarking crit_test: Collecting 100 samples in estimated 5.0000 s (15B iterations)\nBenchmarking crit_test: Analyzing\n",
            "prefix Benchmarking crit_test\nBenchmarking crit_test: Warming up for 3.0000 s\nBenchmarking crit_test: Collecting 100 samples in estimated 5.0000 s (15B iterations)\nBenchmarking crit_test: Analyzing\n",
        ]
        .iter()
        .enumerate()
        {
            assert_eq!(true, parse_criterion_benchmarking_file(input).is_err(), "#{index}: {input}")
        }
    }

    #[test]
    fn test_parse_bench() {
        for (index, (expected, input)) in [
            (
                Ok((
                    "",
                    JsonMetric {
                        value: 3_161.0.into(),
                        lower_bound: Some(975.0.into()),
                        upper_bound: Some(975.0.into()),
                    },
                )),
                "bench:             3,161 ns/iter (+/- 975)",
            ),
            (
                Ok((
                    "",
                    JsonMetric {
                        value: 1_000.0.into(),
                        lower_bound: Some(1_000.0.into()),
                        upper_bound: Some(1_000.0.into()),
                    },
                )),
                "bench:                 1 μs/iter (+/- 1)",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(expected, parse_bench(input), "#{index}: {input}")
        }
    }

    #[test]
    fn test_parse_criterion_metric() {
        for (index, (expected, input)) in [
            (
                Ok((
                    "",
                    JsonMetric {
                        value: 280.0.into(),
                        lower_bound: Some(222.2.into()),
                        upper_bound: Some(333.33.into()),
                    },
                )),
                "[222.2 ns 280.0 ns 333.33 ns]",
            ),
            (
                Ok((
                    "",
                    JsonMetric {
                        value: 5.280.into(),
                        lower_bound: Some(0.222.into()),
                        upper_bound: Some(0.33333.into()),
                    },
                )),
                "[222.0 ps 5,280.0 ps 333.33 ps]",
            ),
            (
                Ok((
                    "",
                    JsonMetric {
                        value: 18_019.0.into(),
                        lower_bound: Some(16_652.0.into()),
                        upper_bound: Some(19_562.0.into()),
                    },
                )),
                "[16.652 µs 18.019 µs 19.562 µs]",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(expected, parse_criterion_metric(input), "#{index}: {input}")
        }
    }

    #[test]
    fn test_parse_criterion_change() {
        for (index, input) in [
            "                        change: [-2.0565% -0.2521% +1.6377%] (p = 0.79 > 0.05)\n                        No change in performance detected.\nFound 8 outliers among 100 measurements (8.00%)\n  6 (6.00%) high mild\n  2 (2.00%) high severe\n",
            "                        change: [+11.193% +20.965% +31.814%] (p = 0.00 < 0.05)\n                        Performance has regressed.\nFound 11 outliers among 100 measurements (11.00%)\n  6 (6.00%) high mild\n  5 (5.00%) high severe\n",
        ]
        .iter()
        .enumerate()
        {
            assert_eq!(
                UNIT_RESULT,
                parse_criterion_change(input),
                "#{index}: {input}"
            )
        }
    }

    #[test]
    fn test_adapter_rust_zero() {
        let results = convert_rust_bench("zero");
        assert_eq!(results.inner.len(), 0);
    }

    #[test]
    fn test_adapter_rust_one() {
        let results = convert_rust_bench("one");
        assert_eq!(results.inner.len(), 1);
        validate_bench_metrics(&results, "tests::benchmark");
    }

    #[test]
    fn test_adapter_rust_ignore() {
        let results = convert_rust_bench("ignore");
        assert_eq!(results.inner.len(), 1);
        validate_bench_metrics(&results, "tests::benchmark");
    }

    #[test]
    fn test_adapter_rust_many() {
        let results = convert_rust_bench("many");
        validate_adapter_rust_many(results);
    }

    pub fn validate_adapter_rust_many(results: AdapterResults) {
        assert_eq!(results.inner.len(), 6);
        validate_bench_metrics(&results, "tests::benchmark");
        validate_bench_metrics(&results, "tests::other_benchmark");
        validate_bench_metrics(&results, "tests::last_benchmark");

        let number = 1_000.0;
        let metrics = results.inner.get("tests::one_digit").unwrap();
        validate_metrics(metrics, number, Some(number), Some(number));

        let number = 22_000_000.0;
        let metrics = results.inner.get("tests::two_digit").unwrap();
        validate_metrics(metrics, number, Some(number), Some(number));

        let number = 333_000_000_000.0;
        let metrics = results.inner.get("tests::three_digit").unwrap();
        validate_metrics(metrics, number, Some(number), Some(number));
    }

    #[test]
    fn test_adapter_rust_multi_target() {
        let results = convert_rust_bench("multi_target");
        assert_eq!(results.inner.len(), 2);
        validate_bench_metrics(&results, "tests::benchmark");
        validate_bench_metrics(&results, "tests::other_benchmark");
    }

    #[test]
    fn test_adapter_rust_failed() {
        let contents =
            std::fs::read_to_string("./tool_output/rust/cargo_bench_failed.txt").unwrap();
        assert!(AdapterRust::parse(&contents, Settings::default()).is_err());
    }

    #[test]
    fn test_adapter_rust_failed_allow_failure() {
        let contents =
            std::fs::read_to_string("./tool_output/rust/cargo_bench_failed.txt").unwrap();
        let results = AdapterRust::parse(
            &contents,
            Settings {
                allow_failure: true,
            },
        )
        .unwrap();
        assert_eq!(results.inner.len(), 2);

        let metrics = results.inner.get("tests::benchmark_a").unwrap();
        validate_metrics(metrics, 3_296.0, Some(521.0), Some(521.0));

        let metrics = results.inner.get("tests::benchmark_c").unwrap();
        validate_metrics(metrics, 3_215.0, Some(356.0), Some(356.0));
    }

    #[test]
    fn test_adapter_rust_criterion() {
        let results = convert_rust_bench("criterion");
        println!("{results:#?}");
        assert_eq!(results.inner.len(), 4);
    }

    #[test]
    fn test_adapter_rust_test_report_time() {
        let results = convert_rust_test("report_time");
        assert_eq!(results.inner.len(), 3);

        let metrics = results.inner.get("tests::unit_test").unwrap();
        validate_metrics(metrics, 0.0, None, None);

        let metrics = results.inner.get("tests::other_test").unwrap();
        validate_metrics(metrics, 1_000_000.0, None, None);

        let metrics = results.inner.get("tests::last_test").unwrap();
        validate_metrics(metrics, 2_000_000.0, None, None);
    }

    #[test]
    fn test_adapter_rust_test_failed() {
        let contents = std::fs::read_to_string("./tool_output/rust/cargo_test_failed.txt").unwrap();
        assert!(AdapterRust::parse(&contents, Settings::default()).is_err());
    }

    #[test]
    fn test_adapter_rust_test_failed_allow_failure() {
        let contents = std::fs::read_to_string("./tool_output/rust/cargo_test_failed.txt").unwrap();
        let results = AdapterRust::parse(
            &contents,
            Settings {
                allow_failure: true,
            },
        )
        .unwrap();
        assert_eq!(results.inner.len(), 3);

        let metrics = results.inner.get("tests::ignored").unwrap();
        validate_metrics(metrics, 0.0, None, None);

        let metrics = results.inner.get("tests::benchmark_a").unwrap();
        validate_metrics(metrics, 1_000_000.0, None, None);

        let metrics = results.inner.get("tests::benchmark_b").unwrap();
        validate_metrics(metrics, 2_000_000.0, None, None);
    }
}
