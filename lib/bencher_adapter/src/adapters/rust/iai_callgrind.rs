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

// TODO: char instead of single character tags ??

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
                    is_not(":\r\n"),
                    tag("::"),
                    is_not(":\r\n"),
                    tag("::"),
                    not_line_ending(),
                ))),
                line_ending(),
            ),
            // TODO: permutation. The order isn't fixed
            // Callgrind tool if it was enabled:
            opt(callgrind_tool_measures()),
            // Add DHAT tool measures if it was enabled:
            opt(dhat_tool_measures()),
        )),
        |(benchmark_name, callgrind_measures_1, dhat_measures)| {
            let benchmark_name = benchmark_name.parse().ok()?;

            let mut measures = vec![];
            measures.extend(callgrind_measures_1.into_iter().flatten());
            measures.extend(dhat_measures.into_iter().flatten());

            Some((benchmark_name, measures))
        },
    )
}

fn callgrind_tool_measures<'a>() -> impl FnMut(&'a str) -> IResult<&'a str, Vec<IaiCallgrindMeasure>>
{
    map(
        preceded(opt(tool_name_line("CALLGRIND")), many0(metric_line())),
        |metrics| {
            metrics
                .into_iter()
                .map(|(metric_name, json)| match metric_name.as_str() {
                    iai_callgrind::Instructions::NAME_STR => {
                        IaiCallgrindMeasure::Instructions(json)
                    },
                    iai_callgrind::L1Hits::NAME_STR => IaiCallgrindMeasure::L1Hits(json),
                    iai_callgrind::L2Hits::NAME_STR => IaiCallgrindMeasure::L2Hits(json),
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
                    _ => IaiCallgrindMeasure::Unknown(json),
                })
                .collect()
        },
    )
}

fn dhat_tool_measures<'a>() -> impl FnMut(&'a str) -> IResult<&'a str, Vec<IaiCallgrindMeasure>> {
    map(
        preceded(
            // TODO: not opt
            opt(tool_name_line("DHAT")),
            tuple((
                // TODO: all opt
                metric_line_u64(iai_callgrind::TotalBytes::NAME_STR),
                metric_line_u64(iai_callgrind::TotalBlocks::NAME_STR),
                metric_line_u64(iai_callgrind::AtTGmaxBytes::NAME_STR),
                metric_line_u64(iai_callgrind::AtTGmaxBlocks::NAME_STR),
                metric_line_u64(iai_callgrind::AtTEndBytes::NAME_STR),
                metric_line_u64(iai_callgrind::AtTEndBlocks::NAME_STR),
                metric_line_u64(iai_callgrind::ReadsBytes::NAME_STR),
                metric_line_u64(iai_callgrind::WritesBytes::NAME_STR),
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

// TODO: char instead of tag
fn metric_line<'a>() -> impl FnMut(&'a str) -> IResult<&'a str, (String, JsonNewMetric)> {
    map(
        tuple((
            space0,
            is_not(":\r\n"),
            tag(":"),
            space1,
            // the current run value:
            parse_f64,
            tag("|"),
            alt((
                // No previous run:
                // TODO: USE delimiter instead of tag
                recognize(tuple((tag("N/A"), space1, tag("(*********)")))),
                // Comparison to previous run:
                recognize(tuple((
                    parse_f64,
                    space0,
                    alt((
                        // TODO: USE delimiter instead of tag
                        recognize(tag("(No change)")),
                        recognize(tuple((
                            // TODO: infinity??
                            delimited(
                                tag("("),
                                tuple((alt((tag("+"), tag("-"))), parse_f64, tag("%"))),
                                tag(")"),
                            ),
                            space1,
                            // TODO: infinity
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

fn metric_line_u64<'a>(
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
        dbg!(&actual);
        assert_eq!(actual.inner.len(), expected.len());

        for (key, value) in expected {
            let metric = actual.get(key).unwrap();
            assert_eq!(metric.value, OrderedFloat::from(*value));
            assert_eq!(metric.lower_value, None);
            assert_eq!(metric.upper_value, None);
        }
    }
}
