use bencher_json::{project::report::JsonAverage, BenchmarkName, JsonMetric};
use nom::{
    bytes::complete::tag,
    character::complete::{anychar, space0, space1},
    combinator::{eof, map, map_res},
    multi::many_till,
    sequence::tuple,
    IResult,
};

use crate::{
    adapters::util::{nom_error, parse_benchmark_name, parse_f64, NomError},
    results::adapter_results::AdapterResults,
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

        let mut prior_line = None;
        for line in input.lines() {
            if let Ok((remainder, benchmark_metric)) = parse_iai(prior_line, line) {
                if remainder.is_empty() {
                    benchmark_metrics.push(benchmark_metric);
                }
            }
            prior_line = Some(line);
        }

        AdapterResults::new_latency(benchmark_metrics)
    }
}

fn parse_iai<'i>(
    prior_line: Option<&str>,
    input: &'i str,
) -> IResult<&'i str, (BenchmarkName, JsonMetric)> {
    map_res(
        many_till(anychar, parse_iai_instructions),
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

fn parse_iai_instructions(input: &str) -> IResult<&str, JsonMetric> {
    map(parse_from_header("Instructions:"), |instructions| {
        JsonMetric {
            value: instructions.into(),
            lower_bound: None,
            upper_bound: None,
        }
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

    use pretty_assertions::assert_eq;

    #[test]
    fn test_parse_line() {
        assert_eq!(
            super::parse_from_header("Instructions:")("  Instructions:  1234"),
            Ok(("", 1234.0))
        );
    }
}
