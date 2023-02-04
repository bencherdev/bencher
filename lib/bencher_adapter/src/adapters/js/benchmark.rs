use bencher_json::{BenchmarkName, JsonEmpty, JsonMetric};

use nom::{
    bytes::complete::{tag, take_till1},
    character::complete::{anychar, space1},
    combinator::{eof, map_res},
    multi::many_till,
    sequence::tuple,
    IResult,
};
use rust_decimal::Decimal;
use serde::Deserialize;

use crate::{
    adapters::util::{latency_as_nanos, Units},
    results::adapter_results::AdapterResults,
    Adapter, AdapterError,
};

pub struct AdapterJsBenchmark;

impl Adapter for AdapterJsBenchmark {
    fn parse(input: &str) -> Option<AdapterResults> {
        let mut benchmark_metrics = Vec::new();

        for line in input.lines() {
            // if let Ok((remainder, benchmark_metric)) = parse_benchmark(line) {
            //     if remainder.is_empty() {
            //         benchmark_metrics.push(benchmark_metric);
            //     }
            // }
        }

        AdapterResults::new_latency(benchmark_metrics)
    }
}

// fn parse_benchmark(input: &str) -> IResult<&str, (BenchmarkName, JsonMetric)> {
//     map_res(
//         many_till(anychar, parse_benchmark_time),
//         |(name_chars, json_metric)| -> Result<(BenchmarkName, JsonMetric), NomError> {
//             let name: String = if name_chars.is_empty() {
//                 prior_line.ok_or_else(|| nom_error(String::new()))?.into()
//             } else {
//                 name_chars.into_iter().collect()
//             };
//             let benchmark_name = parse_benchmark_name(&name)?;
//             Ok((benchmark_name, json_metric))
//         },
//     )(input)
// }

// fn parse_benchmark_time(input: &str) -> IResult<&str, JsonMetric> {
//     map_res(
//         tuple((parse_f64, space1, parse_units, tag("/op"))),
//         |(duration, _, units, _)| -> Result<JsonMetric, NomError> {
//             let value = latency_as_nanos(duration, units);
//             Ok(JsonMetric {
//                 value,
//                 lower_bound: None,
//                 upper_bound: None,
//             })
//         },
//     )(input)
// }

#[cfg(test)]
pub(crate) mod test_js_benchmark {
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::test_util::{convert_file_path, validate_latency},
        AdapterResults,
    };

    use super::AdapterJsBenchmark;

    fn convert_js_benchmark(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/js/benchmark/{suffix}.json");
        convert_file_path::<AdapterJsBenchmark>(&file_path)
    }

    #[test]
    fn test_adapter_js_benchmark() {
        let results = convert_js_benchmark("two");
        validate_adapter_js_benchmark(results);
    }

    pub fn validate_adapter_js_benchmark(results: AdapterResults) {
        assert_eq!(results.inner.len(), 2);

        let metrics = results
            .get("BenchmarkDotNet.Samples.Intro.Sleep10")
            .unwrap();
        validate_latency(
            metrics,
            10362283.085796878,
            Some(10316580.967427673),
            Some(10407985.204166083),
        );

        let metrics = results
            .get("BenchmarkDotNet.Samples.Intro.Sleep20")
            .unwrap();
        validate_latency(
            metrics,
            20360791.931687497,
            Some(20312811.199369717),
            Some(20408772.664005276),
        );
    }
}
