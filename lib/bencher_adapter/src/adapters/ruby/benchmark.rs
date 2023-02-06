use bencher_json::{BenchmarkName, JsonMetric};
use nom::{
    bytes::complete::tag,
    character::complete::{anychar, space1},
    combinator::{eof, map, map_res},
    multi::many_till,
    sequence::{delimited, tuple},
    IResult,
};

use crate::{
    adapters::util::{latency_as_nanos, parse_benchmark_name_chars, parse_f64, NomError, Units},
    results::adapter_results::AdapterResults,
    Adapter,
};

pub struct AdapterRubyBenchmark;

impl Adapter for AdapterRubyBenchmark {
    fn parse(input: &str) -> Option<AdapterResults> {
        let mut benchmark_metrics = Vec::new();

        let mut header = false;
        for line in input.lines() {
            if !header {
                header = parse_header(line).is_ok();
                continue;
            }

            if let Ok((remainder, benchmark_metric)) = parse_ruby(line) {
                if remainder.is_empty() {
                    benchmark_metrics.push(benchmark_metric);
                    continue;
                }
            }

            header = false;
        }

        AdapterResults::new_latency(benchmark_metrics)
    }
}

fn parse_header(input: &str) -> IResult<&str, ()> {
    map(
        tuple((
            space1,
            tag("user"),
            space1,
            tag("system"),
            space1,
            tag("total"),
            space1,
            tag("real"),
            eof,
        )),
        |_| (),
    )(input)
}

fn parse_ruby(input: &str) -> IResult<&str, (BenchmarkName, JsonMetric)> {
    map_res(
        many_till(anychar, parse_ruby_benchmark),
        |(name, json_metric)| -> Result<(BenchmarkName, JsonMetric), NomError> {
            let benchmark_name = parse_benchmark_name_chars(&name)?;
            Ok((benchmark_name, json_metric))
        },
    )(input)
}

fn parse_ruby_benchmark(input: &str) -> IResult<&str, JsonMetric> {
    map_res(
        tuple((
            space1,
            parse_f64,
            space1,
            parse_f64,
            space1,
            parse_f64,
            space1,
            delimited(tag("("), tuple((space1, parse_f64)), tag(")")),
            eof,
        )),
        |(_, _user, _, _system, _, _total, _, (_, real), _)| -> Result<JsonMetric, NomError> {
            let units = Units::Sec;
            let value = latency_as_nanos(real, units);
            Ok(JsonMetric {
                value,
                lower_bound: None,
                upper_bound: None,
            })
        },
    )(input)
}

#[cfg(test)]
pub(crate) mod test_ruby_benchmark {
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::test_util::{convert_file_path, validate_latency},
        AdapterResults,
    };

    use super::AdapterRubyBenchmark;

    fn convert_ruby_benchmark(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/ruby/benchmark/{suffix}.txt");
        convert_file_path::<AdapterRubyBenchmark>(&file_path)
    }

    #[test]
    fn test_adapter_ruby_benchmark_two() {
        let results = convert_ruby_benchmark("two");
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("sort!").unwrap();
        validate_latency(metrics, 1460465000.0, None, None);

        let metrics = results.get("sort").unwrap();
        validate_latency(metrics, 1448327000.0, None, None);
    }

    #[test]
    fn test_adapter_ruby_benchmark_five() {
        let results = convert_ruby_benchmark("five");
        validate_adapter_ruby_benchmark(results);
    }

    pub fn validate_adapter_ruby_benchmark(results: AdapterResults) {
        assert_eq!(results.inner.len(), 5);

        let metrics = results.get("for:").unwrap();
        validate_latency(metrics, 952039000.0, None, None);

        let metrics = results.get("times:").unwrap();
        validate_latency(metrics, 984938000.0, None, None);

        let metrics = results.get("upto:").unwrap();
        validate_latency(metrics, 946787000.0, None, None);

        let metrics = results.get(">total:").unwrap();
        validate_latency(metrics, 2883764000.0, None, None);

        let metrics = results.get(">avg:").unwrap();
        validate_latency(metrics, 961255000.0, None, None);
    }
}
