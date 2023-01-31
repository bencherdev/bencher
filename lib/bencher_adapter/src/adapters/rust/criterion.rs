use bencher_json::{BenchmarkName, JsonMetric};
use nom::{
    bytes::complete::tag,
    character::complete::{anychar, space1},
    combinator::{eof, map, map_res},
    multi::many_till,
    sequence::{delimited, tuple},
    IResult,
};
use ordered_float::OrderedFloat;

use crate::{
    adapters::util::{
        nom_error, parse_benchmark_name, parse_f64, parse_units, time_as_nanos, NomError,
    },
    results::adapter_results::AdapterResults,
    Adapter, AdapterError,
};

pub struct AdapterRustCriterion;

impl Adapter for AdapterRustCriterion {
    fn parse(input: &str) -> Result<AdapterResults, AdapterError> {
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
        many_till(anychar, parse_criterion_time),
        |(name_chars, json_metric)| -> Result<(BenchmarkName, JsonMetric), NomError> {
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
            parse_criterion_metric,
            eof,
        )),
        |(_, json_metric, _)| json_metric,
    )(input)
}

fn parse_criterion_metric(input: &str) -> IResult<&str, JsonMetric> {
    map(
        delimited(
            tag("["),
            tuple((
                parse_criterion_duration,
                space1,
                parse_criterion_duration,
                space1,
                parse_criterion_duration,
            )),
            tag("]"),
        ),
        |(lower_bound, _, value, _, upper_bound)| JsonMetric {
            value,
            lower_bound: Some(lower_bound),
            upper_bound: Some(upper_bound),
        },
    )(input)
}

fn parse_criterion_duration(input: &str) -> IResult<&str, OrderedFloat<f64>> {
    map_res(
        tuple((parse_f64, space1, parse_units)),
        |(duration, _, units)| -> Result<OrderedFloat<f64>, NomError> {
            Ok(time_as_nanos(duration, units))
        },
    )(input)
}

#[cfg(test)]
pub(crate) mod test_rust_criterion {
    use bencher_json::JsonMetric;
    use pretty_assertions::assert_eq;

    use crate::{
        adapters::test_util::{convert_file_path, validate_metrics},
        Adapter, AdapterResults,
    };

    use super::{parse_criterion, AdapterRustCriterion};

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
                            lower_bound: Some(222.2.into()),
                            upper_bound: Some(333.33.into()),
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
                            lower_bound: Some(0.222.into()),
                            upper_bound: Some(0.33333.into()),
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
                            lower_bound: Some(16_652.0.into()),
                            upper_bound: Some(19_562.0.into()),
                        },
                    ),
                )),
                "criterion_benchmark                    time:   [16.652 µs 18.019 µs 19.562 µs]",
            ),
        ]
        .into_iter()
        .enumerate()
        {
            assert_eq!(expected, parse_criterion(None, input), "#{index}: {input}")
        }
    }

    #[test]
    fn test_adapter_rust_criterion() {
        let results = convert_rust_criterion("many");
        validate_adapter_rust_criterion(results);
    }

    pub fn validate_adapter_rust_criterion(results: AdapterResults) {
        assert_eq!(results.inner.len(), 5);

        let metrics = results.get("file").unwrap();
        validate_metrics(metrics, 0.32389999999999997, Some(0.32062), Some(0.32755));

        let metrics = results.get("rolling_file").unwrap();
        validate_metrics(metrics, 0.42966000000000004, Some(0.38179), Some(0.48328));

        let metrics = results.get("tracing_file").unwrap();
        validate_metrics(metrics, 18019.0, Some(16652.0), Some(19562.0));

        let metrics = results.get("tracing_rolling_file").unwrap();
        validate_metrics(metrics, 20930.0, Some(18195.0), Some(24240.0));

        let metrics = results.get("benchmark: name with spaces").unwrap();
        validate_metrics(metrics, 20.930, Some(18.195), Some(24.240));
    }

    #[test]
    fn test_adapter_rust_criterion_failed() {
        let contents = std::fs::read_to_string("./tool_output/rust/criterion/failed.txt").unwrap();
        let results = AdapterRustCriterion::parse(&contents).unwrap();
        assert_eq!(results.inner.len(), 4);
    }

    #[test]
    fn test_adapter_rust_criterion_dogfood() {
        let results = convert_rust_criterion("dogfood");
        assert_eq!(results.inner.len(), 4);

        let metrics = results.get("JsonAdapter::Magic (JSON)").unwrap();
        validate_metrics(
            metrics,
            3463.2000000000003,
            Some(3462.2999999999997),
            Some(3464.1000000000003),
        );

        let metrics = results.get("JsonAdapter::Json").unwrap();
        validate_metrics(metrics, 3479.6, Some(3479.2999999999997), Some(3480.0));

        let metrics = results.get("JsonAdapter::Magic (Rust)").unwrap();
        validate_metrics(metrics, 14726.0, Some(14721.0), Some(14730.0));

        let metrics = results.get("JsonAdapter::Rust").unwrap();
        validate_metrics(metrics, 14884.0, Some(14881.0), Some(14887.0));
    }
}
