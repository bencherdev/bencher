use bencher_json::{project::report::JsonAverage, BenchmarkName, JsonMetric};
use nom::{
    character::complete::{anychar, space0, space1},
    combinator::{eof, map, map_res},
    multi::many_till,
    sequence::tuple,
    IResult,
};
use ordered_float::OrderedFloat;

use crate::{
    adapters::util::{
        latency_as_nanos, nom_error, parse_benchmark_name_chars, parse_f64, parse_u64, parse_units,
        NomError, Units,
    },
    results::adapter_results::AdapterResults,
    Adapter, Settings,
};

pub struct AdapterCppCatch2;

impl Adapter for AdapterCppCatch2 {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        match settings.average {
            Some(JsonAverage::Mean) | None => {},
            Some(JsonAverage::Median) => return None,
        }

        let mut benchmark_metrics = Vec::new();

        let mut benchmark_name_line = None;
        let mut mean_line = None;
        for line in input.lines() {
            if let Some(benchmark_name_line) = benchmark_name_line {
                if let Some(mean_line) = mean_line {
                    if let Ok((remainder, benchmark_metric)) =
                        parse_catch2(benchmark_name_line, mean_line, line)
                    {
                        if remainder.is_empty() {
                            benchmark_metrics.push(benchmark_metric);
                        }
                    }
                }
            }

            benchmark_name_line = mean_line.replace(line);
        }

        AdapterResults::new_latency(benchmark_metrics)
    }
}

fn parse_catch2<'i>(
    benchmark_name_line: &str,
    mean_line: &str,
    input: &'i str,
) -> IResult<&'i str, (BenchmarkName, JsonMetric)> {
    map_res(
        parse_catch2_time,
        |std_dev| -> Result<(BenchmarkName, JsonMetric), NomError> {
            #[allow(clippy::map_err_ignore)]
            let (benchmark_name_remainder, benchmark_name) =
                parse_catch2_benchmark_name(benchmark_name_line)
                    .map_err(|_| nom_error(benchmark_name_line))?;

            #[allow(clippy::map_err_ignore)]
            let (mean_remainder, mean) =
                parse_catch2_time(mean_line).map_err(|_| nom_error(mean_line))?;

            if benchmark_name_remainder.is_empty() && mean_remainder.is_empty() {
                let json_metric = JsonMetric {
                    value: mean,
                    lower_bound: Some(mean - std_dev),
                    upper_bound: Some(mean + std_dev),
                };

                Ok((benchmark_name, json_metric))
            } else {
                Err(nom_error(input))
            }
        },
    )(input)
}

fn parse_catch2_time(input: &str) -> IResult<&str, OrderedFloat<f64>> {
    map(
        tuple((
            space1,
            parse_catch2_duration,
            space1,
            parse_catch2_duration,
            space1,
            parse_catch2_duration,
            space1,
            eof,
        )),
        |(_, column_one, _, _, _, _, _, _)| column_one,
    )(input)
}

fn parse_catch2_duration(input: &str) -> IResult<&str, OrderedFloat<f64>> {
    map_res(
        tuple((parse_f64, space1, parse_units)),
        |(duration, _, units)| -> Result<OrderedFloat<f64>, NomError> {
            Ok(latency_as_nanos(duration, units))
        },
    )(input)
}

fn parse_catch2_benchmark_name(input: &str) -> IResult<&str, BenchmarkName> {
    map_res(
        many_till(anychar, parse_catch2_prelude),
        |(name_chars, _)| -> Result<BenchmarkName, nom::Err<nom::error::Error<String>>> {
            parse_benchmark_name_chars(&name_chars)
        },
    )(input)
}

#[allow(dead_code)]
struct Prelude {
    samples: u64,
    iterations: u64,
    estimated: f64,
    estimated_units: Units,
}

fn parse_catch2_prelude(input: &str) -> IResult<&str, Prelude> {
    map(
        tuple((
            space1,
            parse_u64,
            space1,
            parse_u64,
            space1,
            parse_f64,
            space1,
            parse_units,
            space0,
            eof,
        )),
        |(_, samples, _, iterations, _, estimated, _, estimated_units, _, _)| Prelude {
            samples,
            iterations,
            estimated,
            estimated_units,
        },
    )(input)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(crate) mod test_cpp_catch2 {
    use bencher_json::project::report::JsonAverage;
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::test_util::{convert_file_path, opt_convert_file_path, validate_latency},
        AdapterResults, Settings,
    };

    use super::{parse_catch2_benchmark_name, AdapterCppCatch2};

    fn convert_cpp_catch2(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/cpp/catch2/{suffix}.txt");
        convert_file_path::<AdapterCppCatch2>(&file_path)
    }

    #[test]
    fn test_parse_benchmark_name() {
        for (index, (expected, input)) in [
            (
                Ok(("", "Fibonacci 10".parse().unwrap())),
                "Fibonacci 10                                              100           208     7.1968 ms ",
            ),
            (
                Ok(("", "Fibonacci 20".parse().unwrap())),
                "Fibonacci 20                                              100             2     8.3712 ms ",
            ),
            (
                Ok(("", "Fibonacci~ 5!".parse().unwrap())),
                "Fibonacci~ 5!                                             100          1961     7.0596 ms ",
            ),
            (
                Ok(("", "Fibonacci-15_bench".parse().unwrap())),
                "Fibonacci-15_bench                                        100            20       7.48 ms ",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(
                expected,
                parse_catch2_benchmark_name(input),
                "#{index}: {input}"
            );
        }
    }

    #[test]
    fn test_adapter_cpp_catch2_average() {
        let file_path = "./tool_output/cpp/catch2/four.txt";
        let results = opt_convert_file_path::<AdapterCppCatch2>(
            file_path,
            Settings {
                average: Some(JsonAverage::Mean),
            },
        )
        .unwrap();
        validate_adapter_cpp_catch2(&results);

        assert_eq!(
            None,
            opt_convert_file_path::<AdapterCppCatch2>(
                file_path,
                Settings {
                    average: Some(JsonAverage::Median)
                }
            )
        );
    }

    #[test]
    fn test_adapter_cpp_catch2() {
        let results = convert_cpp_catch2("four");
        validate_adapter_cpp_catch2(&results);
    }

    pub fn validate_adapter_cpp_catch2(results: &AdapterResults) {
        assert_eq!(results.inner.len(), 4);

        let metrics = results.get("Fibonacci 10").unwrap();
        validate_latency(metrics, 344.0, Some(325.0), Some(363.0));

        let metrics = results.get("Fibonacci 20").unwrap();
        validate_latency(metrics, 41731.0, Some(38475.0), Some(44987.0));

        let metrics = results.get("Fibonacci~ 5!").unwrap();
        validate_latency(metrics, 36.0, Some(32.0), Some(40.0));

        let metrics = results.get("Fibonacci-15_bench").unwrap();
        validate_latency(metrics, 3789.0, Some(3427.0), Some(4151.0));
    }

    #[test]
    fn test_adapter_cpp_catch2_two() {
        let results = convert_cpp_catch2("two");
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("Fibonacci 10").unwrap();
        validate_latency(metrics, 0.0, Some(0.0), Some(0.0));

        let metrics = results.get("Fibonacci 20").unwrap();
        validate_latency(metrics, 1.0, Some(1.0), Some(1.0));
    }
}
