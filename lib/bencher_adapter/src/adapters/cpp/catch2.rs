use bencher_json::{BenchmarkName, JsonNewMetric, project::report::JsonAverage};
use nom::{
    IResult,
    character::complete::{anychar, space0, space1},
    combinator::{eof, map, map_res},
    multi::many_till,
    sequence::tuple,
};
use ordered_float::OrderedFloat;

use crate::{
    Adaptable, Settings,
    adapters::util::{
        NomError, Units, latency_as_nanos, parse_number_as_f64, parse_u64, parse_units,
    },
    results::adapter_results::AdapterResults,
};

const CATCH2_METRICS_LINE_COUNT: usize = 5;

pub struct AdapterCppCatch2;

impl Adaptable for AdapterCppCatch2 {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        match settings.average {
            Some(JsonAverage::Mean) | None => {},
            Some(JsonAverage::Median) => return None,
        }

        let mut benchmark_metrics = Vec::new();
        let mut test_case = None;
        let lines = input.lines().collect::<Vec<_>>();
        for lines in lines.windows(CATCH2_METRICS_LINE_COUNT) {
            let Ok(lines) = lines.try_into() else {
                debug_assert!(
                    false,
                    "Windows struct should always be convertible to array of the same size."
                );
                continue;
            };
            if let Some(name) = parse_catch2_test_case(lines) {
                test_case = Some(name);
                continue;
            }
            let Some(name) = test_case.clone() else {
                continue;
            };
            if let Some((benchmark_name, metrics)) = parse_catch2_lines(name, lines) {
                benchmark_metrics.push((benchmark_name, metrics));
            }
        }

        AdapterResults::new_latency(benchmark_metrics)
    }
}

fn parse_catch2_test_case(lines: [&str; CATCH2_METRICS_LINE_COUNT]) -> Option<String> {
    const PAGE_BREAK: &str =
        "-------------------------------------------------------------------------------";

    let [page_break, test_case, l1, l2, l3] = lines;
    if page_break != PAGE_BREAK {
        return None;
    }
    let mut name = test_case.trim().to_owned();
    for l in [l1, l2, l3] {
        if l == PAGE_BREAK {
            return Some(name);
        }
        name.push(' ');
        name.push_str(l.trim());
    }
    None
}

fn parse_catch2_lines(
    mut name: String,
    lines: [&str; CATCH2_METRICS_LINE_COUNT],
) -> Option<(BenchmarkName, JsonNewMetric)> {
    let [prelude_line, mean_line, std_dev_line, ..] = lines;

    let Ok(("", benchmark_name_prelude)) = parse_catch2_prelude_line(prelude_line) else {
        return None;
    };
    name.push_str(": ");
    name.push_str(&benchmark_name_prelude);

    let Ok(("", (benchmark_name_mean, mean))) = parse_catch2_benchmark_time(mean_line) else {
        return None;
    };
    if let Some(benchmark_name_mean) = benchmark_name_mean {
        name.push(' ');
        name.push_str(&benchmark_name_mean);
    }

    let Ok(("", (benchmark_name_std_dev, std_dev))) = parse_catch2_benchmark_time(std_dev_line)
    else {
        return None;
    };
    if let Some(benchmark_name_std_dev) = benchmark_name_std_dev {
        name.push(' ');
        name.push_str(&benchmark_name_std_dev);
    }

    let benchmark_name = name.parse().ok()?;

    let json_metric = JsonNewMetric {
        value: mean,
        lower_value: Some(mean - std_dev),
        upper_value: Some(mean + std_dev),
    };

    Some((benchmark_name, json_metric))
}

fn parse_catch2_prelude_line(input: &str) -> IResult<&str, String> {
    map_res(
        many_till(anychar, parse_catch2_prelude),
        |(name_chars, _)| -> Result<String, NomError> { Ok(name_chars.into_iter().collect()) },
    )(input)
}

fn parse_catch2_benchmark_time(input: &str) -> IResult<&str, (Option<String>, OrderedFloat<f64>)> {
    map_res(
        many_till(anychar, parse_catch2_time),
        |(name_chars, time)| -> Result<(Option<String>, OrderedFloat<f64>), NomError> {
            let name = name_chars.into_iter().collect::<String>();
            let name = (!name.is_empty()).then_some(name);
            Ok((name, time))
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
            parse_number_as_f64,
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

fn parse_catch2_time(input: &str) -> IResult<&str, OrderedFloat<f64>> {
    map(
        tuple((
            space1,
            parse_catch2_duration,
            space1,
            parse_catch2_duration,
            space1,
            parse_catch2_duration,
            space0,
            eof,
        )),
        |(_, column_one, _, _, _, _, _, _)| column_one,
    )(input)
}

fn parse_catch2_duration(input: &str) -> IResult<&str, OrderedFloat<f64>> {
    map_res(
        tuple((parse_number_as_f64, space1, parse_units)),
        |(duration, _, units)| -> Result<OrderedFloat<f64>, NomError> {
            Ok(latency_as_nanos(duration, units))
        },
    )(input)
}

#[cfg(test)]
pub(crate) mod test_cpp_catch2 {
    use bencher_json::project::report::JsonAverage;
    use pretty_assertions::assert_eq;

    use crate::{
        AdapterResults, Settings,
        adapters::test_util::{convert_file_path, opt_convert_file_path, validate_latency},
    };

    use super::{AdapterCppCatch2, parse_catch2_prelude_line};

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
                parse_catch2_prelude_line(input),
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

        let metrics = results.get("Fibonacci: Fibonacci 10").unwrap();
        validate_latency(metrics, 344.0, Some(325.0), Some(363.0));

        let metrics = results.get("Fibonacci: Fibonacci 20").unwrap();
        validate_latency(metrics, 41731.0, Some(38475.0), Some(44987.0));

        let metrics = results.get("More Fibonacci: Fibonacci~ 5!").unwrap();
        validate_latency(metrics, 36.0, Some(32.0), Some(40.0));

        let metrics = results.get("More Fibonacci: Fibonacci-15_bench").unwrap();
        validate_latency(metrics, 3789.0, Some(3427.0), Some(4151.0));
    }

    #[test]
    fn test_adapter_cpp_catch2_two() {
        let results = convert_cpp_catch2("two");
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("Fibonacci: Fibonacci 10").unwrap();
        validate_latency(metrics, 0.0, Some(0.0), Some(0.0));

        let metrics = results.get("Fibonacci: Fibonacci 20").unwrap();
        validate_latency(metrics, 1.0, Some(1.0), Some(1.0));
    }

    #[test]
    fn test_adapter_cpp_catch2_issue_351() {
        let results = convert_cpp_catch2("issue_351");
        assert_eq!(results.inner.len(), 6);

        let metrics = results
            .get("Unit_assignment Construction: Fibonacci 10")
            .unwrap();
        validate_latency(metrics, 344.0, Some(325.0), Some(363.0));

        let metrics = results
            .get("Unit_assignment Construction: Fibonacci 20")
            .unwrap();
        validate_latency(metrics, 41_731.0, Some(38_475.0), Some(44_987.0));

        let metrics = results.get("More Fibonacci: Fibonacci~ 5!").unwrap();
        validate_latency(metrics, 36.0, Some(32.0), Some(40.0));

        let metrics = results.get("More Fibonacci: Fibonacci-15_bench").unwrap();
        validate_latency(metrics, 3_789.0, Some(3_427.0), Some(4_151.0));

        let metrics = results
            .get("Even More Fibonacci With a long name: Fibonacci 10 with a long name")
            .unwrap();
        validate_latency(metrics, 344.0, Some(325.0), Some(363.0));

        let metrics = results
            .get("Even More Fibonacci With a long name: Fibonacci 20")
            .unwrap();
        validate_latency(metrics, 41_731.0, Some(38_475.0), Some(44_987.0));
    }
}
