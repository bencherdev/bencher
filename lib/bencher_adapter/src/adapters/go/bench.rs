use bencher_json::{BenchmarkName, JsonMetric};
use nom::{
    bytes::complete::{tag, take_until1},
    character::complete::space1,
    combinator::{eof, map_res},
    sequence::{delimited, tuple},
    IResult,
};

use crate::{
    adapters::util::{
        parse_benchmark_name, parse_f64, parse_u64, parse_units, time_as_nanos, NomError,
    },
    results::adapter_results::AdapterResults,
    Adapter, AdapterError,
};

pub struct AdapterGoBench;

impl Adapter for AdapterGoBench {
    fn parse(input: &str) -> Result<AdapterResults, AdapterError> {
        let mut benchmark_metrics = Vec::new();

        for line in input.lines() {
            if let Ok((remainder, benchmark_metric)) = parse_go(line) {
                if remainder.is_empty() {
                    benchmark_metrics.push(benchmark_metric);
                }
            }
        }

        benchmark_metrics.try_into()
    }
}

fn parse_go(input: &str) -> IResult<&str, (BenchmarkName, JsonMetric)> {
    map_res(
        tuple((
            take_until1(" "),
            space1,
            parse_u64,
            space1,
            parse_go_bench,
            eof,
        )),
        |(name, _, _iter, _, json_metric, _)| -> Result<(BenchmarkName, JsonMetric), NomError> {
            let benchmark_name = parse_benchmark_name(name)?;
            Ok((benchmark_name, json_metric))
        },
    )(input)
}

fn parse_go_bench(input: &str) -> IResult<&str, JsonMetric> {
    map_res(
        tuple((parse_f64, space1, parse_units, tag("/op"))),
        |(duration, _, units, _)| -> Result<JsonMetric, NomError> {
            let value = time_as_nanos(duration, units);
            Ok(JsonMetric {
                value,
                lower_bound: None,
                upper_bound: None,
            })
        },
    )(input)
}

#[cfg(test)]
pub(crate) mod test_go_bench {
    use bencher_json::JsonMetric;
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::test_util::{convert_file_path, validate_metrics},
        Adapter, AdapterResults,
    };

    use super::{parse_go, AdapterGoBench};

    fn convert_go_bench(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/go/bench/{suffix}.txt");
        convert_file_path::<AdapterGoBench>(&file_path)
    }

    #[test]
    fn test_parse_go() {
        for (index, (expected, input)) in [(
            Ok((
                "",
                (
                    "BenchmarkFib10-8".parse().unwrap(),
                    JsonMetric {
                        value: 325.0.into(),
                        lower_bound: None,
                        upper_bound: None,
                    },
                ),
            )),
            "BenchmarkFib10-8   		 					5000000		325 ns/op",
        )]
        .into_iter()
        .enumerate()
        {
            assert_eq!(expected, parse_go(input), "#{index}: {input}")
        }
    }

    // #[test]
    // fn test_adapter_rust_one() {
    //     let results = convert_rust_bench("one");
    //     assert_eq!(results.inner.len(), 1);
    //     validate_bench_metrics(&results, "tests::benchmark");
    // }

    // #[test]
    // fn test_adapter_rust_ignore() {
    //     let results = convert_rust_bench("ignore");
    //     assert_eq!(results.inner.len(), 1);
    //     validate_bench_metrics(&results, "tests::benchmark");
    // }

    // #[test]
    // fn test_adapter_rust_many() {
    //     let results = convert_rust_bench("many");
    //     validate_adapter_rust_many(results);
    // }

    // pub fn validate_adapter_rust_many(results: AdapterResults) {
    //     assert_eq!(results.inner.len(), 6);
    //     validate_bench_metrics(&results, "tests::benchmark");
    //     validate_bench_metrics(&results, "tests::other_benchmark");
    //     validate_bench_metrics(&results, "tests::last_benchmark");

    //     let number = 1_000.0;
    //     let metrics = results.get("tests::one_digit").unwrap();
    //     validate_metrics(metrics, number, Some(0.0), Some(2000.0));

    //     let number = 22_000_000.0;
    //     let metrics = results.get("tests::two_digit").unwrap();
    //     validate_metrics(metrics, number, Some(0.0), Some(44_000_000.0));

    //     let number = 333_000_000_000.0;
    //     let metrics = results.get("tests::three_digit").unwrap();
    //     validate_metrics(metrics, number, Some(0.0), Some(666_000_000_000.0));
    // }

    // #[test]
    // fn test_adapter_rust_multi_target() {
    //     let results = convert_rust_bench("multi_target");
    //     assert_eq!(results.inner.len(), 2);
    //     validate_bench_metrics(&results, "tests::benchmark");
    //     validate_bench_metrics(&results, "tests::other_benchmark");
    // }

    // #[test]
    // fn test_adapter_rust_failed() {
    //     let contents = std::fs::read_to_string("./tool_output/rust/bench/failed.txt").unwrap();
    //     let results = AdapterGoBench::parse(&contents).unwrap();
    //     assert_eq!(results.inner.len(), 2);

    //     let metrics = results.get("tests::benchmark_a").unwrap();
    //     validate_metrics(metrics, 3_296.0, Some(2_775.0), Some(3_817.0));

    //     let metrics = results.get("tests::benchmark_c").unwrap();
    //     validate_metrics(metrics, 3_215.0, Some(2_859.0), Some(3_571.0));
    // }
}
