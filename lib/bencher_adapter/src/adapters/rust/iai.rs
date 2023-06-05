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
    results::adapter_results::{AdapterMetricKindIai, AdapterResults},
    Adapter, Settings,
};

pub struct AdapterRustIai;

const IAI_METRICS_LINE_COUNT: usize = 6;

impl Adapter for AdapterRustIai {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        match settings.average {
            None => {},
            Some(JsonAverage::Mean) | Some(JsonAverage::Median) => return None,
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
) -> Option<(BenchmarkName, Vec<AdapterMetricKindIai>)> {
    let [benchmark_name_line, instructions_line, l1_accesses_line, l2_accesses_line, ram_accesses_line, estimated_cycles_line] =
        lines;

    let name = benchmark_name_line.parse().ok()?;
    let metrics = [
        (
            INSTRUCTIONS_NAME_STR,
            instructions_line,
            AdapterMetricKindIai::Instructions as fn(JsonMetric) -> AdapterMetricKindIai,
        ),
        (
            L1_ACCESSES_NAME_STR,
            l1_accesses_line,
            AdapterMetricKindIai::L1Accesses,
        ),
        (
            L2_ACCESSES_NAME_STR,
            l2_accesses_line,
            AdapterMetricKindIai::L2Accesses,
        ),
        (
            RAM_ACCESSES_NAME_STR,
            ram_accesses_line,
            AdapterMetricKindIai::RamAccesses,
        ),
        (
            ESTIMATED_CYCLES_NAME_STR,
            estimated_cycles_line,
            AdapterMetricKindIai::EstimatedCycles,
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
                                map(tuple((parse_f64, tag("%"))), |_| ()),
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

    use crate::Adapter;
    use bencher_json::{project::metric_kind::INSTRUCTIONS_NAME_STR, JsonMetric};
    use pretty_assertions::assert_eq;

    #[test]
    fn test_parse_line() {
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
            super::parse_iai_metric("  Instructions:  1234 (3.14%)", INSTRUCTIONS_NAME_STR),
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
    fn test_parse_multiple_lines() {
        let input = "bench_fibonacci_short
  Instructions:                1735
  L1 Accesses:                 2364
  L2 Accesses:                    1
  RAM Accesses:                   1
  Estimated Cycles:            2404";
        let output = super::AdapterRustIai::parse(input, crate::Settings::default());
        assert!(output.is_some());
    }
}
