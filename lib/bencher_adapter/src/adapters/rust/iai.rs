use bencher_json::{project::report::JsonAverage, BenchmarkName, JsonMetric};
use nom::{
    bytes::complete::tag,
    character::complete::{space0, space1},
    combinator::{eof, map},
    sequence::tuple,
    IResult,
};
use std::collections::HashSet;

use crate::{
    adapters::util::parse_u64,
    results::adapter_results::{AdapterMultiMetricKind, AdapterResults},
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

        AdapterResults::new_multi(benchmark_metrics)
    }
}

fn parse_iai_lines(
    lines: [&str; IAI_METRICS_LINE_COUNT],
) -> Option<(BenchmarkName, HashSet<AdapterMultiMetricKind>)> {
    let [benchmark_name_line, instructions_line, l1_accesses_line, l2_accesses_line, ram_accesses_line, cycles_line] =
        lines;

    let metrics = [
        (
            "Instructions:",
            instructions_line,
            AdapterMultiMetricKind::Instructions as fn(JsonMetric) -> AdapterMultiMetricKind,
        ),
        (
            "L1 Accesses:",
            l1_accesses_line,
            AdapterMultiMetricKind::L1Accesses,
        ),
        (
            "L2 Accesses:",
            l2_accesses_line,
            AdapterMultiMetricKind::L2Accesses,
        ),
        (
            "RAM Accesses:",
            ram_accesses_line,
            AdapterMultiMetricKind::RamAccesses,
        ),
        (
            "Estimated Cycles:",
            cycles_line,
            AdapterMultiMetricKind::Cycles,
        ),
    ]
    .into_iter()
    .map(|(header, input, to_variant)| {
        parse_iai_metric(input, header).map(|metric| to_variant(metric.1))
    })
    .collect::<Result<HashSet<_>, _>>()
    .ok()?;
    let name = benchmark_name_line.parse().ok()?;
    Some((name, metrics))
}

fn parse_iai_metric<'a>(input: &'a str, header: &'static str) -> IResult<&'a str, JsonMetric> {
    map(
        tuple((space0, tag(header), space1, parse_u64, eof)),
        |(_, _, _, metric, _)| JsonMetric {
            value: (metric as f64).into(),
            lower_bound: None,
            upper_bound: None,
        },
    )(input)
}

#[cfg(test)]
pub(crate) mod test_rust_iai {

    use crate::Adapter;
    use bencher_json::JsonMetric;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_parse_line() {
        assert_eq!(
            super::parse_iai_metric("  Instructions:  1234", "Instructions:"),
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
