use bencher_json::{BenchmarkName, JsonNewMetric, project::report::JsonAverage};
use nom::{
    IResult, Parser as _,
    bytes::complete::{tag, take_until},
    character::complete::{space0, space1},
    combinator::{eof, map_res, opt},
};

use crate::{
    Adaptable, Settings,
    adapters::util::{NomError, Units, latency_as_nanos, parse_benchmark_name, parse_f64},
    results::adapter_results::AdapterResults,
};

pub struct AdapterDartBenchmarkHarness;

impl Adaptable for AdapterDartBenchmarkHarness {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        match settings.average {
            Some(JsonAverage::Mean) | None => {},
            Some(JsonAverage::Median) => return None,
        }

        let mut benchmark_metrics = Vec::new();

        for line in input.lines() {
            if let Ok((remainder, benchmark_metric)) = parse_dart_line(line)
                && remainder.is_empty()
            {
                benchmark_metrics.push(benchmark_metric);
            }
        }

        AdapterResults::new_latency(benchmark_metrics)
    }
}

/// Parses a line from [Dart `benchmark_harness`](https://pub.dev/packages/benchmark_harness)
/// `PrintEmitter`: `<name>(RunTime): <microseconds> us.`
fn parse_dart_line(input: &str) -> IResult<&str, (BenchmarkName, JsonNewMetric)> {
    map_res(
        (
            take_until("(RunTime):"),
            tag("(RunTime):"),
            space0,
            parse_f64,
            space1,
            tag("us."),
            opt(space0),
            eof,
        ),
        |(name_raw, _, _, micros, _, _, _, _): (&str, _, _, f64, _, _, _, _)| -> Result<
            (BenchmarkName, JsonNewMetric),
            NomError,
        > {
            let benchmark_name = parse_benchmark_name(name_raw.trim())?;
            let value = latency_as_nanos(micros, Units::Micro);
            Ok((
                benchmark_name,
                JsonNewMetric {
                    value,
                    lower_value: None,
                    upper_value: None,
                },
            ))
        },
    )
    .parse(input)
}

#[cfg(test)]
pub(crate) mod test_dart_benchmark_harness {
    use bencher_json::project::report::JsonAverage;
    use pretty_assertions::assert_eq;

    use crate::{
        Adaptable as _, AdapterResults, Settings,
        adapters::test_util::{convert_file_path, opt_convert_file_path, validate_latency},
    };

    use super::AdapterDartBenchmarkHarness;

    fn convert_dart(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/dart/benchmark_harness/{suffix}.txt");
        convert_file_path::<AdapterDartBenchmarkHarness>(&file_path)
    }

    #[test]
    fn adapter_dart_benchmark_harness_median() {
        let file_path = "./tool_output/dart/benchmark_harness/two.txt";
        assert_eq!(
            None,
            opt_convert_file_path::<AdapterDartBenchmarkHarness>(
                file_path,
                Settings {
                    average: Some(JsonAverage::Median)
                }
            )
        );
    }

    #[test]
    fn adapter_dart_benchmark_harness_one() {
        let results = convert_dart("one");
        assert_eq!(results.inner.len(), 1);
        let metrics = results.get("Template").unwrap();
        validate_latency(metrics, 1_000.0, None, None);
    }

    #[test]
    fn adapter_dart_benchmark_harness_two() {
        let results = convert_dart("two");
        validate_adapter_dart_benchmark_harness(&results);
    }

    #[test]
    fn adapter_dart_benchmark_harness_ignores_non_matching_lines() {
        let input = "Some log line\n\
Template(RunTime): 1.0 us.\n\
ignored\n\
ForLoop(RunTime): 2.0 us.\n";
        let results = AdapterDartBenchmarkHarness::parse(input, Settings::default()).unwrap();
        assert_eq!(results.inner.len(), 2);
        validate_latency(results.get("Template").unwrap(), 1_000.0, None, None);
        validate_latency(results.get("ForLoop").unwrap(), 2_000.0, None, None);
    }

    pub fn validate_adapter_dart_benchmark_harness(results: &AdapterResults) {
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("Template").unwrap();
        validate_latency(metrics, 1_000.0, None, None);

        let metrics = results.get("ForLoop").unwrap();
        validate_latency(metrics, 2_000.0, None, None);
    }
}
