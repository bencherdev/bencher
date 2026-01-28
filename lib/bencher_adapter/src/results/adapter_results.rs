use std::collections::HashMap;

use bencher_json::{
    BenchmarkName, BenchmarkNameId, JsonNewMetric,
    project::{
        measure::built_in::{self, BuiltInMeasure as _},
        metric::Mean,
    },
};
use literally::hmap;
use serde::{Deserialize, Serialize};

use super::{CombinedKind, adapter_metrics::AdapterMetrics};

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterResults {
    #[serde(flatten)]
    pub inner: ResultsMap,
}

pub type ResultsMap = HashMap<BenchmarkNameId, AdapterMetrics>;

impl From<ResultsMap> for AdapterResults {
    fn from(inner: ResultsMap) -> Self {
        Self { inner }
    }
}

#[derive(Debug, Clone)]
pub enum AdapterMeasure {
    Latency(JsonNewMetric),
    Throughput(JsonNewMetric),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IaiMeasure {
    Instructions(JsonNewMetric),
    L1Accesses(JsonNewMetric),
    L2Accesses(JsonNewMetric),
    RamAccesses(JsonNewMetric),
    EstimatedCycles(JsonNewMetric),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GungraunMeasure {
    /*
     * Callgrind tool:
     */
    Instructions(JsonNewMetric),
    L1Hits(JsonNewMetric),
    L2Hits(JsonNewMetric),
    LLHits(JsonNewMetric), // renamed from L2 Hits
    RamHits(JsonNewMetric),
    TotalReadWrite(JsonNewMetric),
    EstimatedCycles(JsonNewMetric),
    GlobalBusEvents(JsonNewMetric),        // Ge
    DataCacheReads(JsonNewMetric),         // Dr
    DataCacheWrites(JsonNewMetric),        // Dw
    L1InstrCacheReadMisses(JsonNewMetric), // I1mr
    L1DataCacheReadMisses(JsonNewMetric),  // D1mr
    L1DataCacheWriteMisses(JsonNewMetric), // D1mw
    LLInstrCacheReadMisses(JsonNewMetric), // ILmr
    LLDataCacheReadMisses(JsonNewMetric),  // DLmr
    LLDataCacheWriteMisses(JsonNewMetric), // DLmw
    L1InstrCacheMissRate(JsonNewMetric),   // I1MissRate
    LLInstrCacheMissRate(JsonNewMetric),   // LLiMissRate
    L1DataCacheMissRate(JsonNewMetric),    // D1MissRate
    LLDataCacheMissRate(JsonNewMetric),    // LLdMissRate
    LLCacheMissRate(JsonNewMetric),        // LLMissRate
    L1HitRate(JsonNewMetric),
    LLHitRate(JsonNewMetric),
    RamHitRate(JsonNewMetric),
    NumberSystemCalls(JsonNewMetric),               // SysCount
    TimeSystemCalls(JsonNewMetric),                 // SysTime
    CpuTimeSystemCalls(JsonNewMetric),              // SysCpuTime
    ExecutedConditionalBranches(JsonNewMetric),     // Bc
    MispredictedConditionalBranches(JsonNewMetric), // Bcm
    ExecutedIndirectBranches(JsonNewMetric),        // Bi
    MispredictedIndirectBranches(JsonNewMetric),    // Bim
    DirtyMissInstructionRead(JsonNewMetric),        // ILdmr
    DirtyMissDataRead(JsonNewMetric),               // DLdmr
    DirtyMissDataWrite(JsonNewMetric),              // DLdmw
    L1BadTemporalLocality(JsonNewMetric),           // AcLoss1
    LLBadTemporalLocality(JsonNewMetric),           // AcLoss2
    L1BadSpatialLocality(JsonNewMetric),            // SpLoss1
    LLBadSpatialLocality(JsonNewMetric),            // SpLoss2

    /*
     * DHAT tool:
     */
    TotalBytes(JsonNewMetric),
    TotalBlocks(JsonNewMetric),
    AtTGmaxBytes(JsonNewMetric),
    AtTGmaxBlocks(JsonNewMetric),
    AtTEndBytes(JsonNewMetric),
    AtTEndBlocks(JsonNewMetric),
    ReadsBytes(JsonNewMetric),
    WritesBytes(JsonNewMetric),

    /*
     * Memcheck tool:
     */
    MemcheckErrors(JsonNewMetric),
    MemcheckContexts(JsonNewMetric),
    MemcheckSuppressedErrors(JsonNewMetric),
    MemcheckSuppressedContexts(JsonNewMetric),

    /*
     * Helgrind tool:
     */
    HelgrindErrors(JsonNewMetric),
    HelgrindContexts(JsonNewMetric),
    HelgrindSuppressedErrors(JsonNewMetric),
    HelgrindSuppressedContexts(JsonNewMetric),

    /*
     * Drd tool:
     */
    DrdErrors(JsonNewMetric),
    DrdContexts(JsonNewMetric),
    DrdSuppressedErrors(JsonNewMetric),
    DrdSuppressedContexts(JsonNewMetric),

    /*
     * Unknown
     */
    Unknown,
}

impl AdapterResults {
    pub fn new(benchmark_metrics: Vec<(BenchmarkName, AdapterMeasure)>) -> Option<Self> {
        if benchmark_metrics.is_empty() {
            return None;
        }

        let mut results_map = HashMap::new();
        for (benchmark_name, measure) in benchmark_metrics {
            let adapter_metrics = AdapterMetrics {
                inner: match measure {
                    AdapterMeasure::Latency(json_metric) => {
                        hmap! {
                            built_in::default::Latency::name_id() => json_metric
                        }
                    },
                    AdapterMeasure::Throughput(json_metric) => {
                        hmap! {
                            built_in::default::Throughput::name_id() => json_metric
                        }
                    },
                },
            };
            results_map.insert(BenchmarkNameId::new_name(benchmark_name), adapter_metrics);
        }

        Some(results_map.into())
    }

    pub fn new_latency(benchmark_metrics: Vec<(BenchmarkName, JsonNewMetric)>) -> Option<Self> {
        Self::new(
            benchmark_metrics
                .into_iter()
                .map(|(benchmark_name, json_metric)| {
                    (benchmark_name, AdapterMeasure::Latency(json_metric))
                })
                .collect(),
        )
    }

    pub fn new_throughput(benchmark_metrics: Vec<(BenchmarkName, JsonNewMetric)>) -> Option<Self> {
        Self::new(
            benchmark_metrics
                .into_iter()
                .map(|(benchmark_name, json_metric)| {
                    (benchmark_name, AdapterMeasure::Throughput(json_metric))
                })
                .collect(),
        )
    }

    pub fn new_iai(benchmark_metrics: Vec<(BenchmarkName, Vec<IaiMeasure>)>) -> Option<Self> {
        if benchmark_metrics.is_empty() {
            return None;
        }

        let mut results_map = HashMap::new();
        for (benchmark_name, metrics) in benchmark_metrics {
            let metrics_value = results_map
                .entry(BenchmarkNameId::new_name(benchmark_name))
                .or_insert_with(AdapterMetrics::default);
            for metric in metrics {
                let (resource_id, metric) = match metric {
                    IaiMeasure::Instructions(json_metric) => {
                        (built_in::iai::Instructions::name_id(), json_metric)
                    },
                    IaiMeasure::L1Accesses(json_metric) => {
                        (built_in::iai::L1Accesses::name_id(), json_metric)
                    },
                    IaiMeasure::L2Accesses(json_metric) => {
                        (built_in::iai::L2Accesses::name_id(), json_metric)
                    },
                    IaiMeasure::RamAccesses(json_metric) => {
                        (built_in::iai::RamAccesses::name_id(), json_metric)
                    },
                    IaiMeasure::EstimatedCycles(json_metric) => {
                        (built_in::iai::EstimatedCycles::name_id(), json_metric)
                    },
                };
                metrics_value.inner.insert(resource_id, metric);
            }
        }

        Some(results_map.into())
    }

    #[expect(clippy::too_many_lines)]
    pub fn new_gungraun(
        benchmark_metrics: Vec<(BenchmarkName, Vec<GungraunMeasure>)>,
    ) -> Option<Self> {
        if benchmark_metrics.is_empty() {
            return None;
        }

        let mut results_map = HashMap::new();
        for (benchmark_name, metrics) in benchmark_metrics {
            let metrics_value = results_map
                .entry(BenchmarkNameId::new_name(benchmark_name))
                .or_insert_with(AdapterMetrics::default);
            for metric in metrics {
                let (resource_id, metric) = match metric {
                    // Callgrind/Cachgrind
                    GungraunMeasure::Instructions(json_metric) => {
                        (built_in::gungraun::Instructions::name_id(), json_metric)
                    },
                    GungraunMeasure::L1Hits(json_metric) => {
                        (built_in::gungraun::L1Hits::name_id(), json_metric)
                    },
                    GungraunMeasure::L2Hits(json_metric) => {
                        (built_in::gungraun::L2Hits::name_id(), json_metric)
                    },
                    GungraunMeasure::LLHits(json_metric) => {
                        (built_in::gungraun::LLHits::name_id(), json_metric)
                    },
                    GungraunMeasure::RamHits(json_metric) => {
                        (built_in::gungraun::RamHits::name_id(), json_metric)
                    },
                    GungraunMeasure::TotalReadWrite(json_metric) => {
                        (built_in::gungraun::TotalReadWrite::name_id(), json_metric)
                    },
                    GungraunMeasure::EstimatedCycles(json_metric) => {
                        (built_in::gungraun::EstimatedCycles::name_id(), json_metric)
                    },
                    GungraunMeasure::GlobalBusEvents(json_metric) => {
                        (built_in::gungraun::GlobalBusEvents::name_id(), json_metric)
                    },
                    GungraunMeasure::DataCacheReads(json_metric) => {
                        (built_in::gungraun::Dr::name_id(), json_metric)
                    },
                    GungraunMeasure::DataCacheWrites(json_metric) => {
                        (built_in::gungraun::Dw::name_id(), json_metric)
                    },
                    GungraunMeasure::L1InstrCacheReadMisses(json_metric) => {
                        (built_in::gungraun::I1mr::name_id(), json_metric)
                    },
                    GungraunMeasure::L1DataCacheReadMisses(json_metric) => {
                        (built_in::gungraun::D1mr::name_id(), json_metric)
                    },
                    GungraunMeasure::L1DataCacheWriteMisses(json_metric) => {
                        (built_in::gungraun::D1mw::name_id(), json_metric)
                    },
                    GungraunMeasure::LLInstrCacheReadMisses(json_metric) => {
                        (built_in::gungraun::ILmr::name_id(), json_metric)
                    },
                    GungraunMeasure::LLDataCacheReadMisses(json_metric) => {
                        (built_in::gungraun::DLmr::name_id(), json_metric)
                    },
                    GungraunMeasure::LLDataCacheWriteMisses(json_metric) => {
                        (built_in::gungraun::DLmw::name_id(), json_metric)
                    },
                    GungraunMeasure::L1InstrCacheMissRate(json_metric) => {
                        (built_in::gungraun::I1MissRate::name_id(), json_metric)
                    },
                    GungraunMeasure::LLInstrCacheMissRate(json_metric) => {
                        (built_in::gungraun::LLiMissRate::name_id(), json_metric)
                    },
                    GungraunMeasure::L1DataCacheMissRate(json_metric) => {
                        (built_in::gungraun::D1MissRate::name_id(), json_metric)
                    },
                    GungraunMeasure::LLDataCacheMissRate(json_metric) => {
                        (built_in::gungraun::LLdMissRate::name_id(), json_metric)
                    },
                    GungraunMeasure::LLCacheMissRate(json_metric) => {
                        (built_in::gungraun::LLMissRate::name_id(), json_metric)
                    },
                    GungraunMeasure::L1HitRate(json_metric) => {
                        (built_in::gungraun::L1HitRate::name_id(), json_metric)
                    },
                    GungraunMeasure::LLHitRate(json_metric) => {
                        (built_in::gungraun::LLHitRate::name_id(), json_metric)
                    },
                    GungraunMeasure::RamHitRate(json_metric) => {
                        (built_in::gungraun::RamHitRate::name_id(), json_metric)
                    },
                    GungraunMeasure::NumberSystemCalls(json_metric) => {
                        (built_in::gungraun::SysCount::name_id(), json_metric)
                    },
                    GungraunMeasure::TimeSystemCalls(json_metric) => {
                        (built_in::gungraun::SysTime::name_id(), json_metric)
                    },
                    GungraunMeasure::CpuTimeSystemCalls(json_metric) => {
                        (built_in::gungraun::SysCpuTime::name_id(), json_metric)
                    },
                    GungraunMeasure::ExecutedConditionalBranches(json_metric) => {
                        (built_in::gungraun::Bc::name_id(), json_metric)
                    },
                    GungraunMeasure::MispredictedConditionalBranches(json_metric) => {
                        (built_in::gungraun::Bcm::name_id(), json_metric)
                    },
                    GungraunMeasure::ExecutedIndirectBranches(json_metric) => {
                        (built_in::gungraun::Bi::name_id(), json_metric)
                    },
                    GungraunMeasure::MispredictedIndirectBranches(json_metric) => {
                        (built_in::gungraun::Bim::name_id(), json_metric)
                    },
                    GungraunMeasure::DirtyMissInstructionRead(json_metric) => {
                        (built_in::gungraun::ILdmr::name_id(), json_metric)
                    },
                    GungraunMeasure::DirtyMissDataRead(json_metric) => {
                        (built_in::gungraun::DLdmr::name_id(), json_metric)
                    },
                    GungraunMeasure::DirtyMissDataWrite(json_metric) => {
                        (built_in::gungraun::DLdmw::name_id(), json_metric)
                    },
                    GungraunMeasure::L1BadTemporalLocality(json_metric) => {
                        (built_in::gungraun::AcCost1::name_id(), json_metric)
                    },
                    GungraunMeasure::LLBadTemporalLocality(json_metric) => {
                        (built_in::gungraun::AcCost2::name_id(), json_metric)
                    },
                    GungraunMeasure::L1BadSpatialLocality(json_metric) => {
                        (built_in::gungraun::SpLoss1::name_id(), json_metric)
                    },
                    GungraunMeasure::LLBadSpatialLocality(json_metric) => {
                        (built_in::gungraun::SpLoss2::name_id(), json_metric)
                    },
                    // DHAT
                    GungraunMeasure::TotalBytes(json_metric) => {
                        (built_in::gungraun::TotalBytes::name_id(), json_metric)
                    },
                    GungraunMeasure::TotalBlocks(json_metric) => {
                        (built_in::gungraun::TotalBlocks::name_id(), json_metric)
                    },
                    GungraunMeasure::AtTGmaxBytes(json_metric) => {
                        (built_in::gungraun::AtTGmaxBytes::name_id(), json_metric)
                    },
                    GungraunMeasure::AtTGmaxBlocks(json_metric) => {
                        (built_in::gungraun::AtTGmaxBlocks::name_id(), json_metric)
                    },
                    GungraunMeasure::AtTEndBytes(json_metric) => {
                        (built_in::gungraun::AtTEndBytes::name_id(), json_metric)
                    },
                    GungraunMeasure::AtTEndBlocks(json_metric) => {
                        (built_in::gungraun::AtTEndBlocks::name_id(), json_metric)
                    },
                    GungraunMeasure::ReadsBytes(json_metric) => {
                        (built_in::gungraun::ReadsBytes::name_id(), json_metric)
                    },
                    GungraunMeasure::WritesBytes(json_metric) => {
                        (built_in::gungraun::WritesBytes::name_id(), json_metric)
                    },
                    // Memcheck
                    GungraunMeasure::MemcheckErrors(json_metric) => {
                        (built_in::gungraun::MemcheckErrors::name_id(), json_metric)
                    },
                    GungraunMeasure::MemcheckContexts(json_metric) => {
                        (built_in::gungraun::MemcheckContexts::name_id(), json_metric)
                    },
                    GungraunMeasure::MemcheckSuppressedErrors(json_metric) => (
                        built_in::gungraun::MemcheckSuppressedErrors::name_id(),
                        json_metric,
                    ),
                    GungraunMeasure::MemcheckSuppressedContexts(json_metric) => (
                        built_in::gungraun::MemcheckSuppressedContexts::name_id(),
                        json_metric,
                    ),
                    // Helgrind
                    GungraunMeasure::HelgrindErrors(json_metric) => {
                        (built_in::gungraun::HelgrindErrors::name_id(), json_metric)
                    },
                    GungraunMeasure::HelgrindContexts(json_metric) => {
                        (built_in::gungraun::HelgrindContexts::name_id(), json_metric)
                    },
                    GungraunMeasure::HelgrindSuppressedErrors(json_metric) => (
                        built_in::gungraun::HelgrindSuppressedErrors::name_id(),
                        json_metric,
                    ),
                    GungraunMeasure::HelgrindSuppressedContexts(json_metric) => (
                        built_in::gungraun::HelgrindSuppressedContexts::name_id(),
                        json_metric,
                    ),
                    // Drd
                    GungraunMeasure::DrdErrors(json_metric) => {
                        (built_in::gungraun::DrdErrors::name_id(), json_metric)
                    },
                    GungraunMeasure::DrdContexts(json_metric) => {
                        (built_in::gungraun::DrdContexts::name_id(), json_metric)
                    },
                    GungraunMeasure::DrdSuppressedErrors(json_metric) => (
                        built_in::gungraun::DrdSuppressedErrors::name_id(),
                        json_metric,
                    ),
                    GungraunMeasure::DrdSuppressedContexts(json_metric) => (
                        built_in::gungraun::DrdSuppressedContexts::name_id(),
                        json_metric,
                    ),
                    // Unknown
                    GungraunMeasure::Unknown => {
                        continue;
                    },
                };
                metrics_value.inner.insert(resource_id, metric);
            }
        }

        Some(results_map.into())
    }

    pub(crate) fn combined(self, mut other: Self, kind: CombinedKind) -> Self {
        let mut results_map = HashMap::new();
        for (benchmark_name, metrics) in self.inner {
            let other_metrics = other.inner.remove(&benchmark_name);
            let combined_metrics = if let Some(other_metrics) = other_metrics {
                metrics.combined(other_metrics, kind)
            } else {
                metrics
            };
            results_map.insert(benchmark_name, combined_metrics);
        }
        results_map.extend(other.inner);
        results_map.into()
    }

    #[cfg(test)]
    pub fn get(&self, key: &str) -> Option<&AdapterMetrics> {
        use std::str::FromStr as _;

        self.inner.get(&BenchmarkNameId::new_name(
            BenchmarkName::from_str(key).ok()?,
        ))
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl std::ops::Add for AdapterResults {
    type Output = Self;

    fn add(self, rhs: Self) -> Self {
        self.combined(rhs, CombinedKind::Add)
    }
}

impl std::iter::Sum for AdapterResults {
    fn sum<I>(iter: I) -> Self
    where
        I: Iterator<Item = Self>,
    {
        iter.into_iter().fold(
            HashMap::new().into(),
            |results: AdapterResults, other_results| results + other_results,
        )
    }
}

impl std::ops::Div<usize> for AdapterResults {
    type Output = Self;

    fn div(self, rhs: usize) -> Self::Output {
        self.inner
            .into_iter()
            .map(|(benchmark_name, metrics)| (benchmark_name, metrics / rhs))
            .collect::<ResultsMap>()
            .into()
    }
}

impl Mean for AdapterResults {}
