use std::{collections::HashMap, str::FromStr, time::Duration};

use bencher_json::{project::metric_kind::LATENCY_SLUG_STR, JsonMetric};
use literally::hmap;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_until1},
    character::complete::{anychar, digit1, line_ending, space1},
    combinator::{map, map_res, peek, success},
    multi::{fold_many1, many0, many1, many_till},
    sequence::{delimited, tuple},
    IResult,
};

use crate::{
    results::{adapter_metrics::AdapterMetrics, adapter_results::AdapterResults},
    Adapter, AdapterError, Settings,
};

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
    fold_many1(
        |i| parse_running(i, settings),
        HashMap::new,
        |benchmarks, new_benchmarks| benchmarks.into_iter().chain(new_benchmarks).collect(),
    )(input)
    .map(|(remainder, benchmarks)| (remainder, benchmarks.into()))
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
                parse_running_x_tests,
            )),
            // test rust::mod::path::to_test ... ignored/Y ns/iter (+/- Z)
            many0(|input| parse_cargo_bench(input, settings)),
            tuple((
                // This will contain failure information
                // failures: ...
                many_till(anychar, tag("test result:")),
                many_till(anychar, line_ending),
                alt((
                    // error: test failed ...
                    map(
                        tuple((
                            many0(line_ending),
                            tag("error:"),
                            many_till(anychar, line_ending),
                            many0(line_ending),
                        )),
                        |_| (),
                    ),
                    map(many0(line_ending), |_| ()),
                )),
            )),
        )),
        |(_, benchmarks, _)| -> Result<_, AdapterError> {
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

fn parse_cargo_bench(
    input: &str,
    settings: Settings,
) -> IResult<&str, Result<Option<(String, JsonMetric)>, AdapterError>> {
    map(
        tuple((|input| parse_test_result(input), success(""))),
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

fn parse_test_result(input: &str) -> IResult<&str, (String, Test)> {
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

pub enum Units {
    Nano,
    Micro,
    Milli,
    Sec,
}

impl From<&str> for Units {
    fn from(time: &str) -> Self {
        match time {
            "ns" => Self::Nano,
            "μs" => Self::Micro,
            "ms" => Self::Milli,
            "s" => Self::Sec,
            _ => panic!("Unexpected time abbreviation"),
        }
    }
}

impl Units {
    fn as_nanos(&self) -> usize {
        match self {
            Self::Nano => 1,
            Self::Micro => 1_000,
            Self::Milli => 1_000_000,
            Self::Sec => 1_000_000_000,
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
            take_until1("/"),
            tag("/iter"),
            space1,
            delimited(tag("("), tuple((tag("+/-"), space1, parse_int)), tag(")")),
        )),
        |(_, _, duration, _, units, _, _, (_, _, variance))| {
            let units = Units::from(units);
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
            delimited(tag("<"), tuple((parse_float, take_until1(">"))), tag(">")),
        )),
        |(_, _, (duration, units))| {
            let units = Units::from(units);
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
        alt((digit1, tag("."))),
        Vec::new,
        |mut float_chars, float_char| {
            float_chars.push(float_char);
            float_chars
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

    use super::{parse_multi_mod, parse_running_x_tests, parse_test_result, AdapterRust, Test};
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
            "Running benches/name.rs (/oath/to/target/release/deps/name-hash)",
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
    fn test_parse_test_result() {
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
            assert_eq!(expected, parse_test_result(input), "#{index}: {input}")
        }

        for (index, input) in ["", "tests::is_ignored", " tests::is_ignored"]
            .iter()
            .enumerate()
        {
            assert_eq!(true, parse_test_result(input).is_err(), "#{index}: {input}")
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
        assert_eq!(results.inner.len(), 0);
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
