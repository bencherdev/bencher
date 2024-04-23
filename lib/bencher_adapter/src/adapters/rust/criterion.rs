use bencher_json::{project::report::JsonAverage, BenchmarkName, JsonMetric};
use nom::{
    bytes::complete::tag,
    character::complete::{anychar, space1, space0},
    combinator::{eof, map, map_res},
    multi::many_till,
    sequence::{delimited, tuple},
    IResult,
};
use ordered_float::OrderedFloat;

use crate::{
    adapters::util::{
        latency_as_nanos, nom_error, parse_benchmark_name, parse_f64, parse_units, NomError,
    },
    results::adapter_results::AdapterResults,
    Adaptable, Settings,
};

pub struct AdapterRustCriterion;

impl Adaptable for AdapterRustCriterion {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        match settings.average {
            Some(JsonAverage::Mean) | None => {},
            Some(JsonAverage::Median) => return None,
        }

        let mut benchmark_metrics = Vec::new();

        let mut prior_line = None;
        for line in input.lines() {
            if let Ok((remainder, benchmark_metric)) = parse_criterion(prior_line, line) {
                if remainder.is_empty() {
                    benchmark_metrics.push(benchmark_metric);
                }
            }

            prior_line = Some(line);
        }

        AdapterResults::new_latency(benchmark_metrics)
    }
}

fn parse_criterion<'i>(
    prior_line: Option<&str>,
    input: &'i str,
) -> IResult<&'i str, (BenchmarkName, JsonMetric)> {
    map_res(
        many_till(anychar, tuple((parse_criterion_time, nom::combinator::opt(tuple((nom::character::complete::multispace1, parse_criterion_throughput)))))),
        |(name_chars, (json_metric, throuput))| -> Result<(BenchmarkName, JsonMetric), NomError> {
            dbg!(throuput);
            let name: String = if name_chars.is_empty() {
                prior_line.ok_or_else(|| nom_error(String::new()))?.into()
            } else {
                name_chars.into_iter().collect()
            };
            let benchmark_name = parse_benchmark_name(&name)?;
            Ok((benchmark_name, json_metric))
        },
    )(input)
}

fn parse_criterion_time(input: &str) -> IResult<&str, JsonMetric> {
    map(
        tuple((
            tuple((space1, tag("time:"), space1)),
            parse_criterion_metric(parse_criterion_duration),
            eof,
        )),
        |(_, json_metric, _)| json_metric,
    )(input)
}

fn parse_criterion_throughput(input: &str) -> IResult<&str, JsonMetric> {
    dbg!(input);
    map(
        tuple((
            tuple((space0, tag("thrpt:"), space1)),
            parse_criterion_metric(parse_criterion_elements),
            eof,
        )),
        |(_, metric, _)| metric,
    )(input)
}

fn parse_criterion_metric<'i, P>(mut part: P) -> impl FnMut(&'i str) -> IResult<&'i str, JsonMetric> where P: FnMut(&'i str) -> IResult<&'i str, OrderedFloat<f64>> + Copy {
    move |input| {
        let (input, _) = tag("[")(input)?;

        let (input, lower_value) = part(input)?;
        let (input, _) = space1(input)?;
        let (input, value) = part(input)?;
        let (input, _) = space1(input)?;
        let (input, upper_value) = part(input)?;

        let (input, _) = tag("]")(input)?;
       
        Ok((input, JsonMetric {
            value,
            lower_value: Some(lower_value),
            upper_value: Some(upper_value),
        }))
    }
}

fn parse_criterion_duration(input: &str) -> IResult<&str, OrderedFloat<f64>> {
    map(
        tuple((parse_f64, space1, parse_units)),
        |(duration, _, units)| latency_as_nanos(duration, units),
    )(input)
}

fn parse_criterion_elements(input: &str) -> IResult<&str, OrderedFloat<f64>> {
    map(
        tuple((
            parse_f64, space1, nom::branch::alt((
                map(tag("Melem/s"), |_| 1000 * 1000),
                map(tag("Kelem/s"), |_| 1000),
                map(tag("elem/s"), |_| 1),
            )),
        )),
        |(base, _, multiplier)| OrderedFloat(base * multiplier as f64),
    )(input)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(crate) mod test_rust_criterion {
    use bencher_json::{project::report::JsonAverage, JsonMetric};
    use pretty_assertions::assert_eq;
    use ordered_float::OrderedFloat;

    use crate::{
        adapters::test_util::{convert_file_path, opt_convert_file_path, validate_latency},
        Adaptable, AdapterResults, Settings,
    };

    use super::{parse_criterion, parse_criterion_throughput, AdapterRustCriterion};

    fn convert_rust_criterion(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/rust/criterion/{suffix}.txt");
        convert_file_path::<AdapterRustCriterion>(&file_path)
    }

    #[test]
    fn test_parse_criterion() {
        for (index, (expected, input)) in [
            (
                Ok((
                    "",
                    (
                        "criterion_benchmark".parse().unwrap(),
                        JsonMetric {
                            value: 280.0.into(),
                            lower_value: Some(222.2.into()),
                            upper_value: Some(333.33.into()),
                        },
                    ),
                )),
                "criterion_benchmark                    time:   [222.2 ns 280.0 ns 333.33 ns]",
            ),
            (
                Ok((
                    "",
                    (
                        "criterion_benchmark".parse().unwrap(),
                        JsonMetric {
                            value: 5.280.into(),
                            lower_value: Some(0.222.into()),
                            upper_value: Some(0.33333.into()),
                        },
                    ),
                )),
                "criterion_benchmark                    time:   [222.0 ps 5,280.0 ps 333.33 ps]",
            ),
            (
                Ok((
                    "",
                    (
                        "criterion_benchmark".parse().unwrap(),
                        JsonMetric {
                            value: 18_019.0.into(),
                            lower_value: Some(16_652.0.into()),
                            upper_value: Some(19_562.0.into()),
                        },
                    ),
                )),
                "criterion_benchmark                    time:   [16.652 µs 18.019 µs 19.562 µs]",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(expected, parse_criterion(None, input), "#{index}: {input}");
        }
    }

    #[test]
    fn test_adapter_rust_criterion_average() {
        let file_path = "./tool_output/rust/criterion/many.txt";
        let results = opt_convert_file_path::<AdapterRustCriterion>(
            file_path,
            Settings {
                average: Some(JsonAverage::Mean),
            },
        )
        .unwrap();
        validate_adapter_rust_criterion(&results);

        assert_eq!(
            None,
            opt_convert_file_path::<AdapterRustCriterion>(
                file_path,
                Settings {
                    average: Some(JsonAverage::Median)
                }
            )
        );
    }

    #[test]
    fn test_adapter_rust_criterion() {
        let results = convert_rust_criterion("many");
        validate_adapter_rust_criterion(&results);
    }

    pub fn validate_adapter_rust_criterion(results: &AdapterResults) {
        assert_eq!(results.inner.len(), 5);

        let metrics = results.get("file").unwrap();
        validate_latency(
            metrics,
            0.323_899_999_999_999_97,
            Some(0.32062),
            Some(0.32755),
        );

        let metrics = results.get("rolling_file").unwrap();
        validate_latency(
            metrics,
            0.429_660_000_000_000_04,
            Some(0.38179),
            Some(0.48328),
        );

        let metrics = results.get("tracing_file").unwrap();
        validate_latency(metrics, 18019.0, Some(16652.0), Some(19562.0));

        let metrics = results.get("tracing_rolling_file").unwrap();
        validate_latency(metrics, 20930.0, Some(18195.0), Some(24240.0));

        let metrics = results.get("benchmark: name with spaces").unwrap();
        validate_latency(metrics, 20.930, Some(18.195), Some(24.240));
    }

    #[test]
    fn test_adapter_rust_criterion_failed() {
        let contents = std::fs::read_to_string("./tool_output/rust/criterion/failed.txt").unwrap();
        let results = AdapterRustCriterion::parse(&contents, Settings::default()).unwrap();
        assert_eq!(results.inner.len(), 4);
    }

    #[test]
    fn test_adapter_rust_criterion_dogfood() {
        let results = convert_rust_criterion("dogfood");
        assert_eq!(results.inner.len(), 4);

        let metrics = results.get("Adapter::Magic (JSON)").unwrap();
        validate_latency(
            metrics,
            3_463.200_000_000_000_3,
            Some(3_462.299_999_999_999_7),
            Some(3_464.100_000_000_000_3),
        );

        let metrics = results.get("Adapter::Json").unwrap();
        validate_latency(metrics, 3479.6, Some(3_479.299_999_999_999_7), Some(3480.0));

        let metrics = results.get("Adapter::Magic (Rust)").unwrap();
        validate_latency(metrics, 14726.0, Some(14721.0), Some(14730.0));

        let metrics = results.get("Adapter::Rust").unwrap();
        validate_latency(metrics, 14884.0, Some(14881.0), Some(14887.0));
    }

    #[test]
    fn parse_throughputs() {
        let (_, tmp) = parse_criterion_throughput("thrpt:  [3.2268 Melem/s 3.2314 Melem/s 3.2352 Melem/s]").unwrap();
        assert_eq!(JsonMetric {
            value: OrderedFloat(3231400.0),
            lower_value: Some(OrderedFloat(3226800.0)),
            upper_value: Some(OrderedFloat(3235200.0)),
        }, tmp);
    }
}
