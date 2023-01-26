use std::collections::HashMap;

use bencher_json::JsonMetric;
use literally::hmap;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_until1},
    character::complete::{anychar, space1},
    combinator::{eof, map, map_res, peek},
    multi::many_till,
    sequence::{delimited, tuple},
    IResult,
};

use crate::{
    adapters::util::{parse_f64, parse_u64, parse_units, time_as_nanos},
    results::{
        adapter_metrics::AdapterMetrics, adapter_results::AdapterResults, LATENCY_RESOURCE_ID,
    },
    Adapter, AdapterError, Settings,
};

use super::rust_panic;

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

            rust_panic(line, settings)?;
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
            delimited(tag("<"), tuple((parse_f64, parse_units)), tag(">")),
        )),
        |(_, _, (duration, units))| -> Result<JsonMetric, nom::Err<nom::error::Error<String>>> {
            let value = duration * units.as_nanos();
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
            parse_u64,
            space1,
            parse_units,
            tag("/iter"),
            space1,
            delimited(tag("("), tuple((tag("+/-"), space1, parse_u64)), tag(")")),
        )),
        |(_, _, duration, _, units, _, _, (_, _, variance))| -> Result<JsonMetric, nom::Err<nom::error::Error<String>>> {
            let value = time_as_nanos(duration, units);
            let variance = Some(time_as_nanos(variance, units));
            Ok(JsonMetric {
                value,
                lower_bound: variance.map(|v| value - v),
                upper_bound: variance.map(|v| value + v),
            })
        },
    )(input)
}

#[cfg(test)]
pub(crate) mod test_rust_bench {
    use bencher_json::JsonMetric;
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::test_util::{convert_file_path, validate_metrics},
        Adapter, AdapterResults, Settings,
    };

    use super::{parse_cargo, AdapterRustBench, TestMetric};

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
