use bencher_json::{project::report::JsonAverage, BenchmarkName, JsonNewMetric};

use nom::{
    bytes::complete::tag,
    character::complete::{anychar, space1},
    combinator::{eof, map, map_res},
    multi::many_till,
    sequence::tuple,
    IResult,
};

use crate::{
    adapters::util::{
        latency_as_nanos, nom_error, parse_benchmark_name_chars, parse_u64, parse_units, NomError,
    },
    results::adapter_results::AdapterResults,
    Adaptable, Settings,
};

pub struct AdapterJsTime;

impl Adaptable for AdapterJsTime {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        if let Some(JsonAverage::Mean | JsonAverage::Median) = settings.average {
            return None;
        }

        let mut benchmark_metrics = Vec::new();

        for line in input.lines() {
            if let Ok((remainder, benchmark_metric)) = parse_time(line) {
                if remainder.is_empty() {
                    benchmark_metrics.push(benchmark_metric);
                }
            }
        }

        AdapterResults::new_latency(benchmark_metrics)
    }
}

fn parse_time(input: &str) -> IResult<&str, (BenchmarkName, JsonNewMetric)> {
    map_res(
        many_till(anychar, parse_time_time),
        |(name_chars, json_metric)| -> Result<(BenchmarkName, JsonNewMetric), NomError> {
            if name_chars.is_empty() {
                return Err(nom_error(String::new()));
            }
            let benchmark_name = parse_benchmark_name_chars(&name_chars)?;
            Ok((benchmark_name, json_metric))
        },
    )(input)
}

fn parse_time_time(input: &str) -> IResult<&str, JsonNewMetric> {
    map(
        tuple((
            tuple((tag(":"), space1)),
            parse_u64,
            parse_units,
            tuple((
                space1,
                tag("-"),
                space1,
                tag("timer"),
                space1,
                tag("ended"),
                eof,
            )),
        )),
        |(_, duration, units, _)| {
            let value = latency_as_nanos(duration, units);
            JsonNewMetric {
                value,
                lower_value: None,
                upper_value: None,
            }
        },
    )(input)
}

#[cfg(test)]
pub(crate) mod test_js_time {
    use bencher_json::project::report::JsonAverage;
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::test_util::{convert_file_path, opt_convert_file_path, validate_latency},
        AdapterResults, Settings,
    };

    use super::AdapterJsTime;

    fn convert_js_time(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/js/time/{suffix}.txt");
        convert_file_path::<AdapterJsTime>(&file_path)
    }

    #[test]
    fn test_adapter_js_time_average() {
        let file_path = "./tool_output/js/time/four.txt";
        assert_eq!(
            None,
            opt_convert_file_path::<AdapterJsTime>(
                file_path,
                Settings {
                    average: Some(JsonAverage::Mean)
                }
            )
        );

        assert_eq!(
            None,
            opt_convert_file_path::<AdapterJsTime>(
                file_path,
                Settings {
                    average: Some(JsonAverage::Median)
                }
            )
        );
    }

    #[test]
    fn test_adapter_js_time() {
        let results = convert_js_time("four");
        validate_adapter_js_time(&results);
    }

    pub fn validate_adapter_js_time(results: &AdapterResults) {
        assert_eq!(results.inner.len(), 4);

        let metrics = results.get("default").unwrap();
        validate_latency(metrics, 7_617_000_000.0, None, None);

        let metrics = results.get("benchmark_1").unwrap();
        validate_latency(metrics, 12_714_000_000.0, None, None);

        let metrics = results.get("benchmark 2").unwrap();
        validate_latency(metrics, 9_034_000_000.0, None, None);

        let metrics = results
            .get("benchmark 3: The Third - timer/timerEnd")
            .unwrap();
        validate_latency(metrics, 8_827_000_000.0, None, None);
    }
}
