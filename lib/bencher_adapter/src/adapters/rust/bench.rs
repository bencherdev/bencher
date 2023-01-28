use bencher_json::JsonMetric;
use nom::{
    branch::alt,
    bytes::complete::{tag, take_until1},
    character::complete::space1,
    combinator::{eof, map, map_res},
    sequence::{delimited, tuple},
    IResult,
};

use crate::{
    adapters::util::{parse_u64, parse_units, time_as_nanos},
    results::adapter_results::AdapterResults,
    Adapter, AdapterError,
};

pub struct AdapterRustBench;

impl Adapter for AdapterRustBench {
    fn parse(input: &str) -> Result<AdapterResults, AdapterError> {
        let mut benchmark_metrics = Vec::new();

        for line in input.lines() {
            if let Ok((remainder, (benchmark_name, test_metric))) = parse_cargo(line) {
                if remainder.is_empty() {
                    if let TestMetric::Bench(metric) = test_metric {
                        benchmark_metrics.push((benchmark_name, metric));
                    }
                }
            }
        }

        benchmark_metrics.try_into()
    }
}

#[derive(Debug, PartialEq, Eq)]
enum TestMetric {
    Ok,
    Ignored,
    Failed,
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
                map(tag("ok"), |_| TestMetric::Ok),
                map(tag("ignored"), |_| TestMetric::Ignored),
                map(tag("FAILED"), |_| TestMetric::Failed),
                map(parse_cargo_bench, TestMetric::Bench),
            )),
            eof,
        )),
        |(_, _, benchmark_name, _, _, _, test_metric, _)| (benchmark_name.into(), test_metric),
    )(input)
}

// cargo bench
// TODO cargo test -- -Z unstable-options --format json
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
        Adapter, AdapterResults,
    };

    use super::{parse_cargo, AdapterRustBench, TestMetric};

    fn convert_rust_bench(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/rust/bench/{}.txt", suffix);
        convert_file_path::<AdapterRustBench>(&file_path)
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
        let results = AdapterRustBench::parse(&contents).unwrap();
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("tests::benchmark_a").unwrap();
        validate_metrics(metrics, 3_296.0, Some(2_775.0), Some(3_817.0));

        let metrics = results.get("tests::benchmark_c").unwrap();
        validate_metrics(metrics, 3_215.0, Some(2_859.0), Some(3_571.0));
    }
}
