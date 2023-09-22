use bencher_json::{project::report::JsonAverage, BenchmarkName, JsonMetric};

use nom::{
    bytes::complete::tag,
    character::complete::{anychar, space1},
    combinator::{eof, map, map_res},
    multi::many_till,
    sequence::{delimited, tuple},
    IResult,
};

use crate::{
    adapters::util::{
        nom_error, parse_benchmark_name_chars, parse_f64, parse_u64, throughput_as_secs, NomError,
        Units,
    },
    results::adapter_results::AdapterResults,
    Adapter, Settings,
};

pub struct AdapterJsBenchmark;

impl Adapter for AdapterJsBenchmark {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        match settings.average {
            Some(JsonAverage::Median) | None => {},
            Some(JsonAverage::Mean) => return None,
        }

        let mut benchmark_metrics = Vec::new();

        for line in input.lines() {
            if let Ok((remainder, benchmark_metric)) = parse_benchmark(line) {
                if remainder.is_empty() {
                    benchmark_metrics.push(benchmark_metric);
                }
            }
        }

        AdapterResults::new_throughput(benchmark_metrics)
    }
}

fn parse_benchmark(input: &str) -> IResult<&str, (BenchmarkName, JsonMetric)> {
    map_res(
        many_till(anychar, parse_benchmark_time),
        |(name_chars, json_metric)| -> Result<(BenchmarkName, JsonMetric), NomError> {
            if name_chars.is_empty() {
                return Err(nom_error(String::new()));
            }
            let benchmark_name = parse_benchmark_name_chars(&name_chars)?;
            Ok((benchmark_name, json_metric))
        },
    )(input)
}

fn parse_benchmark_time(input: &str) -> IResult<&str, JsonMetric> {
    map(
        tuple((
            tuple((space1, tag("x"), space1)),
            parse_u64,
            tuple((space1, tag("ops/sec"), space1, tag("\u{b1}"))),
            parse_f64,
            tuple((
                tag("%"),
                space1,
                delimited(
                    tag("("),
                    tuple((parse_u64, space1, tag("runs"), space1, tag("sampled"))),
                    tag(")"),
                ),
                eof,
            )),
        )),
        |(_, throughput, _, percent_error, _)| {
            let value = throughput_as_secs(throughput, Units::Sec);
            let error = value * percent_error;
            JsonMetric {
                value,
                lower_bound: Some(value - error),
                upper_bound: Some(value + error),
            }
        },
    )(input)
}

#[cfg(test)]
pub(crate) mod test_js_benchmark {
    use bencher_json::project::report::JsonAverage;
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::test_util::{convert_file_path, opt_convert_file_path, validate_throughput},
        AdapterResults, Settings,
    };

    use super::AdapterJsBenchmark;

    fn convert_js_benchmark(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/js/benchmark/{suffix}.txt");
        convert_file_path::<AdapterJsBenchmark>(&file_path)
    }

    #[test]
    fn test_adapter_js_benchmark_average() {
        let file_path = "./tool_output/js/benchmark/three.txt";
        assert_eq!(
            None,
            opt_convert_file_path::<AdapterJsBenchmark>(
                file_path,
                Settings {
                    average: Some(JsonAverage::Mean)
                }
            )
        );

        let results = opt_convert_file_path::<AdapterJsBenchmark>(
            file_path,
            Settings {
                average: Some(JsonAverage::Median),
            },
        )
        .unwrap();
        validate_adapter_js_benchmark(&results);
    }

    #[test]
    fn test_adapter_js_benchmark() {
        let results = convert_js_benchmark("three");
        validate_adapter_js_benchmark(&results);
    }

    pub fn validate_adapter_js_benchmark(results: &AdapterResults) {
        assert_eq!(results.inner.len(), 3);

        let metrics = results.get("fib(10)").unwrap();
        validate_throughput(
            metrics,
            1_431_759.0,
            Some(372_257.340_000_000_1),
            Some(2_491_260.66),
        );

        let metrics = results.get("fib(20)").unwrap();
        validate_throughput(
            metrics,
            12146.0,
            Some(8_259.279_999_999_999),
            Some(16_032.720_000_000_001),
        );

        let metrics = results.get("benchmark with x 2 many things").unwrap();
        validate_throughput(metrics, 50.0, Some(45.0), Some(55.0));
    }
}
