use bencher_json::{BenchmarkName, JsonNewMetric, project::report::JsonAverage};

use gungraun_summary::{
    either_or_both::EitherOrBoth,
    util::SummaryByVersion,
    v6::{
        CachegrindMetric, DhatMetric, ErrorMetric, EventKind, Metric, MetricsDiff, MetricsSummary,
        ToolMetricSummary, ValgrindTool,
    },
};

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

fn parse_multiple(input: &str) -> Option<Vec<(BenchmarkName, Vec<GungraunMeasure>)>> {
    let parsed = input.lines().filter_map(parse_line).collect::<Vec<_>>();

    (!parsed.is_empty()).then_some(parsed)
}

fn parse_line(input: &str) -> Option<(BenchmarkName, Vec<GungraunMeasure>)> {
    // Using the version aware gungraun_summary parsing method to simplify adapting gungraun summary
    // version updates. At the moment there's just v6
    let summary = match gungraun_summary::util::parse_slice(input.as_bytes()) {
        Ok(summary_by_version) => match summary_by_version {
            SummaryByVersion::V6(benchmark_summary) => benchmark_summary,
            _ => return None,
        },
        Err(_) => return None,
    };

    let name: BenchmarkName = if let Some(id) = &summary.id {
        format!("{}::{id}", summary.module_path)
    } else {
        summary.module_path
    }
    .parse()
    .ok()?;

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
