use std::{collections::HashMap, str::FromStr, time::Duration};

use bencher_json::JsonMetric;
use literally::hmap;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_until1},
    character::complete::{anychar, digit1, space1},
    combinator::{eof, map, map_res, peek, success},
    multi::{fold_many1, many1, many_till},
    sequence::{delimited, tuple},
    IResult,
};
use ordered_float::OrderedFloat;

use crate::{
    results::{
        adapter_metrics::AdapterMetrics, adapter_results::AdapterResults, LATENCY_RESOURCE_ID,
    },
    Adapter, AdapterError, Settings,
};

pub struct AdapterRustBench;

impl Adapter for AdapterRustBench {
    fn parse(input: &str, settings: Settings) -> Result<AdapterResults, AdapterError> {
        let mut benchmark_metrics = Vec::new();

        for line in input.lines() {
            if let Ok((remainder, (benchmark_name, test_metric))) = parse_cargo(line) {
                if remainder.is_empty() {
                    match test_metric {
                        TestMetric::Ignored => continue,
                        TestMetric::Failed => {
                            if settings.allow_failure {
                                continue;
                            }

                            return Err(AdapterError::BenchmarkFailed(benchmark_name));
                        },
                        TestMetric::Ok(metric) | TestMetric::Bench(metric) => {
                            benchmark_metrics.push((benchmark_name, metric));
                        },
                    }
                }
            }

            if let Ok((remainder, (thread, context, location))) = parse_panic(line) {
                if remainder.is_empty() {
                    if settings.allow_failure {
                        continue;
                    }

                    return Err(AdapterError::Panic {
                        thread,
                        context,
                        location,
                    });
                }
            }
        }

        Ok(benchmark_metrics
            .into_iter()
            .filter_map(|(benchmark_name, metric)| {
                Some((
                    benchmark_name.as_str().parse().ok()?,
                    AdapterMetrics {
                        inner: hmap! {
                            LATENCY_RESOURCE_ID.clone() => metric
                        },
                    },
                ))
            })
            .collect::<HashMap<_, _>>()
            .into())
    }
}

#[derive(Debug, PartialEq, Eq)]
enum TestMetric {
    Ignored,
    Failed,
    Ok(JsonMetric),
    Bench(JsonMetric),
}

fn parse_cargo(input: &str) -> IResult<&str, (String, TestMetric)> {
    map(
        tuple((
            tag("test"),
            space1,
            take_until1(" "),
            space1,
            tag("..."),
            space1,
            alt((
                map(tag("ignored"), |_| TestMetric::Ignored),
                map(
                    tuple((
                        tag("FAILED"),
                        // Strip trailing report time
                        many_till(anychar, peek(eof)),
                    )),
                    |_| TestMetric::Failed,
                ),
                map(parse_cargo_ok, TestMetric::Ok),
                map(parse_cargo_bench, TestMetric::Bench),
            )),
            eof,
        )),
        |(_, _, benchmark_name, _, _, _, test_metric, _)| (benchmark_name.into(), test_metric),
    )(input)
}

// cargo test -- -Z unstable-options --report-time
// TODO cargo test -- -Z unstable-options --format json --report-time
#[allow(clippy::float_arithmetic)]
fn parse_cargo_ok(input: &str) -> IResult<&str, JsonMetric> {
    map_res(
        tuple((
            tag("ok"),
            space1,
            delimited(tag("<"), tuple((parse_float, parse_units)), tag(">")),
        )),
        |(_, _, (duration, units))| -> Result<JsonMetric, nom::Err<nom::error::Error<String>>> {
            let value = to_f64(duration)? * units.as_nanos();
            Ok(JsonMetric {
                value: value.into(),
                lower_bound: None,
                upper_bound: None,
            })
        },
    )(input)
}

// cargo bench
// TODO cargo test -- -Z unstable-options --format json
#[allow(clippy::arithmetic_side_effects, clippy::cast_precision_loss)]
fn parse_cargo_bench(input: &str) -> IResult<&str, JsonMetric> {
    map_res(
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
        |(_, _, duration, _, units, _, _, (_, _, variance))| -> Result<JsonMetric, nom::Err<nom::error::Error<String>>> {
            let value = (to_duration(to_u64(duration)?, &units).as_nanos() as f64).into();
            let variance = Some(OrderedFloat::from(
                to_duration(to_u64(variance)?, &units).as_nanos() as f64,
            ));
            Ok(JsonMetric {
                value,
                lower_bound: variance.map(|v| value - v),
                upper_bound: variance.map(|v| value + v),
            })
        },
    )(input)
}

fn parse_panic(input: &str) -> IResult<&str, (String, String, String)> {
    map(
        tuple((
            tag("thread "),
            delimited(tag("'"), many_till(anychar, peek(tag("'"))), tag("'")),
            tag(" panicked at "),
            delimited(tag("'"), many_till(anychar, peek(tag("'"))), tag("'")),
            tag(", "),
            many_till(anychar, eof),
        )),
        |(_, (thread, _), _, (context, _), _, (location, _))| {
            (
                thread.into_iter().collect(),
                context.into_iter().collect(),
                location.into_iter().collect(),
            )
        },
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
        map(tag("\u{3bc}s"), |_| Units::Micro),
        map(tag("\u{b5}s"), |_| Units::Micro),
        map(tag("ms"), |_| Units::Milli),
        map(tag("s"), |_| Units::Sec),
    ))(input)
}

impl Units {
    #[allow(clippy::float_arithmetic)]
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

fn parse_int(input: &str) -> IResult<&str, Vec<(&str, &str)>> {
    many1(tuple((digit1, alt((tag(","), success(" "))))))(input)
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

fn to_f64(input: Vec<&str>) -> Result<f64, nom::Err<nom::error::Error<String>>> {
    let mut number = String::new();
    for floating_point in input {
        number.push_str(floating_point);
    }
    f64::from_str(&number)
        .map_err(|_e| nom::Err::Error(nom::error::make_error(number, nom::error::ErrorKind::Tag)))
}

fn to_u64(input: Vec<(&str, &str)>) -> Result<u64, nom::Err<nom::error::Error<String>>> {
    let mut number = String::new();
    for (digit, _) in input {
        number.push_str(digit);
    }
    u64::from_str(&number)
        .map_err(|_e| nom::Err::Error(nom::error::make_error(number, nom::error::ErrorKind::Tag)))
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    clippy::float_arithmetic
)]
fn to_duration(time: u64, units: &Units) -> Duration {
    match units {
        Units::Pico => Duration::from_nanos((time as f64 * units.as_nanos()) as u64),
        Units::Nano => Duration::from_nanos(time),
        Units::Micro => Duration::from_micros(time),
        Units::Milli => Duration::from_millis(time),
        Units::Sec => Duration::from_secs(time),
    }
}

#[cfg(test)]
pub(crate) mod test_rust {
    use bencher_json::JsonMetric;
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::test_util::{convert_file_path, validate_metrics},
        Adapter, AdapterResults, Settings,
    };

    use super::{parse_cargo, parse_panic, AdapterRustBench, TestMetric};

    fn convert_rust_bench(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/rust/bench/{}.txt", suffix);
        convert_file_path::<AdapterRustBench>(&file_path, Settings::default())
    }

    fn convert_rust_test(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/rust/bench/test_{}.txt", suffix);
        convert_file_path::<AdapterRustBench>(&file_path, Settings::default())
    }

    fn validate_bench_metrics(results: &AdapterResults, key: &str) {
        let metrics = results.get(key).unwrap();
        validate_metrics(metrics, 3_161.0, Some(2_186.0), Some(4_136.0));
    }

    #[test]
    fn test_adapter_rust_zero() {
        let results = convert_rust_bench("zero");
        assert_eq!(results.inner.len(), 0);
    }

    #[test]
    fn test_parse_cargo() {
        for (index, (expected, input)) in [
            (
                Ok(("", ("tests::is_ignored".into(), TestMetric::Ignored))),
                "test tests::is_ignored ... ignored",
            ),
            (
                Ok(("", ("tests::is_failed".into(), TestMetric::Failed))),
                "test tests::is_failed ... FAILED",
            ),
            (
                Ok((
                    "",
                    (
                        "tests::is_unit_test".into(),
                        TestMetric::Ok(JsonMetric {
                            value: 1_000_000_000.0.into(),
                            lower_bound: None,
                            upper_bound: None,
                        }),
                    ),
                )),
                "test tests::is_unit_test ... ok <1.000s>",
            ),
            (
                Ok((
                    "",
                    (
                        "tests::is_bench_test".into(),
                        TestMetric::Bench(JsonMetric {
                            value: 5_280.0.into(),
                            lower_bound: Some(4_947.0.into()),
                            upper_bound: Some(5_613.0.into()),
                        }),
                    ),
                )),
                "test tests::is_bench_test ... bench:             5,280 ns/iter (+/- 333)",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(expected, parse_cargo(input), "#{index}: {input}")
        }

        for (index, input) in [
            "",
            "tests::is_ignored",
            "test tests::is_ignored ... ignored\n",
            " test tests::is_ignored ... ignored",
            "prefix test tests::is_ignored ... ignored",
        ]
        .iter()
        .enumerate()
        {
            assert_eq!(true, parse_cargo(input).is_err(), "#{index}: {input}")
        }
    }

    #[test]
    fn test_parse_panic() {
        for (index, (expected, input)) in [(
            Ok((
                "",
                (
                    "main".into(),
                    "explicit panic".into(),
                    "trace4rs/benches/trace4rs_bench.rs:42:5".into(),
                ),
            )),
            "thread 'main' panicked at 'explicit panic', trace4rs/benches/trace4rs_bench.rs:42:5",
        )]
        .into_iter()
        .enumerate()
        {
            assert_eq!(expected, parse_panic(input), "#{index}: {input}")
        }
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
        let metrics = results.get("tests::one_digit").unwrap();
        validate_metrics(metrics, number, Some(0.0), Some(2000.0));

        let number = 22_000_000.0;
        let metrics = results.get("tests::two_digit").unwrap();
        validate_metrics(metrics, number, Some(0.0), Some(44_000_000.0));

        let number = 333_000_000_000.0;
        let metrics = results.get("tests::three_digit").unwrap();
        validate_metrics(metrics, number, Some(0.0), Some(666_000_000_000.0));
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
        let contents = std::fs::read_to_string("./tool_output/rust/bench/failed.txt").unwrap();
        assert!(AdapterRustBench::parse(&contents, Settings::default()).is_err());
    }

    #[test]
    fn test_adapter_rust_failed_allow_failure() {
        let contents = std::fs::read_to_string("./tool_output/rust/bench/failed.txt").unwrap();
        let results = AdapterRustBench::parse(
            &contents,
            Settings {
                allow_failure: true,
            },
        )
        .unwrap();
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("tests::benchmark_a").unwrap();
        validate_metrics(metrics, 3_296.0, Some(2_775.0), Some(3_817.0));

        let metrics = results.get("tests::benchmark_c").unwrap();
        validate_metrics(metrics, 3_215.0, Some(2_859.0), Some(3_571.0));
    }

    #[test]
    fn test_adapter_rust_test_report_time() {
        let results = convert_rust_test("report_time");
        assert_eq!(results.inner.len(), 3);

        let metrics = results.get("tests::unit_test").unwrap();
        validate_metrics(metrics, 0.0, None, None);

        let metrics = results.get("tests::other_test").unwrap();
        validate_metrics(metrics, 1_000_000.0, None, None);

        let metrics = results.get("tests::last_test").unwrap();
        validate_metrics(metrics, 2_000_000.0, None, None);
    }

    #[test]
    fn test_adapter_rust_test_failed() {
        let contents =
            std::fs::read_to_string("./tool_output/rust/bench/test_report_time_failed.txt")
                .unwrap();
        assert!(AdapterRustBench::parse(&contents, Settings::default()).is_err());
    }

    #[test]
    fn test_adapter_rust_test_failed_allow_failure() {
        let contents =
            std::fs::read_to_string("./tool_output/rust/bench/test_report_time_failed.txt")
                .unwrap();
        let results = AdapterRustBench::parse(
            &contents,
            Settings {
                allow_failure: true,
            },
        )
        .unwrap();
        assert_eq!(results.inner.len(), 3);

        let metrics = results.get("tests::ignored").unwrap();
        validate_metrics(metrics, 0.0, None, None);

        let metrics = results.get("tests::benchmark_a").unwrap();
        validate_metrics(metrics, 1_000_000.0, None, None);

        let metrics = results.get("tests::benchmark_b").unwrap();
        validate_metrics(metrics, 2_000_000.0, None, None);
    }
}
