use bencher_json::{BenchmarkName, JsonNewMetric, project::report::JsonAverage};

use gungraun_summary::{
    either_or_both::EitherOrBoth,
    util::SummaryByVersion,
    v6::{
        BenchmarkSummary, CachegrindMetric, DhatMetric, ErrorMetric, EventKind, Metric,
        MetricsDiff, MetricsSummary, ToolMetricSummary, ValgrindTool,
    },
};
use std::fmt::Write as _;
use std::str::Lines;

use crate::{Adaptable, AdapterResults, Settings, results::adapter_results::GungraunMeasure};

pub struct AdapterRustGungraunJson;

impl Adaptable for AdapterRustGungraunJson {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        match settings.average {
            None => {},
            Some(JsonAverage::Mean | JsonAverage::Median) => {
                return None; // 'gungraun' results are for a single run only.
            },
        }

        // Clean up the input by removing ANSI escape codes:
        let input = strip_ansi_escapes::strip_str(input);

        parse_multiple(&input).and_then(AdapterResults::new_gungraun)
    }
}

/// Parse json output in pretty and ndjson format
///
/// Although it is currently not a possibility with gungraun cli/env options, this parser handles
/// mixed pretty and ndjson format making bencher future proof in that regard. The parser also
/// ignores intermixed json output from other tools or cargo itself, for example `cargo bench
/// --message-format=json`.
fn parse_multiple(input: &str) -> Option<Vec<(BenchmarkName, Vec<GungraunMeasure>)>> {
    let mut lines = input.lines();

    let mut parsed = vec![];
    while let Some(line) = lines.next() {
        let trimmed = line.trim();
        if trimmed == "{" {
            let indent = line.len() - line.trim_start().len();
            if let Some(one) = parse_benchmark_pretty(&mut lines, indent) {
                parsed.push(one);
            }
        } else if trimmed.starts_with('{')
            && trimmed.ends_with('}')
            && let Some(one) = parse_benchmark(line)
        {
            parsed.push(one);
        }
    }

    (!parsed.is_empty()).then_some(parsed)
}

fn parse_benchmark_pretty(
    lines: &mut Lines<'_>,
    indent: usize,
) -> Option<(BenchmarkName, Vec<GungraunMeasure>)> {
    let mut payload = "{".to_owned();
    for line in lines {
        writeln!(payload, "{line}").ok()?;

        let trimmed = line.trim();
        if trimmed == "}" && line.len() - line.trim_start().len() == indent {
            break;
        }
    }

    parse_benchmark(payload.as_str())
}

fn parse_benchmark(input: &str) -> Option<(BenchmarkName, Vec<GungraunMeasure>)> {
    let (name, summary) = parse_json(input.as_bytes())?;

    let measures = summary
        .profiles
        .0
        .into_iter()
        .flat_map(|profile| match profile.summaries.total.summary {
            ToolMetricSummary::Cachegrind(metrics_summary) => {
                parse_cachegrind_metrics_summary(metrics_summary)
            },
            ToolMetricSummary::Callgrind(metrics_summary) => {
                parse_callgrind_metrics_summary(metrics_summary)
            },
            ToolMetricSummary::Dhat(metrics_summary) => parse_dhat_metrics_summary(metrics_summary),
            ToolMetricSummary::ErrorTool(metrics_summary) => match profile.tool {
                ValgrindTool::Memcheck => parse_memcheck_metrics_summary(metrics_summary),
                ValgrindTool::Helgrind => parse_helgind_metrics_summary(metrics_summary),
                ValgrindTool::DRD => parse_drd_metrics_summary(metrics_summary),
                // These other tools are no error tools and are already covered
                ValgrindTool::Callgrind
                | ValgrindTool::Cachegrind
                | ValgrindTool::DHAT
                // These tools don't have a metric output (covered by `ToolMetricSummary::None`)
                | ValgrindTool::Massif
                | ValgrindTool::BBV => vec![],
            },
            ToolMetricSummary::None => vec![],
        })
        .collect();

    Some((name, measures))
}

fn parse_json(input: &[u8]) -> Option<(BenchmarkName, BenchmarkSummary)> {
    // Using the version aware gungraun_summary parsing method to simplify adapting gungraun summary
    // version updates. At the moment there's just v6
    let summary = match gungraun_summary::util::parse_slice(input) {
        Ok(summary_by_version) => match summary_by_version {
            SummaryByVersion::V6(benchmark_summary) => benchmark_summary,
            _ => return None,
        },
        Err(_) => return None,
    };

    let name: BenchmarkName = if let Some(id) = &summary.id {
        format!("{}::{id}", summary.module_path)
    } else {
        summary.module_path.clone()
    }
    .parse()
    .ok()?;

    Some((name, summary))
}

fn parse_drd_metrics_summary(metrics_summary: MetricsSummary<ErrorMetric>) -> Vec<GungraunMeasure> {
    metrics_summary
        .0
        .into_iter()
        .filter_map(|(metric, diff)| diff_to_json(&diff).map(|json| (metric, json)))
        .map(|(metric, json)| match metric {
            ErrorMetric::Contexts => GungraunMeasure::DrdContexts(json),
            ErrorMetric::Errors => GungraunMeasure::DrdErrors(json),
            ErrorMetric::SuppressedContexts => GungraunMeasure::DrdSuppressedContexts(json),
            ErrorMetric::SuppressedErrors => GungraunMeasure::DrdSuppressedErrors(json),
        })
        .collect()
}

fn parse_helgind_metrics_summary(
    metrics_summary: MetricsSummary<ErrorMetric>,
) -> Vec<GungraunMeasure> {
    metrics_summary
        .0
        .into_iter()
        .filter_map(|(metric, diff)| diff_to_json(&diff).map(|json| (metric, json)))
        .map(|(metric, json)| match metric {
            ErrorMetric::Contexts => GungraunMeasure::HelgrindContexts(json),
            ErrorMetric::Errors => GungraunMeasure::HelgrindErrors(json),
            ErrorMetric::SuppressedContexts => GungraunMeasure::HelgrindSuppressedContexts(json),
            ErrorMetric::SuppressedErrors => GungraunMeasure::HelgrindSuppressedErrors(json),
        })
        .collect()
}

fn parse_memcheck_metrics_summary(
    metrics_summary: MetricsSummary<ErrorMetric>,
) -> Vec<GungraunMeasure> {
    metrics_summary
        .0
        .into_iter()
        .filter_map(|(metric, diff)| diff_to_json(&diff).map(|json| (metric, json)))
        .map(|(metric, json)| match metric {
            ErrorMetric::Contexts => GungraunMeasure::MemcheckContexts(json),
            ErrorMetric::Errors => GungraunMeasure::MemcheckErrors(json),
            ErrorMetric::SuppressedContexts => GungraunMeasure::MemcheckSuppressedContexts(json),
            ErrorMetric::SuppressedErrors => GungraunMeasure::MemcheckSuppressedErrors(json),
        })
        .collect()
}

fn parse_callgrind_metrics_summary(
    metrics_summary: MetricsSummary<EventKind>,
) -> Vec<GungraunMeasure> {
    metrics_summary
        .0
        .into_iter()
        .filter_map(|(event_kind, diff)| diff_to_json(&diff).map(|json| (event_kind, json)))
        .map(|(event_kind, json)| match event_kind {
            EventKind::AcCost1 => GungraunMeasure::L1BadTemporalLocality(json),
            EventKind::AcCost2 => GungraunMeasure::LLBadTemporalLocality(json),
            EventKind::Bc => GungraunMeasure::ExecutedConditionalBranches(json),
            EventKind::Bcm => GungraunMeasure::MispredictedConditionalBranches(json),
            EventKind::Bi => GungraunMeasure::ExecutedIndirectBranches(json),
            EventKind::Bim => GungraunMeasure::MispredictedIndirectBranches(json),
            EventKind::D1MissRate => GungraunMeasure::L1DataCacheMissRate(json),
            EventKind::D1mr => GungraunMeasure::L1DataCacheReadMisses(json),
            EventKind::D1mw => GungraunMeasure::L1DataCacheWriteMisses(json),
            EventKind::DLdmr => GungraunMeasure::DirtyMissDataRead(json),
            EventKind::DLdmw => GungraunMeasure::DirtyMissDataWrite(json),
            EventKind::DLmr => GungraunMeasure::LLDataCacheReadMisses(json),
            EventKind::DLmw => GungraunMeasure::LLDataCacheWriteMisses(json),
            EventKind::Dr => GungraunMeasure::DataCacheReads(json),
            EventKind::Dw => GungraunMeasure::DataCacheWrites(json),
            EventKind::EstimatedCycles => GungraunMeasure::EstimatedCycles(json),
            EventKind::Ge => GungraunMeasure::GlobalBusEvents(json),
            EventKind::I1MissRate => GungraunMeasure::L1InstrCacheMissRate(json),
            EventKind::I1mr => GungraunMeasure::L1InstrCacheReadMisses(json),
            EventKind::ILdmr => GungraunMeasure::DirtyMissInstructionRead(json),
            EventKind::ILmr => GungraunMeasure::LLInstrCacheReadMisses(json),
            EventKind::Ir => GungraunMeasure::Instructions(json),
            EventKind::L1HitRate => GungraunMeasure::L1HitRate(json),
            EventKind::L1hits => GungraunMeasure::L1Hits(json),
            EventKind::LLHitRate => GungraunMeasure::LLHitRate(json),
            EventKind::LLMissRate => GungraunMeasure::LLCacheMissRate(json),
            EventKind::LLdMissRate => GungraunMeasure::LLDataCacheMissRate(json),
            EventKind::LLhits => GungraunMeasure::LLHits(json),
            EventKind::LLiMissRate => GungraunMeasure::LLInstrCacheMissRate(json),
            EventKind::RamHitRate => GungraunMeasure::RamHitRate(json),
            EventKind::RamHits => GungraunMeasure::RamHits(json),
            EventKind::SpLoss1 => GungraunMeasure::L1BadSpatialLocality(json),
            EventKind::SpLoss2 => GungraunMeasure::LLBadSpatialLocality(json),
            EventKind::SysCount => GungraunMeasure::NumberSystemCalls(json),
            EventKind::SysCpuTime => GungraunMeasure::CpuTimeSystemCalls(json),
            EventKind::SysTime => GungraunMeasure::TimeSystemCalls(json),
            EventKind::TotalRW => GungraunMeasure::TotalReadWrite(json),
        })
        .collect()
}

fn parse_cachegrind_metrics_summary(
    metrics_summary: MetricsSummary<CachegrindMetric>,
) -> Vec<GungraunMeasure> {
    metrics_summary
        .0
        .into_iter()
        .filter_map(|(metric, diff)| diff_to_json(&diff).map(|json| (metric, json)))
        .map(|(metric, json)| match metric {
            CachegrindMetric::Bc => GungraunMeasure::ExecutedConditionalBranches(json),
            CachegrindMetric::Bcm => GungraunMeasure::MispredictedConditionalBranches(json),
            CachegrindMetric::Bi => GungraunMeasure::ExecutedIndirectBranches(json),
            CachegrindMetric::Bim => GungraunMeasure::MispredictedIndirectBranches(json),
            CachegrindMetric::D1MissRate => GungraunMeasure::L1DataCacheMissRate(json),
            CachegrindMetric::D1mr => GungraunMeasure::L1DataCacheReadMisses(json),
            CachegrindMetric::D1mw => GungraunMeasure::L1DataCacheWriteMisses(json),
            CachegrindMetric::DLmr => GungraunMeasure::LLDataCacheReadMisses(json),
            CachegrindMetric::DLmw => GungraunMeasure::LLDataCacheWriteMisses(json),
            CachegrindMetric::Dr => GungraunMeasure::DataCacheReads(json),
            CachegrindMetric::Dw => GungraunMeasure::DataCacheWrites(json),
            CachegrindMetric::EstimatedCycles => GungraunMeasure::EstimatedCycles(json),
            CachegrindMetric::I1MissRate => GungraunMeasure::L1InstrCacheMissRate(json),
            CachegrindMetric::I1mr => GungraunMeasure::L1InstrCacheReadMisses(json),
            CachegrindMetric::ILmr => GungraunMeasure::LLInstrCacheReadMisses(json),
            CachegrindMetric::Ir => GungraunMeasure::Instructions(json),
            CachegrindMetric::L1HitRate => GungraunMeasure::L1HitRate(json),
            CachegrindMetric::L1hits => GungraunMeasure::L1Hits(json),
            CachegrindMetric::LLHitRate => GungraunMeasure::LLHitRate(json),
            CachegrindMetric::LLMissRate => GungraunMeasure::LLCacheMissRate(json),
            CachegrindMetric::LLdMissRate => GungraunMeasure::LLDataCacheMissRate(json),
            CachegrindMetric::LLhits => GungraunMeasure::LLHits(json),
            CachegrindMetric::LLiMissRate => GungraunMeasure::LLInstrCacheMissRate(json),
            CachegrindMetric::RamHitRate => GungraunMeasure::RamHitRate(json),
            CachegrindMetric::RamHits => GungraunMeasure::RamHits(json),
            CachegrindMetric::TotalRW => GungraunMeasure::TotalReadWrite(json),
        })
        .collect()
}

fn parse_dhat_metrics_summary(metrics_summary: MetricsSummary<DhatMetric>) -> Vec<GungraunMeasure> {
    metrics_summary
        .0
        .into_iter()
        .filter_map(|(metric, diff)| diff_to_json(&diff).map(|json| (metric, json)))
        .map(|(metric, json)| match metric {
            DhatMetric::AtTEndBlocks => GungraunMeasure::AtTEndBlocks(json),
            DhatMetric::AtTEndBytes => GungraunMeasure::AtTEndBytes(json),
            DhatMetric::AtTGmaxBlocks => GungraunMeasure::AtTGmaxBlocks(json),
            DhatMetric::AtTGmaxBytes => GungraunMeasure::AtTGmaxBytes(json),
            DhatMetric::MaximumBlocks => GungraunMeasure::MaximumBlocks(json),
            DhatMetric::MaximumBytes => GungraunMeasure::MaximumBytes(json),
            DhatMetric::ReadsBytes => GungraunMeasure::ReadsBytes(json),
            DhatMetric::TotalBlocks => GungraunMeasure::TotalBlocks(json),
            DhatMetric::TotalBytes => GungraunMeasure::TotalBytes(json),
            DhatMetric::TotalEvents => GungraunMeasure::TotalEvents(json),
            DhatMetric::TotalLifetimes => GungraunMeasure::TotalLifetimes(json),
            DhatMetric::TotalUnits => GungraunMeasure::TotalUnits(json),
            DhatMetric::WritesBytes => GungraunMeasure::WritesBytes(json),
        })
        .collect()
}

fn diff_to_json(diff: &MetricsDiff) -> Option<JsonNewMetric> {
    let metric = match diff.metrics {
        EitherOrBoth::Left(new) | EitherOrBoth::Both(new, _) => new,
        EitherOrBoth::Right(_) => return None,
    };

    #[expect(
        clippy::cast_precision_loss,
        reason = "bencher metrics have f64 precision"
    )]
    let value = match metric {
        Metric::Int(int) => int as f64,
        Metric::Float(float) => float,
    };

    Some(JsonNewMetric {
        value: value.into(),
        lower_value: None,
        upper_value: None,
    })
}

#[cfg(test)]
pub(crate) mod test_rust_gungraun_json {
    use crate::{
        AdapterResults, Settings,
        adapters::test_util::{convert_file_path, opt_convert_file_path},
    };
    use bencher_json::project::measure::built_in::{BuiltInMeasure as _, gungraun::*};
    use ordered_float::OrderedFloat;
    use pretty_assertions::assert_eq;

    use super::AdapterRustGungraunJson;
    use std::collections::HashMap;

    #[test]
    fn not_v6() {
        let results = opt_convert_file_path::<AdapterRustGungraunJson>(
            "./tool_output/rust/gungraun/json_not_v6.txt",
            Settings::default(),
        );

        assert!(results.is_none());
    }

    #[test]
    fn no_id_no_details() {
        let results = convert_file_path::<AdapterRustGungraunJson>(
            "./tool_output/rust/gungraun/json_no_id_no_details.txt",
        );

        let expected = HashMap::from([(D1MissRate::SLUG_STR, 0.1), (D1mr::SLUG_STR, 6.0)]);

        assert_eq!(results.inner.len(), 1);
        compare_benchmark(
            &expected,
            &results,
            "play_game::bench_play_game_group::bench_play_game_100",
        );
    }

    #[test]
    fn one_callgrind_diff() {
        let results = convert_file_path::<AdapterRustGungraunJson>(
            "./tool_output/rust/gungraun/json_one_callgrind_diff.txt",
        );

        validate_adapter_rust_gungraun_json(&results);
    }

    #[test]
    fn one_callgrind_no_diff_new() {
        let results = convert_file_path::<AdapterRustGungraunJson>(
            "./tool_output/rust/gungraun/json_one_callgrind_no_diff_new.txt",
        );

        validate_adapter_rust_gungraun_json(&results);
    }

    #[test]
    fn one_callgrind_no_diff_old() {
        let expected = HashMap::new();

        let results = convert_file_path::<AdapterRustGungraunJson>(
            "./tool_output/rust/gungraun/json_one_callgrind_no_diff_old.txt",
        );

        assert_eq!(results.inner.len(), 1);
        compare_benchmark(
            &expected,
            &results,
            "play_game::bench_play_game_group::bench_play_game_100::some_id",
        );
    }

    #[test]
    fn one_callgrind_all_metrics() {
        let expected = HashMap::from([
            (AcCost1::SLUG_STR, 0.0),
            (AcCost2::SLUG_STR, 100.0),
            (Bc::SLUG_STR, 200.0),
            (Bcm::SLUG_STR, 300.0),
            (Bi::SLUG_STR, 400.0),
            (Bim::SLUG_STR, 500.0),
            (D1MissRate::SLUG_STR, 0.0),
            (D1mr::SLUG_STR, 600.0),
            (D1mw::SLUG_STR, 700.0),
            (DLdmr::SLUG_STR, 800.0),
            (DLdmw::SLUG_STR, 900.0),
            (DLmr::SLUG_STR, 1000.0),
            (DLmw::SLUG_STR, 1100.0),
            (Dr::SLUG_STR, 1200.0),
            (Dw::SLUG_STR, 1300.0),
            (EstimatedCycles::SLUG_STR, 1400.0),
            (GlobalBusEvents::SLUG_STR, 1500.0),
            (I1MissRate::SLUG_STR, 0.1),
            (I1mr::SLUG_STR, 1600.0),
            (ILdmr::SLUG_STR, 1700.0),
            (ILmr::SLUG_STR, 1800.0),
            (Instructions::SLUG_STR, 1900.0),
            (L1HitRate::SLUG_STR, 0.2),
            (L1Hits::SLUG_STR, 2000.0),
            (LLHitRate::SLUG_STR, 0.3),
            (LLMissRate::SLUG_STR, 0.4),
            (LLdMissRate::SLUG_STR, 0.5),
            (LLHits::SLUG_STR, 2100.0),
            (LLiMissRate::SLUG_STR, 0.6),
            (RamHitRate::SLUG_STR, 0.7),
            (RamHits::SLUG_STR, 2200.0),
            (SpLoss1::SLUG_STR, 2300.0),
            (SpLoss2::SLUG_STR, 2400.0),
            (SysCount::SLUG_STR, 2500.0),
            (SysCpuTime::SLUG_STR, 2600.0),
            (SysTime::SLUG_STR, 2700.0),
            (TotalReadWrite::SLUG_STR, 2800.0),
        ]);

        let results = convert_file_path::<AdapterRustGungraunJson>(
            "./tool_output/rust/gungraun/json_one_callgrind_all_metrics.txt",
        );

        assert_eq!(results.inner.len(), 1);
        compare_benchmark(
            &expected,
            &results,
            "play_game::bench_play_game_group::bench_play_game_100::some_id",
        );
    }

    #[test]
    fn one_cachegrind_all_metrics() {
        let expected = HashMap::from([
            (Bc::SLUG_STR, 0.0),
            (Bcm::SLUG_STR, 100.0),
            (Bi::SLUG_STR, 200.0),
            (Bim::SLUG_STR, 300.0),
            (D1MissRate::SLUG_STR, 0.0),
            (D1mr::SLUG_STR, 400.0),
            (D1mw::SLUG_STR, 500.0),
            (DLmr::SLUG_STR, 600.0),
            (DLmw::SLUG_STR, 700.0),
            (Dr::SLUG_STR, 800.0),
            (Dw::SLUG_STR, 900.0),
            (EstimatedCycles::SLUG_STR, 1000.0),
            (I1MissRate::SLUG_STR, 0.1),
            (I1mr::SLUG_STR, 1100.0),
            (ILmr::SLUG_STR, 1200.0),
            (Instructions::SLUG_STR, 1300.0),
            (L1HitRate::SLUG_STR, 0.2),
            (L1Hits::SLUG_STR, 1400.0),
            (LLHitRate::SLUG_STR, 0.3),
            (LLMissRate::SLUG_STR, 0.4),
            (LLdMissRate::SLUG_STR, 0.5),
            (LLHits::SLUG_STR, 1500.0),
            (LLiMissRate::SLUG_STR, 0.6),
            (RamHitRate::SLUG_STR, 0.7),
            (RamHits::SLUG_STR, 1600.0),
            (TotalReadWrite::SLUG_STR, 1700.0),
        ]);

        let results = convert_file_path::<AdapterRustGungraunJson>(
            "./tool_output/rust/gungraun/json_one_cachegrind_all_metrics.txt",
        );

        assert_eq!(results.inner.len(), 1);
        compare_benchmark(
            &expected,
            &results,
            "play_game::bench_play_game_group::bench_play_game_100::some_id",
        );
    }

    #[test]
    fn one_dhat_all_metrics() {
        let expected = HashMap::from([
            (AtTEndBlocks::SLUG_STR, 0.0),
            (AtTEndBytes::SLUG_STR, 1.0),
            (AtTGmaxBlocks::SLUG_STR, 2.0),
            (AtTGmaxBytes::SLUG_STR, 3.0),
            (MaximumBlocks::SLUG_STR, 4.0),
            (MaximumBytes::SLUG_STR, 5.0),
            (ReadsBytes::SLUG_STR, 6.0),
            (TotalBlocks::SLUG_STR, 7.0),
            (TotalBytes::SLUG_STR, 8.0),
            (TotalEvents::SLUG_STR, 9.0),
            (TotalLifetimes::SLUG_STR, 10.0),
            (TotalUnits::SLUG_STR, 11.0),
            (WritesBytes::SLUG_STR, 12.0),
        ]);

        let results = convert_file_path::<AdapterRustGungraunJson>(
            "./tool_output/rust/gungraun/json_one_dhat_all_metrics.txt",
        );

        assert_eq!(results.inner.len(), 1);
        compare_benchmark(
            &expected,
            &results,
            "play_game::bench_play_game_group::bench_play_game_100::some_id",
        );
    }

    #[test]
    fn one_memcheck_all_metrics() {
        let expected = HashMap::from([
            (MemcheckContexts::SLUG_STR, 0.0),
            (MemcheckErrors::SLUG_STR, 1.0),
            (MemcheckSuppressedContexts::SLUG_STR, 2.0),
            (MemcheckSuppressedErrors::SLUG_STR, 3.0),
        ]);

        let results = convert_file_path::<AdapterRustGungraunJson>(
            "./tool_output/rust/gungraun/json_one_memcheck_all_metrics.txt",
        );

        assert_eq!(results.inner.len(), 1);
        compare_benchmark(
            &expected,
            &results,
            "play_game::bench_play_game_group::bench_play_game_100::some_id",
        );
    }

    #[test]
    fn one_helgrind_all_metrics() {
        let expected = HashMap::from([
            (HelgrindContexts::SLUG_STR, 0.0),
            (HelgrindErrors::SLUG_STR, 1.0),
            (HelgrindSuppressedContexts::SLUG_STR, 2.0),
            (HelgrindSuppressedErrors::SLUG_STR, 3.0),
        ]);

        let results = convert_file_path::<AdapterRustGungraunJson>(
            "./tool_output/rust/gungraun/json_one_helgrind_all_metrics.txt",
        );

        assert_eq!(results.inner.len(), 1);
        compare_benchmark(
            &expected,
            &results,
            "play_game::bench_play_game_group::bench_play_game_100::some_id",
        );
    }

    #[test]
    fn one_drd_all_metrics() {
        let expected = HashMap::from([
            (DrdContexts::SLUG_STR, 0.0),
            (DrdErrors::SLUG_STR, 1.0),
            (DrdSuppressedContexts::SLUG_STR, 2.0),
            (DrdSuppressedErrors::SLUG_STR, 3.0),
        ]);

        let results = convert_file_path::<AdapterRustGungraunJson>(
            "./tool_output/rust/gungraun/json_one_drd_all_metrics.txt",
        );

        assert_eq!(results.inner.len(), 1);
        compare_benchmark(
            &expected,
            &results,
            "play_game::bench_play_game_group::bench_play_game_100::some_id",
        );
    }

    #[test]
    fn one_tool_without_metrics() {
        let expected = HashMap::new();

        let results = convert_file_path::<AdapterRustGungraunJson>(
            "./tool_output/rust/gungraun/json_one_tool_without_metrics.txt",
        );

        assert_eq!(results.inner.len(), 1);
        compare_benchmark(
            &expected,
            &results,
            "play_game::bench_play_game_group::bench_play_game_100::some_id",
        );
    }

    #[test]
    fn multiple_tools() {
        let results = convert_file_path::<AdapterRustGungraunJson>(
            "./tool_output/rust/gungraun/json_multiple_tools.txt",
        );

        let expected = HashMap::from([
            // Callgrid
            (D1MissRate::SLUG_STR, 0.1),
            (D1mr::SLUG_STR, 6.0),
            // DHAT
            (AtTEndBlocks::SLUG_STR, 1.0),
        ]);

        assert_eq!(results.inner.len(), 1);
        compare_benchmark(
            &expected,
            &results,
            "play_game::bench_play_game_group::bench_play_game_100::some_id",
        );
    }

    #[test]
    fn multiple_benchmarks() {
        let results = convert_file_path::<AdapterRustGungraunJson>(
            "./tool_output/rust/gungraun/json_multiple_benchmarks.txt",
        );

        assert_eq!(results.inner.len(), 2);

        {
            let expected = HashMap::from([
                // Callgrid
                (D1MissRate::SLUG_STR, 0.1),
                (D1mr::SLUG_STR, 6.0),
                // DHAT
                (AtTEndBlocks::SLUG_STR, 1.0),
            ]);

            compare_benchmark(
                &expected,
                &results,
                "play_game::bench_play_game_group::bench_play_game_100::first_id",
            );
        }
        {
            let expected = HashMap::from([
                (MemcheckContexts::SLUG_STR, 0.0),
                (MemcheckErrors::SLUG_STR, 1.0),
                (MemcheckSuppressedContexts::SLUG_STR, 2.0),
                (MemcheckSuppressedErrors::SLUG_STR, 3.0),
            ]);

            compare_benchmark(
                &expected,
                &results,
                "play_game::bench_play_game_group::bench_play_game_100::second_id",
            );
        }
    }

    #[test]
    fn pretty_one_callgrind() {
        let results = convert_file_path::<AdapterRustGungraunJson>(
            "./tool_output/rust/gungraun/json_pretty_one_callgrind.txt",
        );

        validate_adapter_rust_gungraun_json(&results);
    }

    #[test]
    fn pretty_two_callgrind() {
        let results = convert_file_path::<AdapterRustGungraunJson>(
            "./tool_output/rust/gungraun/json_pretty_two_callgrind.txt",
        );

        assert_eq!(results.inner.len(), 2);

        {
            let expected = HashMap::from([(D1MissRate::SLUG_STR, 0.1), (D1mr::SLUG_STR, 6.0)]);

            compare_benchmark(
                &expected,
                &results,
                "play_game::bench_play_game_group::bench_play_game_100::first",
            );
        }

        {
            let expected = HashMap::from([(D1MissRate::SLUG_STR, 0.2), (D1mr::SLUG_STR, 7.0)]);

            compare_benchmark(
                &expected,
                &results,
                "play_game::bench_play_game_group::bench_play_game_100::second",
            );
        }
    }

    #[test]
    fn pretty_mixed_ndjson_two_callgrind() {
        let results = convert_file_path::<AdapterRustGungraunJson>(
            "./tool_output/rust/gungraun/json_pretty_mixed_ndjson_two_callgrind.txt",
        );

        assert_eq!(results.inner.len(), 2);

        {
            let expected = HashMap::from([(D1MissRate::SLUG_STR, 0.1), (D1mr::SLUG_STR, 6.0)]);

            compare_benchmark(
                &expected,
                &results,
                "play_game::bench_play_game_group::bench_play_game_100::first",
            );
        }

        {
            let expected = HashMap::from([(D1MissRate::SLUG_STR, 0.2), (D1mr::SLUG_STR, 7.0)]);

            compare_benchmark(
                &expected,
                &results,
                "play_game::bench_play_game_group::bench_play_game_100::second",
            );
        }
    }

    #[test]
    fn pretty_mixed_with_foreign_ndjson() {
        let results = convert_file_path::<AdapterRustGungraunJson>(
            "./tool_output/rust/gungraun/json_pretty_mixed_foreign_ndjson.txt",
        );

        validate_adapter_rust_gungraun_json(&results);
    }

    #[test]
    fn pretty_mixed_with_foreign_pretty_and_ndjson() {
        let results = convert_file_path::<AdapterRustGungraunJson>(
            "./tool_output/rust/gungraun/json_pretty_mixed_foreign_pretty_and_ndjson.txt",
        );

        validate_adapter_rust_gungraun_json(&results);
    }

    #[test]
    fn pretty_one_callgrind_indented() {
        let results = convert_file_path::<AdapterRustGungraunJson>(
            "./tool_output/rust/gungraun/json_pretty_one_callgrind_indented.txt",
        );

        validate_adapter_rust_gungraun_json(&results);
    }

    pub fn validate_adapter_rust_gungraun_json(results: &AdapterResults) {
        let expected = HashMap::from([(D1MissRate::SLUG_STR, 0.1), (D1mr::SLUG_STR, 6.0)]);

        assert_eq!(results.inner.len(), 1);
        compare_benchmark(
            &expected,
            results,
            "play_game::bench_play_game_group::bench_play_game_100::some_id",
        );
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
