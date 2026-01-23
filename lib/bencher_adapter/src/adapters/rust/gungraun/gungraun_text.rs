use std::ops::Neg as _;

use bencher_json::{
    project::{
        measure::built_in::{gungraun, BuiltInMeasure as _},
        report::JsonAverage,
    },
    BenchmarkName, JsonNewMetric,
};
use nom::{
    branch::alt,
    bytes::complete::{is_a, is_not, tag, take_until1},
    character::complete::{char, space0, space1},
    combinator::{map, opt, peek, recognize},
    multi::{many0, many1},
    sequence::{delimited, preceded, terminated, tuple},
    IResult,
};

use crate::{
    adapters::util::parse_f64,
    results::adapter_results::{AdapterResults, GungraunMeasure},
    Adaptable, Settings,
};

pub struct AdapterRustGungraunText;

impl Adaptable for AdapterRustGungraunText {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        match settings.average {
            None => {},
            Some(JsonAverage::Mean | JsonAverage::Median) => {
                return None; // 'gungraun' results are for a single run only.
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

        AdapterResults::new_gungraun(benchmarks)
    }
}

fn multiple_benchmarks<'a>(
) -> impl FnMut(&'a str) -> IResult<&'a str, Vec<(BenchmarkName, Vec<GungraunMeasure>)>> {
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

fn single_benchmark<'a>(
) -> impl FnMut(&'a str) -> IResult<&'a str, Option<(BenchmarkName, Vec<GungraunMeasure>)>> {
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

fn callgrind_tool_measures<'a>() -> impl FnMut(&'a str) -> IResult<&'a str, Vec<GungraunMeasure>> {
    map(
        preceded(opt(tool_name_line("CALLGRIND")), many1(metric_line())),
        |metrics| {
            metrics
                .into_iter()
                .map(|(metric_name, json)| match metric_name.as_str() {
                    gungraun::Instructions::NAME_STR => GungraunMeasure::Instructions(json),
                    gungraun::L1Hits::NAME_STR => GungraunMeasure::L1Hits(json),
                    gungraun::L2Hits::NAME_STR => GungraunMeasure::L2Hits(json),
                    gungraun::LLHits::NAME_STR => GungraunMeasure::LLHits(json),
                    gungraun::RamHits::NAME_STR => GungraunMeasure::RamHits(json),
                    gungraun::TotalReadWrite::NAME_STR => GungraunMeasure::TotalReadWrite(json),
                    gungraun::EstimatedCycles::NAME_STR => GungraunMeasure::EstimatedCycles(json),
                    gungraun::Dr::NAME_STR => GungraunMeasure::DataCacheReads(json),
                    gungraun::Dw::NAME_STR => GungraunMeasure::DataCacheWrites(json),
                    gungraun::I1mr::NAME_STR => GungraunMeasure::L1InstrCacheReadMisses(json),
                    gungraun::D1mr::NAME_STR => GungraunMeasure::L1DataCacheReadMisses(json),
                    gungraun::D1mw::NAME_STR => GungraunMeasure::L1DataCacheWriteMisses(json),
                    gungraun::ILmr::NAME_STR => GungraunMeasure::LLInstrCacheReadMisses(json),
                    gungraun::DLmr::NAME_STR => GungraunMeasure::LLDataCacheReadMisses(json),
                    gungraun::DLmw::NAME_STR => GungraunMeasure::LLDataCacheWriteMisses(json),
                    gungraun::I1MissRate::NAME_STR => GungraunMeasure::L1InstrCacheMissRate(json),
                    gungraun::LLiMissRate::NAME_STR => GungraunMeasure::LLInstrCacheMissRate(json),
                    gungraun::D1MissRate::NAME_STR => GungraunMeasure::L1DataCacheMissRate(json),
                    gungraun::LLdMissRate::NAME_STR => GungraunMeasure::LLDataCacheMissRate(json),
                    gungraun::LLMissRate::NAME_STR => GungraunMeasure::LLCacheMissRate(json),
                    gungraun::L1HitRate::NAME_STR => GungraunMeasure::L1HitRate(json),
                    gungraun::LLHitRate::NAME_STR => GungraunMeasure::LLHitRate(json),
                    gungraun::RamHitRate::NAME_STR => GungraunMeasure::RamHitRate(json),
                    gungraun::SysCount::NAME_STR => GungraunMeasure::NumberSystemCalls(json),
                    gungraun::SysTime::NAME_STR => GungraunMeasure::TimeSystemCalls(json),
                    gungraun::SysCpuTime::NAME_STR => GungraunMeasure::CpuTimeSystemCalls(json),
                    gungraun::GlobalBusEvents::NAME_STR => GungraunMeasure::GlobalBusEvents(json),
                    gungraun::Bc::NAME_STR => GungraunMeasure::ExecutedConditionalBranches(json),
                    gungraun::Bcm::NAME_STR => {
                        GungraunMeasure::MispredictedConditionalBranches(json)
                    },
                    gungraun::Bi::NAME_STR => GungraunMeasure::ExecutedIndirectBranches(json),
                    gungraun::Bim::NAME_STR => GungraunMeasure::MispredictedIndirectBranches(json),
                    gungraun::ILdmr::NAME_STR => GungraunMeasure::DirtyMissInstructionRead(json),
                    gungraun::DLdmr::NAME_STR => GungraunMeasure::DirtyMissDataRead(json),
                    gungraun::DLdmw::NAME_STR => GungraunMeasure::DirtyMissDataWrite(json),
                    gungraun::AcCost1::NAME_STR => GungraunMeasure::L1BadTemporalLocality(json),
                    gungraun::AcCost2::NAME_STR => GungraunMeasure::LLBadTemporalLocality(json),
                    gungraun::SpLoss1::NAME_STR => GungraunMeasure::L1BadSpatialLocality(json),
                    gungraun::SpLoss2::NAME_STR => GungraunMeasure::LLBadSpatialLocality(json),
                    _ => GungraunMeasure::Unknown,
                })
                .collect()
        },
    )
}

fn cachegrind_tool_measures<'a>() -> impl FnMut(&'a str) -> IResult<&'a str, Vec<GungraunMeasure>> {
    map(
        preceded(tool_name_line("CACHEGRIND"), many1(metric_line())),
        |metrics| {
            metrics
                .into_iter()
                .map(|(metric_name, json)| match metric_name.as_str() {
                    gungraun::Instructions::NAME_STR => GungraunMeasure::Instructions(json),
                    gungraun::L1Hits::NAME_STR => GungraunMeasure::L1Hits(json),
                    gungraun::L2Hits::NAME_STR => GungraunMeasure::L2Hits(json),
                    gungraun::LLHits::NAME_STR => GungraunMeasure::LLHits(json),
                    gungraun::RamHits::NAME_STR => GungraunMeasure::RamHits(json),
                    gungraun::TotalReadWrite::NAME_STR => GungraunMeasure::TotalReadWrite(json),
                    gungraun::EstimatedCycles::NAME_STR => GungraunMeasure::EstimatedCycles(json),
                    gungraun::Dr::NAME_STR => GungraunMeasure::DataCacheReads(json),
                    gungraun::Dw::NAME_STR => GungraunMeasure::DataCacheWrites(json),
                    gungraun::I1mr::NAME_STR => GungraunMeasure::L1InstrCacheReadMisses(json),
                    gungraun::D1mr::NAME_STR => GungraunMeasure::L1DataCacheReadMisses(json),
                    gungraun::D1mw::NAME_STR => GungraunMeasure::L1DataCacheWriteMisses(json),
                    gungraun::ILmr::NAME_STR => GungraunMeasure::LLInstrCacheReadMisses(json),
                    gungraun::DLmr::NAME_STR => GungraunMeasure::LLDataCacheReadMisses(json),
                    gungraun::DLmw::NAME_STR => GungraunMeasure::LLDataCacheWriteMisses(json),
                    gungraun::I1MissRate::NAME_STR => GungraunMeasure::L1InstrCacheMissRate(json),
                    gungraun::LLiMissRate::NAME_STR => GungraunMeasure::LLInstrCacheMissRate(json),
                    gungraun::D1MissRate::NAME_STR => GungraunMeasure::L1DataCacheMissRate(json),
                    gungraun::LLdMissRate::NAME_STR => GungraunMeasure::LLDataCacheMissRate(json),
                    gungraun::LLMissRate::NAME_STR => GungraunMeasure::LLCacheMissRate(json),
                    gungraun::L1HitRate::NAME_STR => GungraunMeasure::L1HitRate(json),
                    gungraun::LLHitRate::NAME_STR => GungraunMeasure::LLHitRate(json),
                    gungraun::RamHitRate::NAME_STR => GungraunMeasure::RamHitRate(json),
                    _ => GungraunMeasure::Unknown,
                })
                .collect()
        },
    )
}

fn dhat_tool_measures<'a>() -> impl FnMut(&'a str) -> IResult<&'a str, Vec<GungraunMeasure>> {
    map(
        preceded(tool_name_line("DHAT"), many1(metric_line())),
        |metrics| {
            metrics
                .into_iter()
                .map(|(metric_name, json)| match metric_name.as_str() {
                    gungraun::TotalBytes::NAME_STR => GungraunMeasure::TotalBytes(json),
                    gungraun::TotalBlocks::NAME_STR => GungraunMeasure::TotalBlocks(json),
                    gungraun::AtTGmaxBytes::NAME_STR => GungraunMeasure::AtTGmaxBytes(json),
                    gungraun::AtTGmaxBlocks::NAME_STR => GungraunMeasure::AtTGmaxBlocks(json),
                    gungraun::AtTEndBytes::NAME_STR => GungraunMeasure::AtTEndBytes(json),
                    gungraun::AtTEndBlocks::NAME_STR => GungraunMeasure::AtTEndBlocks(json),
                    gungraun::ReadsBytes::NAME_STR => GungraunMeasure::ReadsBytes(json),
                    gungraun::WritesBytes::NAME_STR => GungraunMeasure::WritesBytes(json),
                    _ => GungraunMeasure::Unknown,
                })
                .collect()
        },
    )
}

fn memcheck_tool_measures<'a>() -> impl FnMut(&'a str) -> IResult<&'a str, Vec<GungraunMeasure>> {
    map(
        preceded(tool_name_line("MEMCHECK"), many1(metric_line())),
        |metrics| {
            metrics
                .into_iter()
                .map(|(metric_name, json)| match metric_name.as_str() {
                    gungraun::MemcheckErrors::NAME_STR => GungraunMeasure::MemcheckErrors(json),
                    gungraun::MemcheckContexts::NAME_STR => GungraunMeasure::MemcheckContexts(json),
                    gungraun::MemcheckSuppressedErrors::NAME_STR => {
                        GungraunMeasure::MemcheckSuppressedErrors(json)
                    },
                    gungraun::MemcheckSuppressedContexts::NAME_STR => {
                        GungraunMeasure::MemcheckSuppressedContexts(json)
                    },
                    _ => GungraunMeasure::Unknown,
                })
                .collect()
        },
    )
}

fn helgrind_tool_measures<'a>() -> impl FnMut(&'a str) -> IResult<&'a str, Vec<GungraunMeasure>> {
    map(
        preceded(tool_name_line("HELGRIND"), many1(metric_line())),
        |metrics| {
            metrics
                .into_iter()
                .map(|(metric_name, json)| match metric_name.as_str() {
                    gungraun::HelgrindErrors::NAME_STR => GungraunMeasure::HelgrindErrors(json),
                    gungraun::HelgrindContexts::NAME_STR => GungraunMeasure::HelgrindContexts(json),
                    gungraun::HelgrindSuppressedErrors::NAME_STR => {
                        GungraunMeasure::HelgrindSuppressedErrors(json)
                    },
                    gungraun::HelgrindSuppressedContexts::NAME_STR => {
                        GungraunMeasure::HelgrindSuppressedContexts(json)
                    },
                    _ => GungraunMeasure::Unknown,
                })
                .collect()
        },
    )
}

fn drd_tool_measures<'a>() -> impl FnMut(&'a str) -> IResult<&'a str, Vec<GungraunMeasure>> {
    map(
        preceded(tool_name_line("DRD"), many1(metric_line())),
        |metrics| {
            metrics
                .into_iter()
                .map(|(metric_name, json)| match metric_name.as_str() {
                    gungraun::DrdErrors::NAME_STR => GungraunMeasure::DrdErrors(json),
                    gungraun::DrdContexts::NAME_STR => GungraunMeasure::DrdContexts(json),
                    gungraun::DrdSuppressedErrors::NAME_STR => {
                        GungraunMeasure::DrdSuppressedErrors(json)
                    },
                    gungraun::DrdSuppressedContexts::NAME_STR => {
                        GungraunMeasure::DrdSuppressedContexts(json)
                    },
                    _ => GungraunMeasure::Unknown,
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
            if sign == '+' {
                num
            } else {
                num.neg()
            }
        },
    )(input)
}

fn percent(input: &str) -> IResult<&str, f64> {
    map(
        tuple((alt((char('+'), char('-'))), parse_f64, char('%'))),
        |(sign, num, _)| {
            if sign == '+' {
                num
            } else {
                num.neg()
            }
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
pub(crate) mod test_rust_gungraun_text {
    use crate::{adapters::test_util::convert_file_path, AdapterResults};
    use bencher_json::project::measure::built_in::{gungraun, BuiltInMeasure as _};
    use ordered_float::OrderedFloat;
    use pretty_assertions::assert_eq;

    use super::AdapterRustGungraunText;
    use std::collections::HashMap;

    #[test]
    fn without_optional_metrics() {
        let results = convert_file_path::<AdapterRustGungraunText>(
            "./tool_output/rust/gungraun/without-optional-metrics.txt",
        );

        validate_adapter_rust_gungraun(&results, &OptionalMetrics::default());
    }

    #[test]
    fn with_dhat() {
        let results = convert_file_path::<AdapterRustGungraunText>(
            "./tool_output/rust/gungraun/with-dhat.txt",
        );

        validate_adapter_rust_gungraun(
            &results,
            &OptionalMetrics {
                dhat: true,
                ..Default::default()
            },
        );
    }

    #[test]
    fn with_dhat_first_then_callgrind() {
        let results = convert_file_path::<AdapterRustGungraunText>(
            "./tool_output/rust/gungraun/dhat-then-callgrind.txt",
        );

        validate_adapter_rust_gungraun(
            &results,
            &OptionalMetrics {
                dhat: true,
                ..Default::default()
            },
        );
    }

    #[test]
    fn with_dhat_and_global_bus_events() {
        let results = convert_file_path::<AdapterRustGungraunText>(
            "./tool_output/rust/gungraun/with-dhat-and-global-bus-events.txt",
        );

        validate_adapter_rust_gungraun(
            &results,
            &OptionalMetrics {
                dhat: true,
                global_bus_events: true,
            },
        );
    }

    #[test]
    fn delta() {
        let results =
            convert_file_path::<AdapterRustGungraunText>("./tool_output/rust/gungraun/delta.txt");

        validate_adapter_rust_gungraun(&results, &OptionalMetrics::default());
    }

    #[test]
    fn delta_with_infinity() {
        let results = convert_file_path::<AdapterRustGungraunText>(
            "./tool_output/rust/gungraun/delta_with_inf.txt",
        );

        assert_eq!(results.inner.len(), 2);

        {
            let expected = HashMap::from([
                (gungraun::Instructions::SLUG_STR, 1_734.0),
                (gungraun::L1Hits::SLUG_STR, 2_359.0),
                (gungraun::L2Hits::SLUG_STR, 0.0),
                (gungraun::RamHits::SLUG_STR, 3.0),
                (gungraun::TotalReadWrite::SLUG_STR, 0.0),
                (gungraun::EstimatedCycles::SLUG_STR, 2_464.0),
            ]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::bench_fibonacci_group::bench_fibonacci short:10",
            );
        }

        {
            let expected = HashMap::from([
                (gungraun::Instructions::SLUG_STR, 26_214_734.0),
                (gungraun::L1Hits::SLUG_STR, 35_638_619.0),
                (gungraun::L2Hits::SLUG_STR, 0.0),
                (gungraun::RamHits::SLUG_STR, 3.0),
                (gungraun::TotalReadWrite::SLUG_STR, 35_638_622.0),
                (gungraun::EstimatedCycles::SLUG_STR, 35_638_724.0),
            ]);

            compare_benchmark(
                &expected,
                &results,
                "rust_iai_callgrind::bench_fibonacci_group::bench_fibonacci long:30",
            );
        }
    }

    #[test]
    fn with_summary_and_regressions() {
        let results = convert_file_path::<AdapterRustGungraunText>(
            "./tool_output/rust/gungraun/with-summary-and-regressions.txt",
        );

        validate_adapter_rust_gungraun(&results, &OptionalMetrics::default());
    }

    #[test]
    fn with_gungraun_summary() {
        let results = convert_file_path::<AdapterRustGungraunText>(
            "./tool_output/rust/gungraun/with-gungraun-summary.txt",
        );

        validate_adapter_rust_gungraun(&results, &OptionalMetrics::default());
    }

    #[test]
    fn ansi_escapes_issue_345() {
        let results = convert_file_path::<AdapterRustGungraunText>(
            "./tool_output/rust/gungraun/ansi-escapes.txt",
        );

        validate_adapter_rust_gungraun(&results, &OptionalMetrics::default());
    }

    #[test]
    fn with_ge() {
        let results =
            convert_file_path::<AdapterRustGungraunText>("./tool_output/rust/gungraun/with-ge.txt");

        validate_adapter_rust_gungraun(
            &results,
            &OptionalMetrics {
                global_bus_events: true,
                ..Default::default()
            },
        );
    }

    #[test]
    fn without_cachesim() {
        use gungraun::*;

        let results = convert_file_path::<AdapterRustGungraunText>(
            "./tool_output/rust/gungraun/without-cachesim.txt",
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
    fn callgrind_mixed_order() {
        use gungraun::*;

        let results = convert_file_path::<AdapterRustGungraunText>(
            "./tool_output/rust/gungraun/callgrind-mixed-order.txt",
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
    fn callgrind_ll_hits() {
        use gungraun::*;

        let results = convert_file_path::<AdapterRustGungraunText>(
            "./tool_output/rust/gungraun/callgrind-ll-hits.txt",
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
    fn callgrind_all() {
        use gungraun::*;

        let results = convert_file_path::<AdapterRustGungraunText>(
            "./tool_output/rust/gungraun/callgrind-all.txt",
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
    fn cachegrind() {
        use gungraun::*;

        let results = convert_file_path::<AdapterRustGungraunText>(
            "./tool_output/rust/gungraun/cachegrind.txt",
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
    fn memcheck() {
        use gungraun::*;

        let results = convert_file_path::<AdapterRustGungraunText>(
            "./tool_output/rust/gungraun/memcheck.txt",
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
    fn helgrind() {
        use gungraun::*;

        let results = convert_file_path::<AdapterRustGungraunText>(
            "./tool_output/rust/gungraun/helgrind.txt",
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
    fn drd() {
        use gungraun::*;

        let results =
            convert_file_path::<AdapterRustGungraunText>("./tool_output/rust/gungraun/drd.txt");

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
    fn name_multiple_lines() {
        use gungraun::*;

        let results = convert_file_path::<AdapterRustGungraunText>(
            "./tool_output/rust/gungraun/name_on_multiple_lines.txt",
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
    fn name_multiple_lines_mixed() {
        use gungraun::*;

        let results = convert_file_path::<AdapterRustGungraunText>(
            "./tool_output/rust/gungraun/name_on_multiple_lines_mixed.txt",
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

    pub fn validate_adapter_rust_gungraun(
        results: &AdapterResults,
        optional_metrics: &OptionalMetrics,
    ) {
        assert_eq!(results.inner.len(), 2);

        {
            let mut expected = HashMap::new();

            expected.extend([
                (gungraun::Instructions::SLUG_STR, 1_734.0),
                (gungraun::L1Hits::SLUG_STR, 2_359.0),
                (gungraun::L2Hits::SLUG_STR, 0.0),
                (gungraun::RamHits::SLUG_STR, 3.0),
                (gungraun::TotalReadWrite::SLUG_STR, 2_362.0),
                (gungraun::EstimatedCycles::SLUG_STR, 2_464.0),
            ]);

            if optional_metrics.global_bus_events {
                expected.insert(gungraun::GlobalBusEvents::SLUG_STR, 2.0);
            }

            if optional_metrics.dhat {
                expected.extend([
                    (gungraun::TotalBytes::SLUG_STR, 29_499.0),
                    (gungraun::TotalBlocks::SLUG_STR, 2_806.0),
                    (gungraun::AtTGmaxBytes::SLUG_STR, 378.0),
                    (gungraun::AtTGmaxBlocks::SLUG_STR, 34.0),
                    (gungraun::AtTEndBytes::SLUG_STR, 0.0),
                    (gungraun::AtTEndBlocks::SLUG_STR, 0.0),
                    (gungraun::ReadsBytes::SLUG_STR, 57_725.0),
                    (gungraun::WritesBytes::SLUG_STR, 73_810.0),
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
                (gungraun::Instructions::SLUG_STR, 26_214_734.0),
                (gungraun::L1Hits::SLUG_STR, 35_638_619.0),
                (gungraun::L2Hits::SLUG_STR, 0.0),
                (gungraun::RamHits::SLUG_STR, 3.0),
                (gungraun::TotalReadWrite::SLUG_STR, 35_638_622.0),
                (gungraun::EstimatedCycles::SLUG_STR, 35_638_724.0),
            ]);

            if optional_metrics.global_bus_events {
                expected.insert(gungraun::GlobalBusEvents::SLUG_STR, 10.0);
            }

            if optional_metrics.dhat {
                expected.extend([
                    (gungraun::TotalBytes::SLUG_STR, 26_294_939.0),
                    (gungraun::TotalBlocks::SLUG_STR, 2_328_086.0),
                    (gungraun::AtTGmaxBytes::SLUG_STR, 933_718.0),
                    (gungraun::AtTGmaxBlocks::SLUG_STR, 18_344.0),
                    (gungraun::AtTEndBytes::SLUG_STR, 0.0),
                    (gungraun::AtTEndBlocks::SLUG_STR, 0.0),
                    (gungraun::ReadsBytes::SLUG_STR, 47_577_425.0),
                    (gungraun::WritesBytes::SLUG_STR, 37_733_810.0),
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
