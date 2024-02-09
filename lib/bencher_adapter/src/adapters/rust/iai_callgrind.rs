use bencher_json::{
    project::{
        measure::{ESTIMATED_CYCLES_NAME_STR, INSTRUCTIONS_NAME_STR},
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
    results::adapter_results::{AdapterResults, IaiCallgrindMeasure},
    Adaptable, Settings,
};

pub struct AdapterRustIaiCallgrind;

const IAI_CALLGRIND_METRICS_LINE_COUNT: usize = 7;
const L1_HITS_NAME_STR: &str = "L1 Hits";
const L2_HITS_NAME_STR: &str = "L2 Hits";
const RAM_HITS_NAME_STR: &str = "RAM Hits";
const TOTAL_READ_WRITE_NAME_STR: &str = "Total read+write";

impl Adaptable for AdapterRustIaiCallgrind {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        match settings.average {
            None => {},
            Some(JsonAverage::Mean | JsonAverage::Median) => return None,
        }

        let mut benchmark_metrics = Vec::new();
        let lines = input.lines().collect::<Vec<_>>();
        for lines in lines.windows(IAI_CALLGRIND_METRICS_LINE_COUNT) {
            let Ok(lines) = lines.try_into() else {
                debug_assert!(
                    false,
                    "Windows struct should always be convertible to array of the same size."
                );
                continue;
            };
            if let Some((benchmark_name, metrics)) = parse_iai_lines(lines) {
                benchmark_metrics.push((benchmark_name, metrics));
            }
        }

        AdapterResults::new_iai_callgrind(benchmark_metrics)
    }
}

fn parse_iai_lines(
    lines: [&str; IAI_CALLGRIND_METRICS_LINE_COUNT],
) -> Option<(BenchmarkName, Vec<IaiCallgrindMeasure>)> {
    let [benchmark_name_line, instructions_line, l1_accesses_line, l2_accesses_line, ram_accesses_line, total_read_write_line, estimated_cycles_line] =
        lines;

    let name = benchmark_name_line.parse().ok()?;
    #[allow(trivial_casts)]
    let metrics = [
        (
            INSTRUCTIONS_NAME_STR,
            instructions_line,
            IaiCallgrindMeasure::Instructions as fn(JsonMetric) -> IaiCallgrindMeasure,
        ),
        (
            L1_HITS_NAME_STR,
            l1_accesses_line,
            IaiCallgrindMeasure::L1Accesses,
        ),
        (
            L2_HITS_NAME_STR,
            l2_accesses_line,
            IaiCallgrindMeasure::L2Accesses,
        ),
        (
            RAM_HITS_NAME_STR,
            ram_accesses_line,
            IaiCallgrindMeasure::RamAccesses,
        ),
        (
            TOTAL_READ_WRITE_NAME_STR,
            total_read_write_line,
            IaiCallgrindMeasure::TotalReadWrite,
        ),
        (
            ESTIMATED_CYCLES_NAME_STR,
            estimated_cycles_line,
            IaiCallgrindMeasure::EstimatedCycles,
        ),
    ]
    .into_iter()
    .map(|(measure, input, into_variant)| {
        parse_iai_callgrind_metric(input, measure)
            .map(|(_remainder, json_metric)| into_variant(json_metric))
    })
    .collect::<Result<Vec<_>, _>>()
    .ok()?;

    Some((name, metrics))
}

#[allow(clippy::cast_precision_loss)]
fn parse_iai_callgrind_metric<'a>(
    input: &'a str,
    measure: &'static str,
) -> IResult<&'a str, JsonMetric> {
    map(
        tuple((
            space0,
            tag(measure),
            tag(":"),
            space1,
            parse_u64,
            tag("|"),
            alt((
                // No previous run
                map(tuple((tag("N/A"), space1, tag("(*********)"))), |_| ()),
                // Comparison to previous run
                map(
                    tuple((
                        parse_u64,
                        space0,
                        alt((
                            map(tag("(No change)"), |_| ()),
                            map(
                                tuple((
                                    delimited(
                                        tag("("),
                                        tuple((alt((tag("+"), tag("-"))), parse_f64, tag("%"))),
                                        tag(")"),
                                    ),
                                    space1,
                                    delimited(
                                        tag("["),
                                        tuple((alt((tag("+"), tag("-"))), parse_f64, tag("x"))),
                                        tag("]"),
                                    ),
                                )),
                                |_| (),
                            ),
                        )),
                    )),
                    |_| (),
                ),
            )),
            eof,
        )),
        |(_, _, _, _, metric, _, (), _)| JsonMetric {
            value: (metric as f64).into(),
            lower_value: None,
            upper_value: None,
        },
    )(input)
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
pub(crate) mod test_rust_iai_callgrind {

    use crate::{
        adapters::test_util::convert_file_path, results::adapter_metrics::AdapterMetrics,
        Adaptable, AdapterResults,
    };
    use bencher_json::{
        project::measure::{
            ESTIMATED_CYCLES_SLUG_STR, INSTRUCTIONS_NAME_STR, INSTRUCTIONS_SLUG_STR,
            L1_ACCESSES_SLUG_STR, L2_ACCESSES_SLUG_STR, RAM_ACCESSES_SLUG_STR,
            TOTAL_ACCESSES_SLUG_STR,
        },
        JsonMetric,
    };
    use ordered_float::OrderedFloat;
    use pretty_assertions::assert_eq;

    use super::AdapterRustIaiCallgrind;

    fn convert_rust_iai_callgrind(suffix: &str) -> AdapterResults {
        let file_path = format!("./tool_output/rust/iai_callgrind/{suffix}.txt");
        convert_file_path::<AdapterRustIaiCallgrind>(&file_path)
    }

    pub fn validate_iai_callgrind(metrics: &AdapterMetrics, results: [(&str, f64); 6]) {
        assert_eq!(metrics.inner.len(), 6);
        for (key, value) in results {
            let metric = metrics.get(key).unwrap();
            assert_eq!(metric.value, OrderedFloat::from(value));
            assert_eq!(metric.lower_value, None);
            assert_eq!(metric.upper_value, None);
        }
    }

    #[test]
    fn test_adapter_rust_iai_callgrind_parse_line() {
        assert_eq!(
            super::parse_iai_callgrind_metric(
                "  Instructions:                1234|N/A             (*********)",
                INSTRUCTIONS_NAME_STR
            ),
            Ok((
                "",
                JsonMetric {
                    value: 1234.0.into(),
                    upper_value: None,
                    lower_value: None
                }
            ))
        );

        assert_eq!(
            super::parse_iai_callgrind_metric(
                "  Instructions:                1234|1234            (No change)",
                INSTRUCTIONS_NAME_STR
            ),
            Ok((
                "",
                JsonMetric {
                    value: 1234.0.into(),
                    upper_value: None,
                    lower_value: None
                }
            ))
        );

        assert_eq!(
            super::parse_iai_callgrind_metric(
                "  Instructions:                1234|1000            (+23.4000%) [+1.23400x]",
                INSTRUCTIONS_NAME_STR
            ),
            Ok((
                "",
                JsonMetric {
                    value: 1234.0.into(),
                    upper_value: None,
                    lower_value: None
                }
            ))
        );
    }

    #[test]
    fn test_adapter_rust_iai_callgrind_parse_multiple_lines() {
        let input = "rust_iai_callgrind::bench_fibonacci_group::bench_fibonacci short:10
  Instructions:                1734|N/A             (*********)
  L1 Hits:                     2359|N/A             (*********)
  L2 Hits:                        0|N/A             (*********)
  RAM Hits:                       3|N/A             (*********)
  Total read+write:            2362|N/A             (*********)
  Estimated Cycles:            2464|N/A             (*********)";
        let output = super::AdapterRustIaiCallgrind::parse(input, crate::Settings::default());
        assert!(output.is_some());
    }

    #[test]
    fn test_adapter_rust_iai_callgrind() {
        let results = convert_rust_iai_callgrind("two");
        validate_adapter_rust_iai_callgrind(&results);
    }

    pub fn validate_adapter_rust_iai_callgrind(results: &AdapterResults) {
        assert_eq!(results.inner.len(), 2);

        let metrics = results
            .get("rust_iai_callgrind::bench_fibonacci_group::bench_fibonacci short:10")
            .unwrap();
        validate_iai_callgrind(
            metrics,
            [
                (INSTRUCTIONS_SLUG_STR, 1734.0),
                (L1_ACCESSES_SLUG_STR, 2359.0),
                (L2_ACCESSES_SLUG_STR, 0.0),
                (RAM_ACCESSES_SLUG_STR, 3.0),
                (TOTAL_ACCESSES_SLUG_STR, 2362.0),
                (ESTIMATED_CYCLES_SLUG_STR, 2464.0),
            ],
        );
        let metrics = results
            .get("rust_iai_callgrind::bench_fibonacci_group::bench_fibonacci long:30")
            .unwrap();
        validate_iai_callgrind(
            metrics,
            [
                (INSTRUCTIONS_SLUG_STR, 26_214_734.0),
                (L1_ACCESSES_SLUG_STR, 35_638_619.0),
                (L2_ACCESSES_SLUG_STR, 0.0),
                (RAM_ACCESSES_SLUG_STR, 3.0),
                (TOTAL_ACCESSES_SLUG_STR, 35_638_622.0),
                (ESTIMATED_CYCLES_SLUG_STR, 35_638_724.0),
            ],
        );
    }

    #[test]
    fn test_adapter_rust_iai_callgrind_change() {
        let results = convert_rust_iai_callgrind("change");
        assert_eq!(results.inner.len(), 2);

        let metrics = results
            .get("rust_iai_callgrind::bench_fibonacci_group::bench_fibonacci short:10")
            .unwrap();
        validate_iai_callgrind(
            metrics,
            [
                (INSTRUCTIONS_SLUG_STR, 1650.0),
                (L1_ACCESSES_SLUG_STR, 2275.0),
                (L2_ACCESSES_SLUG_STR, 0.0),
                (RAM_ACCESSES_SLUG_STR, 3.0),
                (TOTAL_ACCESSES_SLUG_STR, 2278.0),
                (ESTIMATED_CYCLES_SLUG_STR, 2380.0),
            ],
        );
        let metrics = results
            .get("rust_iai_callgrind::bench_fibonacci_group::bench_fibonacci long:30")
            .unwrap();
        validate_iai_callgrind(
            metrics,
            [
                (INSTRUCTIONS_SLUG_STR, 24_943_490.0),
                (L1_ACCESSES_SLUG_STR, 34_367_375.0),
                (L2_ACCESSES_SLUG_STR, 0.0),
                (RAM_ACCESSES_SLUG_STR, 3.0),
                (TOTAL_ACCESSES_SLUG_STR, 34_367_378.0),
                (ESTIMATED_CYCLES_SLUG_STR, 34_367_480.0),
            ],
        );
    }
}
