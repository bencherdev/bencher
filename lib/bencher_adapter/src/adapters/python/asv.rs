use bencher_json::{project::report::JsonAverage, BenchmarkName, JsonMetric};
use nom::{
    bytes::complete::{tag, take_until1, take_while1},
    character::complete::{space0, space1},
    combinator::{eof, map, map_res},
    sequence::{delimited, tuple},
    IResult,
};

use crate::{
    adapters::util::{
        latency_as_nanos, nom_error, parse_benchmark_name, parse_f64, parse_units, NomError,
    },
    results::adapter_results::AdapterResults,
    Adapter, Settings,
};

pub struct AdapterPythonAsv;

impl Adapter for AdapterPythonAsv {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        match settings.average {
            Some(JsonAverage::Median) | None => {},
            Some(JsonAverage::Mean) => return None,
        }

        let mut benchmark_metrics = Vec::new();

        for line in input.lines() {
            if let Ok((remainder, benchmark_metric)) = parse_asv(line) {
                if remainder.is_empty() {
                    benchmark_metrics.push(benchmark_metric);
                }
            }
        }

        AdapterResults::new_latency(benchmark_metrics)
    }
}

fn parse_asv(input: &str) -> IResult<&str, (BenchmarkName, JsonMetric)> {
    map_res(
        tuple((
            tuple((
                delimited(tag("["), tuple((space0, parse_f64, tag("%"))), tag("]")),
                space1,
                take_while1(|c| c == '·'),
                space1,
            )),
            take_until1(" "),
            space1,
            parse_asv_time,
        )),
        |(_, name, _, json_metric)| -> Result<(BenchmarkName, JsonMetric), NomError> {
            if name.is_empty() {
                return Err(nom_error(String::new()));
            }
            let benchmark_name = parse_benchmark_name(name)?;
            Ok((benchmark_name, json_metric))
        },
    )(input)
}

fn parse_asv_time(input: &str) -> IResult<&str, JsonMetric> {
    map(
        tuple((parse_f64, tag("±"), parse_f64, parse_units, eof)),
        |(duration, _, range, units, _)| {
            let value = latency_as_nanos(duration, units);
            let bound = latency_as_nanos(range, units);
            JsonMetric {
                value,
                lower_bound: Some(value - bound),
                upper_bound: Some(value + bound),
            }
        },
    )(input)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(crate) mod test_python_asv {
    use bencher_json::project::report::JsonAverage;
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::test_util::{convert_file_path, opt_convert_file_path, validate_latency},
        AdapterResults, Settings,
    };

    use super::AdapterPythonAsv;

    fn convert_python_asv(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/python/asv/{suffix}.txt");
        convert_file_path::<AdapterPythonAsv>(&file_path)
    }

    #[test]
    fn test_adapter_python_asv_average() {
        let file_path = "./tool_output/python/asv/six.txt";
        assert_eq!(
            None,
            opt_convert_file_path::<AdapterPythonAsv>(
                file_path,
                Settings {
                    average: Some(JsonAverage::Mean)
                }
            )
        );

        let results = opt_convert_file_path::<AdapterPythonAsv>(
            file_path,
            Settings {
                average: Some(JsonAverage::Median),
            },
        )
        .unwrap();
        validate_adapter_python_asv(&results);
    }

    #[test]
    fn test_adapter_python_asv() {
        let results = convert_python_asv("six");
        validate_adapter_python_asv(&results);
    }

    pub fn validate_adapter_python_asv(results: &AdapterResults) {
        assert_eq!(results.inner.len(), 6);

        let metrics = results.get("benchmarks.TimeSuite.time_iterkeys").unwrap();
        validate_latency(metrics, 11100.0, Some(11090.0), Some(11110.0));

        let metrics = results.get("benchmarks.TimeSuite.time_keys").unwrap();
        validate_latency(metrics, 11200.0, Some(11190.0), Some(11210.0));

        let metrics = results.get("benchmarks.TimeSuite.time_range").unwrap();
        validate_latency(metrics, 32900.0, Some(32890.0), Some(32910.0));

        let metrics = results.get("benchmarks.TimeSuite.time_xrange").unwrap();
        validate_latency(metrics, 30300.0, Some(30290.0), Some(30310.0));

        let metrics = results.get("benchmarks.TimeSuite.time_keys3").unwrap();
        validate_latency(metrics, 9070.0, Some(8570.0), Some(9570.0));

        let metrics = results.get("benchmarks.TimeSuite.time_range3").unwrap();
        validate_latency(metrics, 35500.0, Some(35490.0), Some(35510.0));
    }
}
