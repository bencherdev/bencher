use bencher_json::{project::report::JsonAverage, BenchmarkName, JsonMetric};
use nom::{
    bytes::complete::tag,
    character::complete::{space0, space1},
    combinator::{eof, map},
    sequence::tuple,
    IResult,
};

use crate::{
    adapters::util::parse_f64,
    results::adapter_results::{AdapterMetricKind, AdapterResults},
    Adapter, Settings,
};

pub struct AdapterRustIai;

impl Adapter for AdapterRustIai {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        match settings.average {
            Some(JsonAverage::Mean) | None => {},
            Some(JsonAverage::Median) => return None,
        }

        let mut benchmark_metrics = Vec::new();
        let lines = input.lines().collect::<Vec<_>>();
        for lines in lines.windows(6) {
            if let Some((benchmark_name, benchmark_metric)) = parse_iai_lines(lines) {
                for metric in benchmark_metric {
                    benchmark_metrics.push((benchmark_name.clone(), metric));
                }
            }
        }

        AdapterResults::new(benchmark_metrics)
    }
}

fn parse_iai_lines(lines: &[&str]) -> Option<(BenchmarkName, Vec<AdapterMetricKind>)> {
    if lines.len() != 6 {
        return None;
    }
    let instructions_fn: fn(_) -> _ = |r| AdapterMetricKind::Instructions(r);
    let benchmark_name_line = lines[0];
    let instructions_line = lines[1];
    let l1_accesses_line = lines[2];
    let l2_accesses_line = lines[3];
    let ram_accesses_line = lines[4];
    let cycles_line = lines[5];
    let metrics = [
        ("Instructions:", instructions_line, instructions_fn),
        ("L1 Accesses:", l1_accesses_line, |r| {
            AdapterMetricKind::L1Accesses(r)
        }),
        ("L2 Accesses:", l2_accesses_line, |r| {
            AdapterMetricKind::L2Accesses(r)
        }),
        ("RAM Accesses:", ram_accesses_line, |r| {
            AdapterMetricKind::RamAccesses(r)
        }),
        ("Estimated Cycles:", cycles_line, |r| {
            AdapterMetricKind::Cycles(r)
        }),
    ]
    .into_iter()
    .map(
        |(header, input, to_variant): (_, _, fn(JsonMetric) -> AdapterMetricKind)| {
            parse_iai_metric(input, header).map(|metric| to_variant(metric.1))
        },
    )
    .collect::<Result<Vec<_>, _>>()
    .ok()?;
    let name = benchmark_name_line.parse().ok()?;
    Some((name, metrics))
}

fn parse_iai_metric<'a>(input: &'a str, metric: &'static str) -> IResult<&'a str, JsonMetric> {
    map(parse_from_header(metric), |instructions| JsonMetric {
        value: instructions.into(),
        lower_bound: None,
        upper_bound: None,
    })(input)
}

fn parse_from_header(header: &'static str) -> Box<dyn Fn(&str) -> IResult<&str, f64>> {
    Box::new(move |input| {
        map(
            tuple((space0, tag(header), space1, parse_f64, eof)),
            |(_, _, _, value, _)| value,
        )(input)
    })
}

#[cfg(test)]
pub(crate) mod test_rust_iai {

    use crate::Adapter;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_parse_line() {
        assert_eq!(
            super::parse_from_header("Instructions:")("  Instructions:  1234"),
            Ok(("", 1234.0))
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
