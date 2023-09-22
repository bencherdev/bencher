use bencher_json::{
    project::{
        metric_kind::{
            ESTIMATED_CYCLES_NAME_STR, INSTRUCTIONS_NAME_STR, L1_ACCESSES_NAME_STR,
            L2_ACCESSES_NAME_STR, RAM_ACCESSES_NAME_STR,
        },
        report::JsonAverage,
    },
    BenchmarkName, JsonMetric,
};
use nom::{
    branch::alt,
    bytes::complete::tag,
    character::complete::{space0, space1},
    combinator::{eof, map},
    sequence::{delimited, tuple},
    IResult,
};

use crate::{
    adapters::util::{parse_f64, parse_u64},
    results::adapter_results::{AdapterResults, IaiMetricKind},
    Adapter, Settings,
};

pub struct AdapterRustIai;

const IAI_METRICS_LINE_COUNT: usize = 6;

impl Adapter for AdapterRustIai {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        match settings.average {
            None => {},
            Some(JsonAverage::Mean | JsonAverage::Median) => return None,
        }

        let mut benchmark_metrics = Vec::new();
        let lines = input.lines().collect::<Vec<_>>();
        for lines in lines.windows(IAI_METRICS_LINE_COUNT) {
            let lines = lines
                .try_into()
                .expect("Windows struct should always be convertible to array of the same size.");
            if let Some((benchmark_name, metrics)) = parse_iai_lines(lines) {
                benchmark_metrics.push((benchmark_name, metrics));
            }
        }

        AdapterResults::new_iai(benchmark_metrics)
    }
}

fn parse_iai_lines(
    lines: [&str; IAI_METRICS_LINE_COUNT],
) -> Option<(BenchmarkName, Vec<IaiMetricKind>)> {
    let [benchmark_name_line, instructions_line, l1_accesses_line, l2_accesses_line, ram_accesses_line, estimated_cycles_line] =
        lines;

    let name = benchmark_name_line.parse().ok()?;
    let metrics = [
        (
            INSTRUCTIONS_NAME_STR,
            instructions_line,
            IaiMetricKind::Instructions as fn(JsonMetric) -> IaiMetricKind,
        ),
        (
            L1_ACCESSES_NAME_STR,
            l1_accesses_line,
            IaiMetricKind::L1Accesses,
        ),
        (
            L2_ACCESSES_NAME_STR,
            l2_accesses_line,
            IaiMetricKind::L2Accesses,
        ),
        (
            RAM_ACCESSES_NAME_STR,
            ram_accesses_line,
            IaiMetricKind::RamAccesses,
        ),
        (
            ESTIMATED_CYCLES_NAME_STR,
            estimated_cycles_line,
            IaiMetricKind::EstimatedCycles,
        ),
    ]
    .into_iter()
    .map(|(metric_kind, input, into_variant)| {
        parse_iai_metric(input, metric_kind)
            .map(|(_remainder, json_metric)| into_variant(json_metric))
    })
    .collect::<Result<Vec<_>, _>>()
    .ok()?;

    Some((name, metrics))
}

fn parse_iai_metric<'a>(input: &'a str, metric_kind: &'static str) -> IResult<&'a str, JsonMetric> {
    map(
        tuple((
            space0,
            tag(metric_kind),
            tag(":"),
            space1,
            parse_u64,
            alt((
                map(eof, |_| ()),
                map(
                    tuple((
                        space1,
                        delimited(
                            tag("("),
                            alt((
                                map(tag("No change"), |_| ()),
                                map(
                                    tuple((alt((tag("+"), tag("-"))), parse_f64, tag("%"))),
                                    |_| (),
                                ),
                            )),
                            tag(")"),
                        ),
                        eof,
                    )),
                    |_| (),
                ),
            )),
        )),
        |(_, _, _, _, metric, _)| JsonMetric {
            value: (metric as f64).into(),
            lower_bound: None,
            upper_bound: None,
        },
    )(input)
}

#[cfg(test)]
pub(crate) mod test_rust_iai {

    use crate::{
        adapters::test_util::convert_file_path, results::adapter_metrics::AdapterMetrics, Adapter,
        AdapterResults,
    };
    use bencher_json::{
        project::metric_kind::{
            ESTIMATED_CYCLES_SLUG_STR, INSTRUCTIONS_NAME_STR, INSTRUCTIONS_SLUG_STR,
            L1_ACCESSES_SLUG_STR, L2_ACCESSES_SLUG_STR, RAM_ACCESSES_SLUG_STR,
        },
        JsonMetric,
    };
    use ordered_float::OrderedFloat;
    use pretty_assertions::assert_eq;

    use super::AdapterRustIai;

    fn convert_rust_iai(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/rust/iai/{suffix}.txt");
        convert_file_path::<AdapterRustIai>(&file_path)
    }

    pub fn validate_iai(metrics: &AdapterMetrics, results: [(&str, f64); 5]) {
        assert_eq!(metrics.inner.len(), 5);
        for (key, value) in results {
            let metric = metrics.get(key).unwrap();
            assert_eq!(metric.value, OrderedFloat::from(value));
            assert_eq!(metric.lower_bound, None);
            assert_eq!(metric.upper_bound, None);
        }
    }

    #[test]
    fn test_adapter_rust_iai_parse_line() {
        assert_eq!(
            super::parse_iai_metric("  Instructions:  1234", INSTRUCTIONS_NAME_STR),
            Ok((
                "",
                JsonMetric {
                    value: 1234.0.into(),
                    upper_bound: None,
                    lower_bound: None
                }
            ))
        );

        assert_eq!(
            super::parse_iai_metric("  Instructions:  1234 (No change)", INSTRUCTIONS_NAME_STR),
            Ok((
                "",
                JsonMetric {
                    value: 1234.0.into(),
                    upper_bound: None,
                    lower_bound: None
                }
            ))
        );

        assert_eq!(
            super::parse_iai_metric("  Instructions:  1234 (+3.14%)", INSTRUCTIONS_NAME_STR),
            Ok((
                "",
                JsonMetric {
                    value: 1234.0.into(),
                    upper_bound: None,
                    lower_bound: None
                }
            ))
        );
    }

    #[test]
    fn test_adapter_rust_iai_parse_multiple_lines() {
        let input = "bench_fibonacci_short
  Instructions:                1735
  L1 Accesses:                 2364
  L2 Accesses:                    1
  RAM Accesses:                   1
  Estimated Cycles:            2404";
        let output = super::AdapterRustIai::parse(input, crate::Settings::default());
        assert!(output.is_some());
    }

    #[test]
    fn test_adapter_rust_aia() {
        let results = convert_rust_iai("two");
        validate_adapter_rust_iai(results);
    }

    pub fn validate_adapter_rust_iai(results: AdapterResults) {
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("bench_fibonacci_short").unwrap();
        validate_iai(
            metrics,
            [
                (INSTRUCTIONS_SLUG_STR, 1735.0),
                (L1_ACCESSES_SLUG_STR, 2364.0),
                (L2_ACCESSES_SLUG_STR, 1.0),
                (RAM_ACCESSES_SLUG_STR, 1.0),
                (ESTIMATED_CYCLES_SLUG_STR, 2404.0),
            ],
        );
        let metrics = results.get("bench_fibonacci_long").unwrap();
        validate_iai(
            metrics,
            [
                (INSTRUCTIONS_SLUG_STR, 26_214_735.0),
                (L1_ACCESSES_SLUG_STR, 35_638_623.0),
                (L2_ACCESSES_SLUG_STR, 2.0),
                (RAM_ACCESSES_SLUG_STR, 1.0),
                (ESTIMATED_CYCLES_SLUG_STR, 35_638_668.0),
            ],
        );
    }

    #[test]
    fn test_adapter_rust_aia_change() {
        let results = convert_rust_iai("change");
        assert_eq!(results.inner.len(), 2);

        let metrics = results.get("iai_benchmark_short").unwrap();
        validate_iai(
            metrics,
            [
                (INSTRUCTIONS_SLUG_STR, 1243.0),
                (L1_ACCESSES_SLUG_STR, 1580.0),
                (L2_ACCESSES_SLUG_STR, 1.0),
                (RAM_ACCESSES_SLUG_STR, 2.0),
                (ESTIMATED_CYCLES_SLUG_STR, 1655.0),
            ],
        );
        let metrics = results.get("iai_benchmark_long").unwrap();
        validate_iai(
            metrics,
            [
                (INSTRUCTIONS_SLUG_STR, 18_454_953.0),
                (L1_ACCESSES_SLUG_STR, 23_447_195.0),
                (L2_ACCESSES_SLUG_STR, 6.0),
                (RAM_ACCESSES_SLUG_STR, 2.0),
                (ESTIMATED_CYCLES_SLUG_STR, 23_447_295.0),
            ],
        );
    }
}
