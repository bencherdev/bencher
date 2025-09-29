use std::ops::Neg as _;

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
    bytes::complete::{is_a, is_not, tag, take_until1},
    character::complete::{char, space0, space1},
    combinator::{map, opt, peek, recognize},
    multi::{many0, many1},
    sequence::{delimited, preceded, terminated, tuple},
};

use crate::{
    Adaptable, Settings,
    adapters::util::parse_f64,
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
            terminated(
                recognize(tuple((
                    is_not(":\r\n \t"),
                    tag("::"),
                    is_not(":\r\n \t"),
                    tag("::"),
                    not_benchmark_name_end(),
                ))),
                benchmark_name_end(),
            ),
            many0(alt((
                // Callgrind tool if it was enabled:
                callgrind_tool_measures(),
                // Cachegrind tool if it was enabled:
                cachegrind_tool_measures(),
                // Add DHAT tool measures if it was enabled:
                dhat_tool_measures(),
                // Add Memcheck tool measures if it was enabled:
                memcheck_tool_measures(),
                // Add Helgrind tool measures if it was enabled:
                helgrind_tool_measures(),
                // Add Drd tool measures if it was enabled:
                drd_tool_measures(),
            ))),
        )),
        |(benchmark_name, measures)| {
            // trim here to avoid loose `\r` chars at the end of the string because of the
            // `not_benchmark_name_end` parser. It's maybe not a bad idea anyways.
            let benchmark_name = benchmark_name.trim().parse().ok()?;
            let measures = measures.into_iter().flatten().collect();

            Some((benchmark_name, measures))
        },
    )
}

#[expect(clippy::too_many_lines)]
fn callgrind_tool_measures<'a>() -> impl FnMut(&'a str) -> IResult<&'a str, Vec<IaiCallgrindMeasure>>
{
    map(
        preceded(opt(tool_name_line("CALLGRIND")), many1(metric_line())),
        |metrics| {
            metrics
                .into_iter()
                .map(|(metric_name, json)| match metric_name.as_str() {
                    iai_callgrind::Instructions::NAME_STR => {
                        IaiCallgrindMeasure::Instructions(json)
                    },
                    iai_callgrind::L1Hits::NAME_STR => IaiCallgrindMeasure::L1Hits(json),
                    iai_callgrind::L2Hits::NAME_STR => IaiCallgrindMeasure::L2Hits(json),
                    iai_callgrind::LLHits::NAME_STR => IaiCallgrindMeasure::LLHits(json),
                    iai_callgrind::RamHits::NAME_STR => IaiCallgrindMeasure::RamHits(json),
                    iai_callgrind::TotalReadWrite::NAME_STR => {
                        IaiCallgrindMeasure::TotalReadWrite(json)
                    },
                    iai_callgrind::EstimatedCycles::NAME_STR => {
                        IaiCallgrindMeasure::EstimatedCycles(json)
                    },
                    iai_callgrind::Dr::NAME_STR => IaiCallgrindMeasure::DataCacheReads(json),
                    iai_callgrind::Dw::NAME_STR => IaiCallgrindMeasure::DataCacheWrites(json),
                    iai_callgrind::I1mr::NAME_STR => {
                        IaiCallgrindMeasure::L1InstrCacheReadMisses(json)
                    },
                    iai_callgrind::D1mr::NAME_STR => {
                        IaiCallgrindMeasure::L1DataCacheReadMisses(json)
                    },
                    iai_callgrind::D1mw::NAME_STR => {
                        IaiCallgrindMeasure::L1DataCacheWriteMisses(json)
                    },
                    iai_callgrind::ILmr::NAME_STR => {
                        IaiCallgrindMeasure::LLInstrCacheReadMisses(json)
                    },
                    iai_callgrind::DLmr::NAME_STR => {
                        IaiCallgrindMeasure::LLDataCacheReadMisses(json)
                    },
                    iai_callgrind::DLmw::NAME_STR => {
                        IaiCallgrindMeasure::LLDataCacheWriteMisses(json)
                    },
                    iai_callgrind::I1MissRate::NAME_STR => {
                        IaiCallgrindMeasure::L1InstrCacheMissRate(json)
                    },
                    iai_callgrind::LLiMissRate::NAME_STR => {
                        IaiCallgrindMeasure::LLInstrCacheMissRate(json)
                    },
                    iai_callgrind::D1MissRate::NAME_STR => {
                        IaiCallgrindMeasure::L1DataCacheMissRate(json)
                    },
                    iai_callgrind::LLdMissRate::NAME_STR => {
                        IaiCallgrindMeasure::LLDataCacheMissRate(json)
                    },
                    iai_callgrind::LLMissRate::NAME_STR => {
                        IaiCallgrindMeasure::LLCacheMissRate(json)
                    },
                    iai_callgrind::L1HitRate::NAME_STR => IaiCallgrindMeasure::L1HitRate(json),
                    iai_callgrind::LLHitRate::NAME_STR => IaiCallgrindMeasure::LLHitRate(json),
                    iai_callgrind::RamHitRate::NAME_STR => IaiCallgrindMeasure::RamHitRate(json),
                    iai_callgrind::SysCount::NAME_STR => {
                        IaiCallgrindMeasure::NumberSystemCalls(json)
                    },
                    iai_callgrind::SysTime::NAME_STR => IaiCallgrindMeasure::TimeSystemCalls(json),
                    iai_callgrind::SysCpuTime::NAME_STR => {
                        IaiCallgrindMeasure::CpuTimeSystemCalls(json)
                    },
                    iai_callgrind::GlobalBusEvents::NAME_STR => {
                        IaiCallgrindMeasure::GlobalBusEvents(json)
                    },
                    iai_callgrind::Bc::NAME_STR => {
                        IaiCallgrindMeasure::ExecutedConditionalBranches(json)
                    },
                    iai_callgrind::Bcm::NAME_STR => {
                        IaiCallgrindMeasure::MispredictedConditionalBranches(json)
                    },
                    iai_callgrind::Bi::NAME_STR => {
                        IaiCallgrindMeasure::ExecutedIndirectBranches(json)
                    },
                    iai_callgrind::Bim::NAME_STR => {
                        IaiCallgrindMeasure::MispredictedIndirectBranches(json)
                    },
                    iai_callgrind::ILdmr::NAME_STR => {
                        IaiCallgrindMeasure::DirtyMissInstructionRead(json)
                    },
                    iai_callgrind::DLdmr::NAME_STR => IaiCallgrindMeasure::DirtyMissDataRead(json),
                    iai_callgrind::DLdmw::NAME_STR => IaiCallgrindMeasure::DirtyMissDataWrite(json),
                    iai_callgrind::AcCost1::NAME_STR => {
                        IaiCallgrindMeasure::L1BadTemporalLocality(json)
                    },
                    iai_callgrind::AcCost2::NAME_STR => {
                        IaiCallgrindMeasure::LLBadTemporalLocality(json)
                    },
                    iai_callgrind::SpLoss1::NAME_STR => {
                        IaiCallgrindMeasure::L1BadSpatialLocality(json)
                    },
                    iai_callgrind::SpLoss2::NAME_STR => {
                        IaiCallgrindMeasure::LLBadSpatialLocality(json)
                    },
                    _ => IaiCallgrindMeasure::Unknown,
                })
                .collect()
        },
    )
}

fn cachegrind_tool_measures<'a>()
-> impl FnMut(&'a str) -> IResult<&'a str, Vec<IaiCallgrindMeasure>> {
    map(
        preceded(tool_name_line("CACHEGRIND"), many1(metric_line())),
        |metrics| {
            metrics
                .into_iter()
                .map(|(metric_name, json)| match metric_name.as_str() {
                    iai_callgrind::Instructions::NAME_STR => {
                        IaiCallgrindMeasure::Instructions(json)
                    },
                    iai_callgrind::L1Hits::NAME_STR => IaiCallgrindMeasure::L1Hits(json),
                    iai_callgrind::L2Hits::NAME_STR => IaiCallgrindMeasure::L2Hits(json),
                    iai_callgrind::LLHits::NAME_STR => IaiCallgrindMeasure::LLHits(json),
                    iai_callgrind::RamHits::NAME_STR => IaiCallgrindMeasure::RamHits(json),
                    iai_callgrind::TotalReadWrite::NAME_STR => {
                        IaiCallgrindMeasure::TotalReadWrite(json)
                    },
                    iai_callgrind::EstimatedCycles::NAME_STR => {
                        IaiCallgrindMeasure::EstimatedCycles(json)
                    },
                    iai_callgrind::Dr::NAME_STR => IaiCallgrindMeasure::DataCacheReads(json),
                    iai_callgrind::Dw::NAME_STR => IaiCallgrindMeasure::DataCacheWrites(json),
                    iai_callgrind::I1mr::NAME_STR => {
                        IaiCallgrindMeasure::L1InstrCacheReadMisses(json)
                    },
                    iai_callgrind::D1mr::NAME_STR => {
                        IaiCallgrindMeasure::L1DataCacheReadMisses(json)
                    },
                    iai_callgrind::D1mw::NAME_STR => {
                        IaiCallgrindMeasure::L1DataCacheWriteMisses(json)
                    },
                    iai_callgrind::ILmr::NAME_STR => {
                        IaiCallgrindMeasure::LLInstrCacheReadMisses(json)
                    },
                    iai_callgrind::DLmr::NAME_STR => {
                        IaiCallgrindMeasure::LLDataCacheReadMisses(json)
                    },
                    iai_callgrind::DLmw::NAME_STR => {
                        IaiCallgrindMeasure::LLDataCacheWriteMisses(json)
                    },
                    iai_callgrind::I1MissRate::NAME_STR => {
                        IaiCallgrindMeasure::L1InstrCacheMissRate(json)
                    },
                    iai_callgrind::LLiMissRate::NAME_STR => {
                        IaiCallgrindMeasure::LLInstrCacheMissRate(json)
                    },
                    iai_callgrind::D1MissRate::NAME_STR => {
                        IaiCallgrindMeasure::L1DataCacheMissRate(json)
                    },
                    iai_callgrind::LLdMissRate::NAME_STR => {
                        IaiCallgrindMeasure::LLDataCacheMissRate(json)
                    },
                    iai_callgrind::LLMissRate::NAME_STR => {
                        IaiCallgrindMeasure::LLCacheMissRate(json)
                    },
                    iai_callgrind::L1HitRate::NAME_STR => IaiCallgrindMeasure::L1HitRate(json),
                    iai_callgrind::LLHitRate::NAME_STR => IaiCallgrindMeasure::LLHitRate(json),
                    iai_callgrind::RamHitRate::NAME_STR => IaiCallgrindMeasure::RamHitRate(json),
                    _ => IaiCallgrindMeasure::Unknown,
                })
                .collect()
        },
    )
}

fn dhat_tool_measures<'a>() -> impl FnMut(&'a str) -> IResult<&'a str, Vec<IaiCallgrindMeasure>> {
    map(
        preceded(tool_name_line("DHAT"), many1(metric_line())),
        |metrics| {
            metrics
                .into_iter()
                .map(|(metric_name, json)| match metric_name.as_str() {
                    iai_callgrind::TotalBytes::NAME_STR => IaiCallgrindMeasure::TotalBytes(json),
                    iai_callgrind::TotalBlocks::NAME_STR => IaiCallgrindMeasure::TotalBlocks(json),
                    iai_callgrind::AtTGmaxBytes::NAME_STR => {
                        IaiCallgrindMeasure::AtTGmaxBytes(json)
                    },
                    iai_callgrind::AtTGmaxBlocks::NAME_STR => {
                        IaiCallgrindMeasure::AtTGmaxBlocks(json)
                    },
                    iai_callgrind::AtTEndBytes::NAME_STR => IaiCallgrindMeasure::AtTEndBytes(json),
                    iai_callgrind::AtTEndBlocks::NAME_STR => {
                        IaiCallgrindMeasure::AtTEndBlocks(json)
                    },
                    iai_callgrind::ReadsBytes::NAME_STR => IaiCallgrindMeasure::ReadsBytes(json),
                    iai_callgrind::WritesBytes::NAME_STR => IaiCallgrindMeasure::WritesBytes(json),
                    _ => IaiCallgrindMeasure::Unknown,
                })
                .collect()
        },
    )
}

fn memcheck_tool_measures<'a>() -> impl FnMut(&'a str) -> IResult<&'a str, Vec<IaiCallgrindMeasure>>
{
    map(
        preceded(tool_name_line("MEMCHECK"), many1(metric_line())),
        |metrics| {
            metrics
                .into_iter()
                .map(|(metric_name, json)| match metric_name.as_str() {
                    iai_callgrind::MemcheckErrors::NAME_STR => {
                        IaiCallgrindMeasure::MemcheckErrors(json)
                    },
                    iai_callgrind::MemcheckContexts::NAME_STR => {
                        IaiCallgrindMeasure::MemcheckContexts(json)
                    },
                    iai_callgrind::MemcheckSuppressedErrors::NAME_STR => {
                        IaiCallgrindMeasure::MemcheckSuppressedErrors(json)
                    },
                    iai_callgrind::MemcheckSuppressedContexts::NAME_STR => {
                        IaiCallgrindMeasure::MemcheckSuppressedContexts(json)
                    },
                    _ => IaiCallgrindMeasure::Unknown,
                })
                .collect()
        },
    )
}

fn helgrind_tool_measures<'a>() -> impl FnMut(&'a str) -> IResult<&'a str, Vec<IaiCallgrindMeasure>>
{
    map(
        preceded(tool_name_line("HELGRIND"), many1(metric_line())),
        |metrics| {
            metrics
                .into_iter()
                .map(|(metric_name, json)| match metric_name.as_str() {
                    iai_callgrind::HelgrindErrors::NAME_STR => {
                        IaiCallgrindMeasure::HelgrindErrors(json)
                    },
                    iai_callgrind::HelgrindContexts::NAME_STR => {
                        IaiCallgrindMeasure::HelgrindContexts(json)
                    },
                    iai_callgrind::HelgrindSuppressedErrors::NAME_STR => {
                        IaiCallgrindMeasure::HelgrindSuppressedErrors(json)
                    },
                    iai_callgrind::HelgrindSuppressedContexts::NAME_STR => {
                        IaiCallgrindMeasure::HelgrindSuppressedContexts(json)
                    },
                    _ => IaiCallgrindMeasure::Unknown,
                })
                .collect()
        },
    )
}

fn drd_tool_measures<'a>() -> impl FnMut(&'a str) -> IResult<&'a str, Vec<IaiCallgrindMeasure>> {
    map(
        preceded(tool_name_line("DRD"), many1(metric_line())),
        |metrics| {
            metrics
                .into_iter()
                .map(|(metric_name, json)| match metric_name.as_str() {
                    iai_callgrind::DrdErrors::NAME_STR => IaiCallgrindMeasure::DrdErrors(json),
                    iai_callgrind::DrdContexts::NAME_STR => IaiCallgrindMeasure::DrdContexts(json),
                    iai_callgrind::DrdSuppressedErrors::NAME_STR => {
                        IaiCallgrindMeasure::DrdSuppressedErrors(json)
                    },
                    iai_callgrind::DrdSuppressedContexts::NAME_STR => {
                        IaiCallgrindMeasure::DrdSuppressedContexts(json)
                    },
                    _ => IaiCallgrindMeasure::Unknown,
                })
                .collect()
        },
    )
}

fn tool_name_line<'a>(tool_name: &'static str) -> impl FnMut(&'a str) -> IResult<&'a str, &'a str> {
    delimited(
        tuple((space1, many1(tag("=")), tag(" "))),
        tag(tool_name),
        tuple((tag(" "), many1(tag("=")), line_ending())),
    )
}

fn metric_line<'a>() -> impl FnMut(&'a str) -> IResult<&'a str, (String, JsonNewMetric)> {
    map(
        tuple((
            space1,
            is_not(":\r\n"),
            char(':'),
            space1,
            // the current run value:
            parse_f64,
            char('|'),
            alt((
                // No previous run:
                recognize(tuple((
                    tag("N/A"),
                    space1,
                    delimited(char('('), many1(char('*')), char(')')),
                ))),
                // Comparison to previous run:
                recognize(tuple((
                    parse_f64,
                    space0,
                    alt((
                        recognize(delimited(char('('), tag("No change"), char(')'))),
                        recognize(tuple((
                            delimited(char('('), alt((infinity, percent)), char(')')),
                            space1,
                            delimited(char('['), alt((infinity, factor)), char(']')),
                        ))),
                    )),
                ))),
            )),
            line_ending(),
        )),
        |(_, metric_name, _, _, current_value, _, _, _)| {
            (
                metric_name.to_owned(),
                JsonNewMetric {
                    value: current_value.into(),
                    lower_value: None,
                    upper_value: None,
                },
            )
        },
    )
}

fn infinity(input: &str) -> IResult<&str, f64> {
    map(
        tuple((
            alt((many1(char('+')), many1(char('-')))),
            tag("inf"),
            alt((many1(char('+')), many1(char('-')))),
        )),
        |(signs, _, _)| {
            // The indexing is safe due to the usage of `many1` above
            #[expect(clippy::indexing_slicing)]
            let sign = signs[0];
            if sign == '+' {
                f64::INFINITY
            } else {
                f64::NEG_INFINITY
            }
        },
    )(input)
}

fn factor(input: &str) -> IResult<&str, f64> {
    map(
        tuple((alt((char('+'), char('-'))), parse_f64, char('x'))),
        |(sign, num, _)| {
            if sign == '+' { num } else { num.neg() }
        },
    )(input)
}

fn percent(input: &str) -> IResult<&str, f64> {
    map(
        tuple((alt((char('+'), char('-'))), parse_f64, char('%'))),
        |(sign, num, _)| {
            if sign == '+' { num } else { num.neg() }
        },
    )(input)
}

fn line_ending<'a>() -> impl FnMut(&'a str) -> IResult<&'a str, &'a str> {
    is_a("\r\n")
}

fn not_line_ending<'a>() -> impl FnMut(&'a str) -> IResult<&'a str, &'a str> {
    // Note: `not(line_ending)` doesn't work here, as it won't consume the matched characters
    is_not("\r\n")
}

/// Parser that matches the benchmark name ending sequence, excluding the two spaces
fn benchmark_name_end<'a>() -> impl FnMut(&'a str) -> IResult<&'a str, (&'a str, &'a str)> {
    // we only peek here so the `metric_line` parser can match the first spaces too
    tuple((line_ending(), peek(tag("  "))))
}

/// A parser that matches input until it encounters the benchmark name end sequence.
///
/// Similar to the `benchmark_name_end` parser, this parser addresses an unusual behavior in
/// `gungraun` (iai-callgrind) by allowing benchmark names to span multiple lines. While this
/// behavior is not a bug in `gungraun`, it deviates from the original intent. If there are multiple
/// lines they don't start with two spaces, so we can use that as test for the end of the benchmark
/// name.
fn not_benchmark_name_end<'a>() -> impl FnMut(&'a str) -> IResult<&'a str, &'a str> {
    // Ignore the `\r\n` possibility here and instead trim the benchmark name later
    take_until1("\n  ")
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
    fn test_with_dhat_first_then_callgrind() {
        let results = convert_file_path::<AdapterRustIaiCallgrind>(
            "./tool_output/rust/iai_callgrind/dhat-then-callgrind.txt",
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
    fn test_delta_with_infinity() {
        let results = convert_file_path::<AdapterRustIaiCallgrind>(
            "./tool_output/rust/iai_callgrind/delta_with_inf.txt",
        );

        assert_eq!(results.inner.len(), 2);

        {
            let expected = HashMap::from([
                (iai_callgrind::Instructions::SLUG_STR, 1_734.0),
                (iai_callgrind::L1Hits::SLUG_STR, 2_359.0),
                (iai_callgrind::L2Hits::SLUG_STR, 0.0),
                (iai_callgrind::RamHits::SLUG_STR, 3.0),
                (iai_callgrind::TotalReadWrite::SLUG_STR, 0.0),
                (iai_callgrind::EstimatedCycles::SLUG_STR, 2_464.0),
            ]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::bench_fibonacci_group::bench_fibonacci short:10",
            );
        }

        {
            let expected = HashMap::from([
                (iai_callgrind::Instructions::SLUG_STR, 26_214_734.0),
                (iai_callgrind::L1Hits::SLUG_STR, 35_638_619.0),
                (iai_callgrind::L2Hits::SLUG_STR, 0.0),
                (iai_callgrind::RamHits::SLUG_STR, 3.0),
                (iai_callgrind::TotalReadWrite::SLUG_STR, 35_638_622.0),
                (iai_callgrind::EstimatedCycles::SLUG_STR, 35_638_724.0),
            ]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::bench_fibonacci_group::bench_fibonacci long:30",
            );
        }
    }

    #[test]
    fn test_with_summary_and_regressions() {
        let results = convert_file_path::<AdapterRustIaiCallgrind>(
            "./tool_output/rust/iai_callgrind/with-summary-and-regressions.txt",
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

    #[test]
    fn test_without_cachesim() {
        use iai_callgrind::*;

        let results = convert_file_path::<AdapterRustIaiCallgrind>(
            "./tool_output/rust/iai_callgrind/without-cachesim.txt",
        );

        assert_eq!(results.inner.len(), 2);

        {
            let expected = HashMap::from([(Instructions::SLUG_STR, 1734.0)]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::bench_fibonacci_group::bench_fibonacci short:10",
            );
        }

        {
            let expected = HashMap::from([(Instructions::SLUG_STR, 26_214_734.0)]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::bench_fibonacci_group::bench_fibonacci long:30",
            );
        }
    }

    #[test]
    fn test_callgrind_mixed_order() {
        use iai_callgrind::*;

        let results = convert_file_path::<AdapterRustIaiCallgrind>(
            "./tool_output/rust/iai_callgrind/callgrind-mixed-order.txt",
        );

        assert_eq!(results.inner.len(), 2);

        let expected = HashMap::from([
            (Instructions::SLUG_STR, 1.0),
            (Dr::SLUG_STR, 2.0),
            (Dw::SLUG_STR, 3.0),
        ]);

        compare_benchmark(
            &expected,
            &results,
            "rust_iai_callgrind::custom_format::callgrind_format mixed_1",
        );

        compare_benchmark(
            &expected,
            &results,
            "rust_iai_callgrind::custom_format::callgrind_format mixed_2",
        );
    }

    #[test]
    fn test_callgrind_ll_hits() {
        use iai_callgrind::*;

        let results = convert_file_path::<AdapterRustIaiCallgrind>(
            "./tool_output/rust/iai_callgrind/callgrind-ll-hits.txt",
        );

        assert_eq!(results.inner.len(), 2);

        {
            let expected =
                HashMap::from([(Instructions::SLUG_STR, 10.0), (LLHits::SLUG_STR, 20.0)]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::custom_format::callgrind_format ll_hits",
            );
        }

        {
            let expected = HashMap::from([(Instructions::SLUG_STR, 1.0), (LLHits::SLUG_STR, 2.0)]);
            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::custom_format::callgrind_format ll_hits_mixed",
            );
        }
    }

    #[test]
    fn test_callgrind_all() {
        use iai_callgrind::*;

        let results = convert_file_path::<AdapterRustIaiCallgrind>(
            "./tool_output/rust/iai_callgrind/callgrind-all.txt",
        );

        assert_eq!(results.inner.len(), 2);

        {
            let expected = HashMap::from([
                (Instructions::SLUG_STR, 1.0),
                (Dr::SLUG_STR, 2.0),
                (Dw::SLUG_STR, 3.0),
                (I1mr::SLUG_STR, 4.0),
                (D1mr::SLUG_STR, 5.0),
                (D1mw::SLUG_STR, 6.0),
                (ILmr::SLUG_STR, 7.0),
                (DLmr::SLUG_STR, 8.0),
                (DLmw::SLUG_STR, 9.0),
                (I1MissRate::SLUG_STR, 10.0),
                (LLiMissRate::SLUG_STR, 11.0),
                (D1MissRate::SLUG_STR, 12.0),
                (LLdMissRate::SLUG_STR, 13.0),
                (LLMissRate::SLUG_STR, 14.0),
                (L1Hits::SLUG_STR, 15.0),
                (L2Hits::SLUG_STR, 16.0),
                (RamHits::SLUG_STR, 17.0),
                (L1HitRate::SLUG_STR, 18.0),
                (LLHitRate::SLUG_STR, 19.0),
                (RamHitRate::SLUG_STR, 20.0),
                (TotalReadWrite::SLUG_STR, 21.0),
                (EstimatedCycles::SLUG_STR, 22.0),
                (SysCount::SLUG_STR, 23.0),
                (SysTime::SLUG_STR, 24.0),
                (SysCpuTime::SLUG_STR, 25.0),
                (GlobalBusEvents::SLUG_STR, 26.0),
                (Bc::SLUG_STR, 27.0),
                (Bcm::SLUG_STR, 28.0),
                (Bi::SLUG_STR, 29.0),
                (Bim::SLUG_STR, 30.0),
                (ILdmr::SLUG_STR, 31.0),
                (DLdmr::SLUG_STR, 32.0),
                (DLdmw::SLUG_STR, 33.0),
            ]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::custom_format::callgrind_format all_with_wb",
            );
        }

        {
            let expected = HashMap::from([
                (Instructions::SLUG_STR, 1.0),
                (Dr::SLUG_STR, 2.0),
                (Dw::SLUG_STR, 3.0),
                (I1mr::SLUG_STR, 4.0),
                (D1mr::SLUG_STR, 5.0),
                (D1mw::SLUG_STR, 6.0),
                (ILmr::SLUG_STR, 7.0),
                (DLmr::SLUG_STR, 8.0),
                (DLmw::SLUG_STR, 9.0),
                (I1MissRate::SLUG_STR, 10.0),
                (LLiMissRate::SLUG_STR, 11.0),
                (D1MissRate::SLUG_STR, 12.0),
                (LLdMissRate::SLUG_STR, 13.0),
                (LLMissRate::SLUG_STR, 14.0),
                (L1Hits::SLUG_STR, 15.0),
                (L2Hits::SLUG_STR, 16.0),
                (RamHits::SLUG_STR, 17.0),
                (L1HitRate::SLUG_STR, 18.0),
                (LLHitRate::SLUG_STR, 19.0),
                (RamHitRate::SLUG_STR, 20.0),
                (TotalReadWrite::SLUG_STR, 21.0),
                (EstimatedCycles::SLUG_STR, 22.0),
                (SysCount::SLUG_STR, 23.0),
                (SysTime::SLUG_STR, 24.0),
                (SysCpuTime::SLUG_STR, 25.0),
                (GlobalBusEvents::SLUG_STR, 26.0),
                (Bc::SLUG_STR, 27.0),
                (Bcm::SLUG_STR, 28.0),
                (Bi::SLUG_STR, 29.0),
                (Bim::SLUG_STR, 30.0),
                (AcCost1::SLUG_STR, 31.0),
                (AcCost2::SLUG_STR, 32.0),
                (SpLoss1::SLUG_STR, 33.0),
                (SpLoss2::SLUG_STR, 34.0),
            ]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::custom_format::callgrind_format all_with_cachuse",
            );
        }
    }

    #[test]
    fn test_cachegrind() {
        use iai_callgrind::*;

        let results = convert_file_path::<AdapterRustIaiCallgrind>(
            "./tool_output/rust/iai_callgrind/cachegrind.txt",
        );

        assert_eq!(results.inner.len(), 3);

        {
            let expected = HashMap::from([(Instructions::SLUG_STR, 1.0)]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::custom_format::cachegrind_format just_instructions",
            );
        }

        {
            let expected = HashMap::from([
                (I1MissRate::SLUG_STR, 1.0),
                (Dr::SLUG_STR, 2.0),
                (EstimatedCycles::SLUG_STR, 3.0),
                (TotalReadWrite::SLUG_STR, 4.0),
            ]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::custom_format::cachegrind_format metrics_mixed",
            );
        }

        {
            let expected = HashMap::from([
                (Instructions::SLUG_STR, 1.0),
                (Dr::SLUG_STR, 2.0),
                (Dw::SLUG_STR, 3.0),
                (I1mr::SLUG_STR, 4.0),
                (D1mr::SLUG_STR, 5.0),
                (D1mw::SLUG_STR, 6.0),
                (ILmr::SLUG_STR, 7.0),
                (DLmr::SLUG_STR, 8.0),
                (DLmw::SLUG_STR, 9.0),
                (I1MissRate::SLUG_STR, 10.0),
                (LLiMissRate::SLUG_STR, 11.0),
                (D1MissRate::SLUG_STR, 12.0),
                (LLdMissRate::SLUG_STR, 13.0),
                (LLMissRate::SLUG_STR, 14.0),
                (L1Hits::SLUG_STR, 15.0),
                (L2Hits::SLUG_STR, 16.0),
                (RamHits::SLUG_STR, 17.0),
                (L1HitRate::SLUG_STR, 18.0),
                (LLHitRate::SLUG_STR, 19.0),
                (RamHitRate::SLUG_STR, 20.0),
                (TotalReadWrite::SLUG_STR, 21.0),
                (EstimatedCycles::SLUG_STR, 22.0),
            ]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::custom_format::cachegrind_format all_metrics",
            );
        }
    }

    #[test]
    fn test_memcheck() {
        use iai_callgrind::*;

        let results = convert_file_path::<AdapterRustIaiCallgrind>(
            "./tool_output/rust/iai_callgrind/memcheck.txt",
        );

        assert_eq!(results.inner.len(), 3);

        {
            let expected = HashMap::from([(MemcheckErrors::SLUG_STR, 1.0)]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::custom_format::memcheck_format just_errors",
            );
        }

        {
            let expected = HashMap::from([
                (MemcheckContexts::SLUG_STR, 1.0),
                (MemcheckSuppressedContexts::SLUG_STR, 2.0),
                (MemcheckSuppressedErrors::SLUG_STR, 3.0),
                (MemcheckErrors::SLUG_STR, 4.0),
            ]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::custom_format::memcheck_format metrics_mixed",
            );
        }

        {
            let expected = HashMap::from([
                (MemcheckErrors::SLUG_STR, 1.0),
                (MemcheckContexts::SLUG_STR, 2.0),
                (MemcheckSuppressedErrors::SLUG_STR, 3.0),
                (MemcheckSuppressedContexts::SLUG_STR, 4.0),
            ]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::custom_format::memcheck_format all_metrics",
            );
        }
    }

    #[test]
    fn test_helgrind() {
        use iai_callgrind::*;

        let results = convert_file_path::<AdapterRustIaiCallgrind>(
            "./tool_output/rust/iai_callgrind/helgrind.txt",
        );

        assert_eq!(results.inner.len(), 3);

        {
            let expected = HashMap::from([(HelgrindErrors::SLUG_STR, 1.0)]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::custom_format::helgrind_format just_errors",
            );
        }

        {
            let expected = HashMap::from([
                (HelgrindContexts::SLUG_STR, 1.0),
                (HelgrindSuppressedContexts::SLUG_STR, 2.0),
                (HelgrindSuppressedErrors::SLUG_STR, 3.0),
                (HelgrindErrors::SLUG_STR, 4.0),
            ]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::custom_format::helgrind_format metrics_mixed",
            );
        }

        {
            let expected = HashMap::from([
                (HelgrindErrors::SLUG_STR, 1.0),
                (HelgrindContexts::SLUG_STR, 2.0),
                (HelgrindSuppressedErrors::SLUG_STR, 3.0),
                (HelgrindSuppressedContexts::SLUG_STR, 4.0),
            ]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::custom_format::helgrind_format all_metrics",
            );
        }
    }

    #[test]
    fn test_drd() {
        use iai_callgrind::*;

        let results = convert_file_path::<AdapterRustIaiCallgrind>(
            "./tool_output/rust/iai_callgrind/drd.txt",
        );

        assert_eq!(results.inner.len(), 3);

        {
            let expected = HashMap::from([(DrdErrors::SLUG_STR, 1.0)]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::custom_format::drd_format just_errors",
            );
        }

        {
            let expected = HashMap::from([
                (DrdContexts::SLUG_STR, 1.0),
                (DrdSuppressedContexts::SLUG_STR, 2.0),
                (DrdSuppressedErrors::SLUG_STR, 3.0),
                (DrdErrors::SLUG_STR, 4.0),
            ]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::custom_format::drd_format metrics_mixed",
            );
        }

        {
            let expected = HashMap::from([
                (DrdErrors::SLUG_STR, 1.0),
                (DrdContexts::SLUG_STR, 2.0),
                (DrdSuppressedErrors::SLUG_STR, 3.0),
                (DrdSuppressedContexts::SLUG_STR, 4.0),
            ]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::custom_format::drd_format all_metrics",
            );
        }
    }

    #[test]
    fn test_name_multiple_lines() {
        use iai_callgrind::*;

        let results = convert_file_path::<AdapterRustIaiCallgrind>(
            "./tool_output/rust/iai_callgrind/name_on_multiple_lines.txt",
        );

        assert_eq!(results.inner.len(), 2);

        {
            let mut expected = HashMap::new();

            expected.extend([
                (Instructions::SLUG_STR, 1.0),
                (L1Hits::SLUG_STR, 2.0),
                (L2Hits::SLUG_STR, 3.0),
                (RamHits::SLUG_STR, 4.0),
                (TotalReadWrite::SLUG_STR, 5.0),
                (EstimatedCycles::SLUG_STR, 6.0),
            ]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::bench::multiple_lines id:string with two\nlines",
            );
        }

        {
            let mut expected = HashMap::new();

            expected.extend([
                (Instructions::SLUG_STR, 7.0),
                (L1Hits::SLUG_STR, 8.0),
                (L2Hits::SLUG_STR, 9.0),
                (RamHits::SLUG_STR, 10.0),
                (TotalReadWrite::SLUG_STR, 11.0),
                (EstimatedCycles::SLUG_STR, 12.0),
            ]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::bench::multiple_lines id:string with multiple\nlines\nand one more",
            );
        }
    }

    #[test]
    fn test_name_multiple_lines_mixed() {
        use iai_callgrind::*;

        let results = convert_file_path::<AdapterRustIaiCallgrind>(
            "./tool_output/rust/iai_callgrind/name_on_multiple_lines_mixed.txt",
        );

        assert_eq!(results.inner.len(), 3);

        {
            let mut expected = HashMap::new();

            expected.extend([
                (Instructions::SLUG_STR, 1.0),
                (L1Hits::SLUG_STR, 2.0),
                (L2Hits::SLUG_STR, 3.0),
                (RamHits::SLUG_STR, 4.0),
                (TotalReadWrite::SLUG_STR, 5.0),
                (EstimatedCycles::SLUG_STR, 6.0),
            ]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::bench::multiple_lines id:first with one line",
            );
        }

        {
            let mut expected = HashMap::new();

            expected.extend([
                (Instructions::SLUG_STR, 7.0),
                (L1Hits::SLUG_STR, 8.0),
                (L2Hits::SLUG_STR, 9.0),
                (RamHits::SLUG_STR, 10.0),
                (TotalReadWrite::SLUG_STR, 11.0),
                (EstimatedCycles::SLUG_STR, 12.0),
            ]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::bench::multiple_lines id:two\nlines",
            );
        }

        {
            let mut expected = HashMap::new();

            expected.extend([
                (Instructions::SLUG_STR, 13.0),
                (L1Hits::SLUG_STR, 14.0),
                (L2Hits::SLUG_STR, 15.0),
                (RamHits::SLUG_STR, 16.0),
                (TotalReadWrite::SLUG_STR, 17.0),
                (EstimatedCycles::SLUG_STR, 18.0),
            ]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::bench::multiple_lines id:last with one line",
            );
        }
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
