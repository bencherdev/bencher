use std::{collections::HashMap, hash::Hash};

use bencher_json::{project::report::JsonAverage, BenchmarkName, JsonNewMetric};
use serde::Deserialize;

use crate::{
    results::adapter_results::{AdapterResults, GungraunMeasure},
    Adaptable, Settings,
};

pub struct AdapterRustGungraunJson;

impl Adaptable for AdapterRustGungraunJson {
    fn parse(input: &str, settings: Settings) -> Option<AdapterResults> {
        match settings.average {
            None => {},
            Some(JsonAverage::Mean | JsonAverage::Median) => {
                return None; // 'gungraun' results are for a single run only.
            },
        }

        let results: Vec<BenchmarkResult> = match serde_json::from_str(input) {
            Ok(results) => results,
            Err(_) => {
                debug_assert!(false, "Error deserializing input as gungraun JSON.");
                return None;
            },
        };

        AdapterResults::new_gungraun(
            results
                .into_iter()
                .flat_map(Into::<Option<(BenchmarkName, Vec<GungraunMeasure>)>>::into)
                .collect(),
        )
    }
}

#[derive(Deserialize)]
struct BenchmarkResult {
    id: String,
    module_path: String,
    profiles: Vec<Profile>,
}

impl From<BenchmarkResult> for Option<(BenchmarkName, Vec<GungraunMeasure>)> {
    fn from(value: BenchmarkResult) -> Self {
        Some((
            format!("{}::{}", value.module_path, value.id)
                .parse()
                .ok()?,
            value
                .profiles
                .into_iter()
                .flat_map(Into::<Vec<GungraunMeasure>>::into)
                .collect(),
        ))
    }
}

#[derive(Deserialize)]
struct Profile {
    summaries: ProfileData,
    tool: ValgrindTool,
}

impl From<Profile> for Vec<GungraunMeasure> {
    fn from(value: Profile) -> Self {
        match value.summaries.total.summary {
            ToolMetricSummary::None => Vec::new(),
            ToolMetricSummary::ErrorTool(metrics) => {
                Vec::<(ErrorMetric, JsonNewMetric)>::from(metrics)
                    .into_iter()
                    .map(|(key, json_metric)| match value.tool {
                        ValgrindTool::DRD => match key {
                            ErrorMetric::Errors => GungraunMeasure::DrdErrors(json_metric),
                            ErrorMetric::Contexts => GungraunMeasure::DrdContexts(json_metric),
                            ErrorMetric::SuppressedErrors => {
                                GungraunMeasure::DrdSuppressedErrors(json_metric)
                            },
                            ErrorMetric::SuppressedContexts => {
                                GungraunMeasure::DrdSuppressedContexts(json_metric)
                            },
                        },
                        ValgrindTool::Helgrind => match key {
                            ErrorMetric::Errors => GungraunMeasure::HelgrindErrors(json_metric),
                            ErrorMetric::Contexts => GungraunMeasure::HelgrindContexts(json_metric),
                            ErrorMetric::SuppressedErrors => {
                                GungraunMeasure::HelgrindSuppressedErrors(json_metric)
                            },
                            ErrorMetric::SuppressedContexts => {
                                GungraunMeasure::HelgrindSuppressedContexts(json_metric)
                            },
                        },
                        ValgrindTool::Memcheck => match key {
                            ErrorMetric::Errors => GungraunMeasure::MemcheckErrors(json_metric),
                            ErrorMetric::Contexts => GungraunMeasure::MemcheckContexts(json_metric),
                            ErrorMetric::SuppressedErrors => {
                                GungraunMeasure::MemcheckSuppressedErrors(json_metric)
                            },
                            ErrorMetric::SuppressedContexts => {
                                GungraunMeasure::MemcheckSuppressedContexts(json_metric)
                            },
                        },
                        _ => GungraunMeasure::Unknown,
                    })
                    .collect()
            },
            ToolMetricSummary::Dhat(metrics) => metrics.into(),
            ToolMetricSummary::Callgrind(metrics) => metrics.into(),
            ToolMetricSummary::Cachegrind(metrics) => metrics.into(),
        }
    }
}

#[derive(Deserialize)]
struct ProfileData {
    total: ProfileTotal,
}

#[derive(Deserialize)]
struct ProfileTotal {
    summary: ToolMetricSummary,
}

#[derive(Deserialize)]
enum ToolMetricSummary {
    None,
    ErrorTool(MetricsSummary<ErrorMetric>),
    Dhat(MetricsSummary<DhatMetric>),
    Callgrind(MetricsSummary<EventKind>),
    Cachegrind(MetricsSummary<CachegrindMetric>),
}

#[derive(Deserialize)]
pub enum ValgrindTool {
    /// [Callgrind: a call-graph generating cache and branch prediction profiler](https://valgrind.org/docs/manual/cl-manual.html)
    Callgrind,
    /// [Cachegrind: a high-precision tracing profiler](https://valgrind.org/docs/manual/cg-manual.html)
    Cachegrind,
    /// [DHAT: a dynamic heap analysis tool](https://valgrind.org/docs/manual/dh-manual.html)
    DHAT,
    /// [Memcheck: a memory error detector](https://valgrind.org/docs/manual/mc-manual.html)
    Memcheck,
    /// [Helgrind: a thread error detector](https://valgrind.org/docs/manual/hg-manual.html)
    Helgrind,
    /// [DRD: a thread error detector](https://valgrind.org/docs/manual/drd-manual.html)
    DRD,
    /// [Massif: a heap profiler](https://valgrind.org/docs/manual/ms-manual.html)
    Massif,
    /// [BBV: an experimental basic block vector generation tool](https://valgrind.org/docs/manual/bbv-manual.html)
    BBV,
}

#[derive(Deserialize)]
struct MetricsSummary<K: Hash + Eq>(HashMap<K, MetricsDiff>);

impl From<MetricsSummary<ErrorMetric>> for Vec<(ErrorMetric, JsonNewMetric)> {
    fn from(value: MetricsSummary<ErrorMetric>) -> Self {
        value
            .0
            .into_iter()
            .map(|(key, value)| {
                let json_metric: JsonNewMetric = value.into();
                (key, json_metric)
            })
            .collect()
    }
}

impl From<MetricsSummary<DhatMetric>> for Vec<GungraunMeasure> {
    fn from(value: MetricsSummary<DhatMetric>) -> Self {
        value
            .0
            .into_iter()
            .map(|(key, value)| {
                let json_metric: JsonNewMetric = value.into();
                match key {
                    DhatMetric::TotalUnits => GungraunMeasure::TotalUnits(json_metric),
                    DhatMetric::TotalEvents => GungraunMeasure::TotalEvents(json_metric),
                    DhatMetric::TotalBytes => GungraunMeasure::TotalBytes(json_metric),
                    DhatMetric::TotalBlocks => GungraunMeasure::TotalBlocks(json_metric),
                    DhatMetric::AtTGmaxBytes => GungraunMeasure::AtTGmaxBytes(json_metric),
                    DhatMetric::AtTGmaxBlocks => GungraunMeasure::AtTGmaxBlocks(json_metric),
                    DhatMetric::AtTEndBytes => GungraunMeasure::AtTEndBytes(json_metric),
                    DhatMetric::AtTEndBlocks => GungraunMeasure::AtTEndBlocks(json_metric),
                    DhatMetric::ReadsBytes => GungraunMeasure::ReadsBytes(json_metric),
                    DhatMetric::WritesBytes => GungraunMeasure::WritesBytes(json_metric),
                    DhatMetric::TotalLifetimes => GungraunMeasure::TotalLifetimes(json_metric),
                    DhatMetric::MaximumBytes => GungraunMeasure::MaximumBytes(json_metric),
                    DhatMetric::MaximumBlocks => GungraunMeasure::MaximumBlocks(json_metric),
                }
            })
            .collect()
    }
}

impl From<MetricsSummary<EventKind>> for Vec<GungraunMeasure> {
    fn from(value: MetricsSummary<EventKind>) -> Self {
        value
            .0
            .into_iter()
            .map(|(key, value)| {
                let json_metric: JsonNewMetric = value.into();
                match key {
                    EventKind::Ir => GungraunMeasure::Instructions(json_metric),
                    EventKind::Dr => GungraunMeasure::DataCacheReads(json_metric),
                    EventKind::Dw => GungraunMeasure::DataCacheWrites(json_metric),
                    EventKind::I1mr => GungraunMeasure::L1InstrCacheReadMisses(json_metric),
                    EventKind::D1mr => GungraunMeasure::L1DataCacheReadMisses(json_metric),
                    EventKind::D1mw => GungraunMeasure::L1DataCacheWriteMisses(json_metric),
                    EventKind::ILmr => GungraunMeasure::LLInstrCacheReadMisses(json_metric),
                    EventKind::DLmr => GungraunMeasure::LLDataCacheReadMisses(json_metric),
                    EventKind::DLmw => GungraunMeasure::LLDataCacheWriteMisses(json_metric),
                    EventKind::I1MissRate => GungraunMeasure::L1InstrCacheMissRate(json_metric),
                    EventKind::LLiMissRate => GungraunMeasure::LLInstrCacheMissRate(json_metric),
                    EventKind::D1MissRate => GungraunMeasure::L1DataCacheMissRate(json_metric),
                    EventKind::LLdMissRate => GungraunMeasure::LLDataCacheMissRate(json_metric),
                    EventKind::LLMissRate => GungraunMeasure::LLCacheMissRate(json_metric),
                    EventKind::L1hits => GungraunMeasure::L1Hits(json_metric),
                    EventKind::LLhits => GungraunMeasure::LLHits(json_metric),
                    EventKind::RamHits => GungraunMeasure::RamHits(json_metric),
                    EventKind::L1HitRate => GungraunMeasure::L1HitRate(json_metric),
                    EventKind::LLHitRate => GungraunMeasure::LLHitRate(json_metric),
                    EventKind::RamHitRate => GungraunMeasure::RamHitRate(json_metric),
                    EventKind::TotalRW => GungraunMeasure::TotalReadWrite(json_metric),
                    EventKind::EstimatedCycles => GungraunMeasure::EstimatedCycles(json_metric),
                    EventKind::SysCount => GungraunMeasure::NumberSystemCalls(json_metric),
                    EventKind::SysTime => GungraunMeasure::TimeSystemCalls(json_metric),
                    EventKind::SysCpuTime => GungraunMeasure::CpuTimeSystemCalls(json_metric),
                    EventKind::Ge => GungraunMeasure::GlobalBusEvents(json_metric),
                    EventKind::Bc => GungraunMeasure::ExecutedConditionalBranches(json_metric),
                    EventKind::Bcm => GungraunMeasure::MispredictedConditionalBranches(json_metric),
                    EventKind::Bi => GungraunMeasure::ExecutedIndirectBranches(json_metric),
                    EventKind::Bim => GungraunMeasure::MispredictedIndirectBranches(json_metric),
                    EventKind::ILdmr => GungraunMeasure::DirtyMissInstructionRead(json_metric),
                    EventKind::DLdmr => GungraunMeasure::DirtyMissDataRead(json_metric),
                    EventKind::DLdmw => GungraunMeasure::DirtyMissDataWrite(json_metric),
                    EventKind::AcCost1 => GungraunMeasure::L1BadTemporalLocality(json_metric),
                    EventKind::AcCost2 => GungraunMeasure::LLBadTemporalLocality(json_metric),
                    EventKind::SpLoss1 => GungraunMeasure::L1BadSpatialLocality(json_metric),
                    EventKind::SpLoss2 => GungraunMeasure::LLBadSpatialLocality(json_metric),
                }
            })
            .collect()
    }
}

impl From<MetricsSummary<CachegrindMetric>> for Vec<GungraunMeasure> {
    fn from(value: MetricsSummary<CachegrindMetric>) -> Self {
        value
            .0
            .into_iter()
            .map(|(key, value)| {
                let json_metric: JsonNewMetric = value.into();
                match key {
                    CachegrindMetric::Ir => GungraunMeasure::Instructions(json_metric),
                    CachegrindMetric::Dr => GungraunMeasure::DataCacheReads(json_metric),
                    CachegrindMetric::Dw => GungraunMeasure::DataCacheWrites(json_metric),
                    CachegrindMetric::I1mr => GungraunMeasure::L1InstrCacheReadMisses(json_metric),
                    CachegrindMetric::D1mr => GungraunMeasure::L1DataCacheReadMisses(json_metric),
                    CachegrindMetric::D1mw => GungraunMeasure::L1DataCacheWriteMisses(json_metric),
                    CachegrindMetric::ILmr => GungraunMeasure::LLInstrCacheReadMisses(json_metric),
                    CachegrindMetric::DLmr => GungraunMeasure::LLDataCacheReadMisses(json_metric),
                    CachegrindMetric::DLmw => GungraunMeasure::LLDataCacheWriteMisses(json_metric),
                    CachegrindMetric::I1MissRate => {
                        GungraunMeasure::L1InstrCacheMissRate(json_metric)
                    },
                    CachegrindMetric::LLiMissRate => {
                        GungraunMeasure::LLInstrCacheMissRate(json_metric)
                    },
                    CachegrindMetric::D1MissRate => {
                        GungraunMeasure::L1DataCacheMissRate(json_metric)
                    },
                    CachegrindMetric::LLdMissRate => {
                        GungraunMeasure::LLDataCacheMissRate(json_metric)
                    },
                    CachegrindMetric::LLMissRate => GungraunMeasure::LLCacheMissRate(json_metric),
                    CachegrindMetric::L1hits => GungraunMeasure::L1Hits(json_metric),
                    CachegrindMetric::LLhits => GungraunMeasure::LLHits(json_metric),
                    CachegrindMetric::RamHits => GungraunMeasure::RamHits(json_metric),
                    CachegrindMetric::L1HitRate => GungraunMeasure::L1HitRate(json_metric),
                    CachegrindMetric::LLHitRate => GungraunMeasure::LLHitRate(json_metric),
                    CachegrindMetric::RamHitRate => GungraunMeasure::RamHitRate(json_metric),
                    CachegrindMetric::TotalRW => GungraunMeasure::TotalReadWrite(json_metric),
                    CachegrindMetric::EstimatedCycles => {
                        GungraunMeasure::EstimatedCycles(json_metric)
                    },
                    CachegrindMetric::Bc => {
                        GungraunMeasure::ExecutedConditionalBranches(json_metric)
                    },
                    CachegrindMetric::Bcm => {
                        GungraunMeasure::MispredictedConditionalBranches(json_metric)
                    },
                    CachegrindMetric::Bi => GungraunMeasure::ExecutedIndirectBranches(json_metric),
                    CachegrindMetric::Bim => {
                        GungraunMeasure::MispredictedIndirectBranches(json_metric)
                    },
                }
            })
            .collect()
    }
}

#[derive(Deserialize)]
struct MetricsDiff {
    metrics: EitherOrBoth,
}

impl From<MetricsDiff> for JsonNewMetric {
    fn from(value: MetricsDiff) -> Self {
        let value = match value.metrics {
            EitherOrBoth::Both(left, _) => left,
            EitherOrBoth::Left(left) => left,
        };

        match value {
            Metric::Int(v) => JsonNewMetric {
                value: (v as f64).into(),
                ..Default::default()
            },
            Metric::Float(v) => JsonNewMetric {
                value: v.into(),
                ..Default::default()
            },
        }
    }
}

#[derive(Deserialize)]
enum EitherOrBoth {
    Both(Metric, Metric),
    Left(Metric),
}

#[derive(Deserialize)]
enum Metric {
    Int(u64),
    Float(f64),
}

#[derive(Deserialize, Hash, Eq, PartialEq)]
pub enum CachegrindMetric {
    /// The default event. I cache reads (which equals the number of instructions executed)
    Ir,
    /// D Cache reads (which equals the number of memory reads) (--cache-sim=yes)
    Dr,
    /// D Cache writes (which equals the number of memory writes) (--cache-sim=yes)
    Dw,
    /// I1 cache read misses (--cache-sim=yes)
    I1mr,
    /// D1 cache read misses (--cache-sim=yes)
    D1mr,
    /// D1 cache write misses (--cache-sim=yes)
    D1mw,
    /// LL cache instruction read misses (--cache-sim=yes)
    ILmr,
    /// LL cache data read misses (--cache-sim=yes)
    DLmr,
    /// LL cache data write misses (--cache-sim=yes)
    DLmw,
    /// I1 cache miss rate (--cache-sim=yes)
    I1MissRate,
    /// LL/L2 instructions cache miss rate (--cache-sim=yes)
    LLiMissRate,
    /// D1 cache miss rate (--cache-sim=yes)
    D1MissRate,
    /// LL/L2 data cache miss rate (--cache-sim=yes)
    LLdMissRate,
    /// LL/L2 cache miss rate (--cache-sim=yes)
    LLMissRate,
    /// Derived event showing the L1 hits (--cache-sim=yes)
    L1hits,
    /// Derived event showing the LL hits (--cache-sim=yes)
    LLhits,
    /// Derived event showing the RAM hits (--cache-sim=yes)
    RamHits,
    /// L1 cache hit rate (--cache-sim=yes)
    L1HitRate,
    /// LL/L2 cache hit rate (--cache-sim=yes)
    LLHitRate,
    /// RAM hit rate (--cache-sim=yes)
    RamHitRate,
    /// Derived event showing the total amount of cache reads and writes (--cache-sim=yes)
    TotalRW,
    /// Derived event showing estimated CPU cycles (--cache-sim=yes)
    EstimatedCycles,
    /// Conditional branches executed (--branch-sim=yes)
    Bc,
    /// Conditional branches mispredicted (--branch-sim=yes)
    Bcm,
    /// Indirect branches executed (--branch-sim=yes)
    Bi,
    /// Indirect branches mispredicted (--branch-sim=yes)
    Bim,
}

#[derive(Deserialize, Hash, Eq, PartialEq)]
pub enum EventKind {
    /// The default event. I cache reads (which equals the number of instructions executed)
    Ir,
    /// D Cache reads (which equals the number of memory reads) (--cache-sim=yes)
    Dr,
    /// D Cache writes (which equals the number of memory writes) (--cache-sim=yes)
    Dw,
    /// I1 cache read misses (--cache-sim=yes)
    I1mr,
    /// D1 cache read misses (--cache-sim=yes)
    D1mr,
    /// D1 cache write misses (--cache-sim=yes)
    D1mw,
    /// LL cache instruction read misses (--cache-sim=yes)
    ILmr,
    /// LL cache data read misses (--cache-sim=yes)
    DLmr,
    /// LL cache data write misses (--cache-sim=yes)
    DLmw,
    /// I1 cache miss rate (--cache-sim=yes)
    I1MissRate,
    /// LL/L2 instructions cache miss rate (--cache-sim=yes)
    LLiMissRate,
    /// D1 cache miss rate (--cache-sim=yes)
    D1MissRate,
    /// LL/L2 data cache miss rate (--cache-sim=yes)
    LLdMissRate,
    /// LL/L2 cache miss rate (--cache-sim=yes)
    LLMissRate,
    /// Derived event showing the L1 hits (--cache-sim=yes)
    L1hits,
    /// Derived event showing the LL hits (--cache-sim=yes)
    LLhits,
    /// Derived event showing the RAM hits (--cache-sim=yes)
    RamHits,
    /// L1 cache hit rate (--cache-sim=yes)
    L1HitRate,
    /// LL/L2 cache hit rate (--cache-sim=yes)
    LLHitRate,
    /// RAM hit rate (--cache-sim=yes)
    RamHitRate,
    /// Derived event showing the total amount of cache reads and writes (--cache-sim=yes)
    TotalRW,
    /// Derived event showing estimated CPU cycles (--cache-sim=yes)
    EstimatedCycles,
    /// The number of system calls done (--collect-systime=yes)
    SysCount,
    /// The elapsed time spent in system calls (--collect-systime=yes)
    SysTime,
    /// The cpu time spent during system calls (--collect-systime=nsec)
    SysCpuTime,
    /// The number of global bus events (--collect-bus=yes)
    Ge,
    /// Conditional branches executed (--branch-sim=yes)
    Bc,
    /// Conditional branches mispredicted (--branch-sim=yes)
    Bcm,
    /// Indirect branches executed (--branch-sim=yes)
    Bi,
    /// Indirect branches mispredicted (--branch-sim=yes)
    Bim,
    /// Dirty miss because of instruction read (--simulate-wb=yes)
    ILdmr,
    /// Dirty miss because of data read (--simulate-wb=yes)
    DLdmr,
    /// Dirty miss because of data write (--simulate-wb=yes)
    DLdmw,
    /// Counter showing bad temporal locality for L1 caches (--cachuse=yes)
    AcCost1,
    /// Counter showing bad temporal locality for LL caches (--cachuse=yes)
    AcCost2,
    /// Counter showing bad spatial locality for L1 caches (--cachuse=yes)
    SpLoss1,
    /// Counter showing bad spatial locality for LL caches (--cachuse=yes)
    SpLoss2,
}

#[derive(Deserialize, Hash, Eq, PartialEq)]
pub enum DhatMetric {
    /// In ad-hoc mode, Total units measured over the entire execution
    TotalUnits,
    /// Total ad-hoc events over the entire execution
    TotalEvents,
    /// Total bytes allocated over the entire execution
    TotalBytes,
    /// Total heap blocks allocated over the entire execution
    TotalBlocks,
    /// The bytes alive at t-gmax, the time when the heap size reached its global maximum
    AtTGmaxBytes,
    /// The blocks alive at t-gmax
    AtTGmaxBlocks,
    /// The amount of bytes at the end of the execution.
    ///
    /// This is the amount of bytes which were not explicitly freed.
    AtTEndBytes,
    /// The amount of blocks at the end of the execution.
    ///
    /// This is the amount of heap blocks which were not explicitly freed.
    AtTEndBlocks,
    /// The amount of bytes read during the entire execution
    ReadsBytes,
    /// The amount of bytes written during the entire execution
    WritesBytes,
    /// The total lifetimes of all heap blocks allocated
    TotalLifetimes,
    /// The maximum amount of bytes
    MaximumBytes,
    /// The maximum amount of heap blocks
    MaximumBlocks,
}

#[derive(Deserialize, Hash, Eq, PartialEq)]
pub enum ErrorMetric {
    /// The amount of detected unsuppressed errors
    Errors,
    /// The amount of detected unsuppressed error contexts
    Contexts,
    /// The amount of suppressed errors
    SuppressedErrors,
    /// The amount of suppressed error contexts
    SuppressedContexts,
}

#[cfg(test)]
pub(crate) mod test_rust_gungraun_json {
    // TODO: add tests
}
