use bencher_json::{
    BenchmarkName, JsonNewMetric,
    project::{
        measure::built_in::{BuiltInMeasure as _, iai_callgrind},
        report::JsonAverage,
    },
};
use nom::{
    IResult,
    branch::alt,
    bytes::complete::{is_a, is_not, tag},
    character::complete::{space0, space1},
    combinator::{map, opt, recognize},
    multi::{many0, many1},
    sequence::{delimited, preceded, terminated, tuple},
};

use crate::{
    Adaptable, Settings,
    adapters::util::{parse_f64, parse_u64},
    results::adapter_results::{AdapterResults, IaiCallgrindMeasure},
};

pub struct AdapterRustIaiCallgrind;

impl Adaptable for AdapterRustIaiCallgrind {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        match settings.average {
            None => {},
            Some(JsonAverage::Mean | JsonAverage::Median) => {
                return None; // 'iai_callgrind' results are for a single run only.
            },
        }

        // Clean up the input by removing ANSI escape codes:
        let input = strip_ansi_escapes::strip_str(input);

        let benchmarks = match multiple_benchmarks()(&input) {
            Err(error) => {
                debug_assert!(false, "Error parsing input:\n{error:#?}");
                return None;
            },
            Ok((remainder, benchmarks)) => {
                debug_assert_eq!(remainder.len(), 0, "Unparsed trailing input:\n{remainder}");
                benchmarks
            },
        };

        AdapterResults::new_iai_callgrind(benchmarks)
    }
}

fn multiple_benchmarks<'a>()
-> impl FnMut(&'a str) -> IResult<&'a str, Vec<(BenchmarkName, Vec<IaiCallgrindMeasure>)>> {
    map(
        many0(alt((
            // Try to parse a single benchmark:
            single_benchmark(),
            // Otherwise, parse/ignore unrelated lines:
            map(terminated(not_line_ending(), opt(line_ending())), |_| None),
            // Otherwise, parse/ignore empty lines:
            map(line_ending(), |_| None),
        ))),
        // Skip 'None' resulting from empty/unrelated lines:
        |benchmarks| benchmarks.into_iter().flatten().collect(),
    )
}

fn single_benchmark<'a>()
-> impl FnMut(&'a str) -> IResult<&'a str, Option<(BenchmarkName, Vec<IaiCallgrindMeasure>)>> {
    map(
        tuple((
            terminated(recognize(not_line_ending()), line_ending()),
            // Callgrind tool if it was enabled:
            opt(callgrind_tool_measures()),
            // Add DHAT tool measures if it was enabled:
            opt(dhat_tool_measures()),
        )),
        |(benchmark_name, callgrind_measures, dhat_measures)| {
            let benchmark_name = benchmark_name.parse().ok()?;

            let mut measures = vec![];
            measures.extend(callgrind_measures.into_iter().flatten());
            measures.extend(dhat_measures.into_iter().flatten());

            Some((benchmark_name, measures))
        },
    )
}

fn callgrind_tool_measures<'a>() -> impl FnMut(&'a str) -> IResult<&'a str, Vec<IaiCallgrindMeasure>>
{
    map(
        preceded(
            opt(tool_name_line("CALLGRIND")),
            tuple((
                opt(metric_line(iai_callgrind::Instructions::NAME_STR)),
                opt(metric_line(iai_callgrind::L1Hits::NAME_STR)),
                opt(metric_line(iai_callgrind::L2Hits::NAME_STR)),
                opt(metric_line(iai_callgrind::RamHits::NAME_STR)),
                opt(metric_line(iai_callgrind::TotalReadWrite::NAME_STR)),
                opt(metric_line(iai_callgrind::EstimatedCycles::NAME_STR)),
                opt(metric_line(iai_callgrind::GlobalBusEvents::NAME_STR)),
            )),
        ),
        |(
            instructions,
            l1_hits,
            l2_hits,
            ram_hits,
            total_read_write,
            estimated_cycles,
            global_bus_events,
        )| {
            [
                instructions.map(IaiCallgrindMeasure::Instructions),
                l1_hits.map(IaiCallgrindMeasure::L1Hits),
                l2_hits.map(IaiCallgrindMeasure::L2Hits),
                ram_hits.map(IaiCallgrindMeasure::RamHits),
                total_read_write.map(IaiCallgrindMeasure::TotalReadWrite),
                estimated_cycles.map(IaiCallgrindMeasure::EstimatedCycles),
                global_bus_events.map(IaiCallgrindMeasure::GlobalBusEvents),
            ]
            .into_iter()
            .flatten()
            .collect()
        },
    )
}

fn dhat_tool_measures<'a>() -> impl FnMut(&'a str) -> IResult<&'a str, Vec<IaiCallgrindMeasure>> {
    map(
        preceded(
            opt(tool_name_line("DHAT")),
            tuple((
                metric_line(iai_callgrind::TotalBytes::NAME_STR),
                metric_line(iai_callgrind::TotalBlocks::NAME_STR),
                metric_line(iai_callgrind::AtTGmaxBytes::NAME_STR),
                metric_line(iai_callgrind::AtTGmaxBlocks::NAME_STR),
                metric_line(iai_callgrind::AtTEndBytes::NAME_STR),
                metric_line(iai_callgrind::AtTEndBlocks::NAME_STR),
                metric_line(iai_callgrind::ReadsBytes::NAME_STR),
                metric_line(iai_callgrind::WritesBytes::NAME_STR),
            )),
        ),
        |(
            total_bytes,
            total_blocks,
            at_t_gmax_bytes,
            at_t_gmax_blocks,
            at_t_end_bytes,
            at_t_end_blocks,
            reads_bytes,
            writes_bytes,
        )| {
            vec![
                IaiCallgrindMeasure::TotalBytes(total_bytes),
                IaiCallgrindMeasure::TotalBlocks(total_blocks),
                IaiCallgrindMeasure::AtTGmaxBytes(at_t_gmax_bytes),
                IaiCallgrindMeasure::AtTGmaxBlocks(at_t_gmax_blocks),
                IaiCallgrindMeasure::AtTEndBytes(at_t_end_bytes),
                IaiCallgrindMeasure::AtTEndBlocks(at_t_end_blocks),
                IaiCallgrindMeasure::ReadsBytes(reads_bytes),
                IaiCallgrindMeasure::WritesBytes(writes_bytes),
            ]
        },
    )
}

fn tool_name_line<'a>(tool_name: &'static str) -> impl FnMut(&'a str) -> IResult<&'a str, &'a str> {
    delimited(
        tuple((space0, many1(tag("=")), tag(" "))),
        tag(tool_name),
        tuple((tag(" "), many1(tag("=")), line_ending())),
    )
}

fn metric_line<'a>(
    measure_name: &'static str,
) -> impl FnMut(&'a str) -> IResult<&'a str, JsonNewMetric> {
    map(
        tuple((
            space0,
            tag(measure_name),
            tag(":"),
            space1,
            // the current run value:
            parse_u64,
            tag("|"),
            alt((
                // No previous run:
                recognize(tuple((tag("N/A"), space1, tag("(*********)")))),
                // Comparison to previous run:
                recognize(tuple((
                    parse_u64,
                    space0,
                    alt((
                        recognize(tag("(No change)")),
                        recognize(tuple((
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
                        ))),
                    )),
                ))),
            )),
            line_ending(),
        )),
        |(_, _, _, _, current_value, _, _, _)| JsonNewMetric {
            #[expect(clippy::cast_precision_loss)]
            value: (current_value as f64).into(),
            lower_value: None,
            upper_value: None,
        },
    )
}

fn line_ending<'a>() -> impl FnMut(&'a str) -> IResult<&'a str, &'a str> {
    is_a("\r\n")
}

fn not_line_ending<'a>() -> impl FnMut(&'a str) -> IResult<&'a str, &'a str> {
    // Note: `not(line_ending)` doesn't work here, as it won't consume the matched characters
    is_not("\r\n")
}

#[cfg(test)]
pub(crate) mod test_rust_iai_callgrind {
    use crate::{AdapterResults, adapters::test_util::convert_file_path};
    use bencher_json::project::measure::built_in::{BuiltInMeasure as _, iai_callgrind};
    use ordered_float::OrderedFloat;
    use pretty_assertions::assert_eq;

    use super::AdapterRustIaiCallgrind;
    use std::collections::HashMap;

    #[test]
    fn test_without_optional_metrics() {
        let results = convert_file_path::<AdapterRustIaiCallgrind>(
            "./tool_output/rust/iai_callgrind/without-optional-metrics.txt",
        );

        validate_adapter_rust_iai_callgrind(&results, &OptionalMetrics::default());
    }

    #[test]
    fn test_with_dhat() {
        let results = convert_file_path::<AdapterRustIaiCallgrind>(
            "./tool_output/rust/iai_callgrind/with-dhat.txt",
        );

        validate_adapter_rust_iai_callgrind(
            &results,
            &OptionalMetrics {
                dhat: true,
                ..Default::default()
            },
        );
    }

    #[test]
    fn test_with_dhat_and_global_bus_events() {
        let results = convert_file_path::<AdapterRustIaiCallgrind>(
            "./tool_output/rust/iai_callgrind/with-dhat-and-global-bus-events.txt",
        );

        validate_adapter_rust_iai_callgrind(
            &results,
            &OptionalMetrics {
                dhat: true,
                global_bus_events: true,
            },
        );
    }

    #[test]
    fn test_delta() {
        let results = convert_file_path::<AdapterRustIaiCallgrind>(
            "./tool_output/rust/iai_callgrind/delta.txt",
        );

        validate_adapter_rust_iai_callgrind(&results, &OptionalMetrics::default());
    }

    #[test]
    fn test_ansi_escapes_issue_345() {
        let results = convert_file_path::<AdapterRustIaiCallgrind>(
            "./tool_output/rust/iai_callgrind/ansi-escapes.txt",
        );

        validate_adapter_rust_iai_callgrind(&results, &OptionalMetrics::default());
    }

    #[test]
    fn test_with_ge() {
        let results = convert_file_path::<AdapterRustIaiCallgrind>(
            "./tool_output/rust/iai_callgrind/with-ge.txt",
        );

        validate_adapter_rust_iai_callgrind(
            &results,
            &OptionalMetrics {
                global_bus_events: true,
                ..Default::default()
            },
        );
    }

    #[derive(Default)]
    pub struct OptionalMetrics {
        pub global_bus_events: bool,
        pub dhat: bool,
    }

    pub fn validate_adapter_rust_iai_callgrind(
        results: &AdapterResults,
        optional_metrics: &OptionalMetrics,
    ) {
        assert_eq!(results.inner.len(), 2);

        {
            let mut expected = HashMap::new();

            expected.extend([
                (iai_callgrind::Instructions::SLUG_STR, 1_734.0),
                (iai_callgrind::L1Hits::SLUG_STR, 2_359.0),
                (iai_callgrind::L2Hits::SLUG_STR, 0.0),
                (iai_callgrind::RamHits::SLUG_STR, 3.0),
                (iai_callgrind::TotalReadWrite::SLUG_STR, 2_362.0),
                (iai_callgrind::EstimatedCycles::SLUG_STR, 2_464.0),
            ]);

            if optional_metrics.global_bus_events {
                expected.insert(iai_callgrind::GlobalBusEvents::SLUG_STR, 2.0);
            }

            if optional_metrics.dhat {
                expected.extend([
                    (iai_callgrind::TotalBytes::SLUG_STR, 29_499.0),
                    (iai_callgrind::TotalBlocks::SLUG_STR, 2_806.0),
                    (iai_callgrind::AtTGmaxBytes::SLUG_STR, 378.0),
                    (iai_callgrind::AtTGmaxBlocks::SLUG_STR, 34.0),
                    (iai_callgrind::AtTEndBytes::SLUG_STR, 0.0),
                    (iai_callgrind::AtTEndBlocks::SLUG_STR, 0.0),
                    (iai_callgrind::ReadsBytes::SLUG_STR, 57_725.0),
                    (iai_callgrind::WritesBytes::SLUG_STR, 73_810.0),
                ]);
            }

            compare_benchmark(
                &expected,
                results,
                "rust_iai_callgrind::bench_fibonacci_group::bench_fibonacci short:10",
            );
        }

        {
            let mut expected = HashMap::new();

            expected.extend([
                (iai_callgrind::Instructions::SLUG_STR, 26_214_734.0),
                (iai_callgrind::L1Hits::SLUG_STR, 35_638_619.0),
                (iai_callgrind::L2Hits::SLUG_STR, 0.0),
                (iai_callgrind::RamHits::SLUG_STR, 3.0),
                (iai_callgrind::TotalReadWrite::SLUG_STR, 35_638_622.0),
                (iai_callgrind::EstimatedCycles::SLUG_STR, 35_638_724.0),
            ]);

            if optional_metrics.global_bus_events {
                expected.insert(iai_callgrind::GlobalBusEvents::SLUG_STR, 10.0);
            }

            if optional_metrics.dhat {
                expected.extend([
                    (iai_callgrind::TotalBytes::SLUG_STR, 26_294_939.0),
                    (iai_callgrind::TotalBlocks::SLUG_STR, 2_328_086.0),
                    (iai_callgrind::AtTGmaxBytes::SLUG_STR, 933_718.0),
                    (iai_callgrind::AtTGmaxBlocks::SLUG_STR, 18_344.0),
                    (iai_callgrind::AtTEndBytes::SLUG_STR, 0.0),
                    (iai_callgrind::AtTEndBlocks::SLUG_STR, 0.0),
                    (iai_callgrind::ReadsBytes::SLUG_STR, 47_577_425.0),
                    (iai_callgrind::WritesBytes::SLUG_STR, 37_733_810.0),
                ]);
            }

            compare_benchmark(
                &expected,
                results,
                "rust_iai_callgrind::bench_fibonacci_group::bench_fibonacci long:30",
            );
        }
    }

    fn compare_benchmark(
        expected: &HashMap<&str, f64>,
        results: &AdapterResults,
        benchmark_name: &str,
    ) {
        let actual = results.get(benchmark_name).unwrap();
        assert_eq!(actual.inner.len(), expected.len());

        for (key, value) in expected {
            let metric = actual.get(key).unwrap();
            assert_eq!(metric.value, OrderedFloat::from(*value));
            assert_eq!(metric.lower_value, None);
            assert_eq!(metric.upper_value, None);
        }
    }
}
