use bencher_json::{project::report::JsonAverage, BenchmarkName, JsonMetric};
use nom::{
    bytes::complete::{tag, take_until1},
    character::complete::space1,
    combinator::{eof, map, map_res},
    sequence::{delimited, tuple},
    IResult,
};

use crate::{
    adapters::util::{
        latency_as_nanos, parse_benchmark_name, parse_number_as_f64, parse_units, NomError,
    },
    results::adapter_results::AdapterResults,
    Adaptable, Settings,
};

pub struct AdapterRustBench;

impl Adaptable for AdapterRustBench {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        match settings.average {
            Some(JsonAverage::Median) | None => {},
            Some(JsonAverage::Mean) => return None,
        }

        let mut benchmark_metrics = Vec::new();

        for line in input.lines() {
            if let Ok((remainder, benchmark_metric)) = parse_cargo(line) {
                if remainder.is_empty() {
                    benchmark_metrics.push(benchmark_metric);
                }
            }
        }

        AdapterResults::new_latency(benchmark_metrics)
    }
}

fn parse_cargo(input: &str) -> IResult<&str, (BenchmarkName, JsonMetric)> {
    map_res(
        tuple((
            tag("test"),
            space1,
            take_until1(" "),
            space1,
            tag("..."),
            space1,
            parse_cargo_bench,
            eof,
        )),
        |(_, _, name, _, _, _, json_metric, _)| -> Result<(BenchmarkName, JsonMetric), NomError> {
            let benchmark_name = parse_benchmark_name(name)?;
            Ok((benchmark_name, json_metric))
        },
    )(input)
}

// cargo bench
// TODO cargo test -- -Z unstable-options --format json
fn parse_cargo_bench(input: &str) -> IResult<&str, JsonMetric> {
    map(
        tuple((
            tag("bench:"),
            space1,
            parse_number_as_f64,
            space1,
            parse_units,
            tag("/iter"),
            space1,
            delimited(
                tag("("),
                tuple((tag("+/-"), space1, parse_number_as_f64)),
                tag(")"),
            ),
        )),
        |(_, _, duration, _, units, _, _, (_, _, variance))| {
            let value = latency_as_nanos(duration, units);
            let variance = Some(latency_as_nanos(variance, units));
            JsonMetric {
                value,
                lower_value: variance.map(|v| value - v),
                upper_value: variance.map(|v| value + v),
            }
        },
    )(input)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(crate) mod test_rust_bench {
    use bencher_json::{project::report::JsonAverage, JsonMetric};
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::test_util::{convert_file_path, opt_convert_file_path, validate_latency},
        AdapterResults, Settings,
    };

    use super::{parse_cargo, AdapterRustBench};

    fn convert_rust_bench(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/rust/bench/{suffix}.txt");
        convert_file_path::<AdapterRustBench>(&file_path)
    }

    fn validate_bench_metrics(results: &AdapterResults, key: &str) {
        let metrics = results.get(key).unwrap();
        validate_latency(metrics, 3_161.0, Some(2_186.0), Some(4_136.0));
    }

    #[test]
    fn test_adapter_rust_zero() {
        let file_path = "./tool_output/rust/bench/zero.txt";
        assert_eq!(
            None,
            opt_convert_file_path::<AdapterRustBench>(file_path, Settings::default())
        );
    }

    #[test]
    fn test_adapter_rust_average() {
        let file_path = "./tool_output/rust/bench/many.txt";
        assert_eq!(
            None,
            opt_convert_file_path::<AdapterRustBench>(
                file_path,
                Settings {
                    average: Some(JsonAverage::Mean)
                }
            )
        );

        let results = opt_convert_file_path::<AdapterRustBench>(
            file_path,
            Settings {
                average: Some(JsonAverage::Median),
            },
        )
        .unwrap();
        validate_adapter_rust_bench(&results);
    }

    #[test]
    fn test_parse_cargo() {
        for (index, (expected, input)) in [(
            Ok((
                "",
                (
                    "tests::is_bench_test".parse().unwrap(),
                    JsonMetric {
                        value: 5_280.0.into(),
                        lower_value: Some(4_947.0.into()),
                        upper_value: Some(5_613.0.into()),
                    },
                ),
            )),
            "test tests::is_bench_test ... bench:             5,280 ns/iter (+/- 333)",
        )]
        .into_iter()
        .enumerate()
        {
            assert_eq!(expected, parse_cargo(input), "#{index}: {input}");
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
            assert_eq!(true, parse_cargo(input).is_err(), "#{index}: {input}");
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
        validate_adapter_rust_bench(&results);
    }

    pub fn validate_adapter_rust_bench(results: &AdapterResults) {
        assert_eq!(results.inner.len(), 6);
        validate_bench_metrics(results, "tests::benchmark");
        validate_bench_metrics(results, "tests::other_benchmark");
        validate_bench_metrics(results, "tests::last_benchmark");

        let number = 1_000.0;
        let metrics = results.get("tests::one_digit").unwrap();
        validate_latency(metrics, number, Some(0.0), Some(2000.0));

        let number = 22_000_000.0;
        let metrics = results.get("tests::two_digit").unwrap();
        validate_latency(metrics, number, Some(0.0), Some(44_000_000.0));

        let number = 333_000_000_000.0;
        let metrics = results.get("tests::three_digit").unwrap();
        validate_latency(metrics, number, Some(0.0), Some(666_000_000_000.0));
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
        let results = convert_rust_bench("failed");
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("tests::benchmark_a").unwrap();
        validate_latency(metrics, 3_296.0, Some(2_775.0), Some(3_817.0));

        let metrics = results.get("tests::benchmark_c").unwrap();
        validate_latency(metrics, 3_215.0, Some(2_859.0), Some(3_571.0));
    }

    #[test]
    #[allow(clippy::unreadable_literal)]
    fn test_issue_390() {
        let results = convert_rust_bench("issue_390");
        assert_eq!(results.inner.len(), 4);

        let metrics = results.get("bleu::benchmark::bench_batch_bleu").unwrap();
        validate_latency(
            metrics,
            13_967_756.3,
            Some(13_700_986.66),
            Some(14_234_525.940000001),
        );

        let metrics = results.get("bleu::benchmark::bench_bleu").unwrap();
        validate_latency(
            metrics,
            298_794.1,
            Some(296_154.12),
            Some(301_434.07999999996),
        );

        let metrics = results.get("ngram::benchmark::bench_ngram").unwrap();
        validate_latency(metrics, 49_480.28, Some(48_978.95), Some(49_981.61));

        let metrics = results
            .get("tokenizer::benchmark::bench_tokenizer")
            .unwrap();
        validate_latency(metrics, 15_690.73, Some(8_940.759999999998), Some(22440.7));
    }
}
