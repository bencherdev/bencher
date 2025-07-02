use std::{collections::HashMap, str::FromStr as _};

use bencher_json::{
    BenchmarkName, JsonNewMetric,
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

pub type ResultsMap = HashMap<BenchmarkName, AdapterMetrics>;

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
pub enum IaiCallgrindMeasure {
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
     * Unknown
     */
    Unknown(JsonNewMetric),
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
            results_map.insert(benchmark_name, adapter_metrics);
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
                .entry(benchmark_name)
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
    pub fn new_iai_callgrind(
        benchmark_metrics: Vec<(BenchmarkName, Vec<IaiCallgrindMeasure>)>,
    ) -> Option<Self> {
        if benchmark_metrics.is_empty() {
            return None;
        }

        let mut results_map = HashMap::new();
        for (benchmark_name, metrics) in benchmark_metrics {
            let metrics_value = results_map
                .entry(benchmark_name)
                .or_insert_with(AdapterMetrics::default);
            for metric in metrics {
                let (resource_id, metric) = match metric {
                    IaiCallgrindMeasure::Instructions(json_metric) => (
                        built_in::iai_callgrind::Instructions::name_id(),
                        json_metric,
                    ),
                    IaiCallgrindMeasure::L1Hits(json_metric) => {
                        (built_in::iai_callgrind::L1Hits::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::L2Hits(json_metric) => {
                        (built_in::iai_callgrind::L2Hits::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::LLHits(json_metric) => {
                        (built_in::iai_callgrind::LLHits::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::RamHits(json_metric) => {
                        (built_in::iai_callgrind::RamHits::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::TotalReadWrite(json_metric) => (
                        built_in::iai_callgrind::TotalReadWrite::name_id(),
                        json_metric,
                    ),
                    IaiCallgrindMeasure::EstimatedCycles(json_metric) => (
                        built_in::iai_callgrind::EstimatedCycles::name_id(),
                        json_metric,
                    ),
                    IaiCallgrindMeasure::GlobalBusEvents(json_metric) => (
                        built_in::iai_callgrind::GlobalBusEvents::name_id(),
                        json_metric,
                    ),
                    IaiCallgrindMeasure::DataCacheReads(json_metric) => {
                        (built_in::iai_callgrind::Dr::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::DataCacheWrites(json_metric) => {
                        (built_in::iai_callgrind::Dw::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::L1InstrCacheReadMisses(json_metric) => {
                        (built_in::iai_callgrind::I1mr::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::L1DataCacheReadMisses(json_metric) => {
                        (built_in::iai_callgrind::D1mr::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::L1DataCacheWriteMisses(json_metric) => {
                        (built_in::iai_callgrind::D1mw::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::LLInstrCacheReadMisses(json_metric) => {
                        (built_in::iai_callgrind::ILmr::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::LLDataCacheReadMisses(json_metric) => {
                        (built_in::iai_callgrind::DLmr::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::LLDataCacheWriteMisses(json_metric) => {
                        (built_in::iai_callgrind::DLmw::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::L1InstrCacheMissRate(json_metric) => {
                        (built_in::iai_callgrind::I1MissRate::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::LLInstrCacheMissRate(json_metric) => {
                        (built_in::iai_callgrind::LLiMissRate::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::L1DataCacheMissRate(json_metric) => {
                        (built_in::iai_callgrind::D1MissRate::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::LLDataCacheMissRate(json_metric) => {
                        (built_in::iai_callgrind::LLdMissRate::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::LLCacheMissRate(json_metric) => {
                        (built_in::iai_callgrind::LLMissRate::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::L1HitRate(json_metric) => {
                        (built_in::iai_callgrind::L1HitRate::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::LLHitRate(json_metric) => {
                        (built_in::iai_callgrind::LLHitRate::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::RamHitRate(json_metric) => {
                        (built_in::iai_callgrind::RamHitRate::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::NumberSystemCalls(json_metric) => {
                        (built_in::iai_callgrind::SysCount::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::TimeSystemCalls(json_metric) => {
                        (built_in::iai_callgrind::SysTime::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::CpuTimeSystemCalls(json_metric) => {
                        (built_in::iai_callgrind::SysCpuTime::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::ExecutedConditionalBranches(json_metric) => {
                        (built_in::iai_callgrind::Bc::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::MispredictedConditionalBranches(json_metric) => {
                        (built_in::iai_callgrind::Bcm::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::ExecutedIndirectBranches(json_metric) => {
                        (built_in::iai_callgrind::Bi::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::MispredictedIndirectBranches(json_metric) => {
                        (built_in::iai_callgrind::Bim::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::DirtyMissInstructionRead(json_metric) => {
                        (built_in::iai_callgrind::ILdmr::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::DirtyMissDataRead(json_metric) => {
                        (built_in::iai_callgrind::DLdmr::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::DirtyMissDataWrite(json_metric) => {
                        (built_in::iai_callgrind::DLdmw::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::L1BadTemporalLocality(json_metric) => {
                        (built_in::iai_callgrind::AcCost1::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::LLBadTemporalLocality(json_metric) => {
                        (built_in::iai_callgrind::AcCost2::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::L1BadSpatialLocality(json_metric) => {
                        (built_in::iai_callgrind::SpLoss1::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::LLBadSpatialLocality(json_metric) => {
                        (built_in::iai_callgrind::SpLoss2::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::TotalBytes(json_metric) => {
                        (built_in::iai_callgrind::TotalBytes::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::TotalBlocks(json_metric) => {
                        (built_in::iai_callgrind::TotalBlocks::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::AtTGmaxBytes(json_metric) => (
                        built_in::iai_callgrind::AtTGmaxBytes::name_id(),
                        json_metric,
                    ),
                    IaiCallgrindMeasure::AtTGmaxBlocks(json_metric) => (
                        built_in::iai_callgrind::AtTGmaxBlocks::name_id(),
                        json_metric,
                    ),
                    IaiCallgrindMeasure::AtTEndBytes(json_metric) => {
                        (built_in::iai_callgrind::AtTEndBytes::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::AtTEndBlocks(json_metric) => (
                        built_in::iai_callgrind::AtTEndBlocks::name_id(),
                        json_metric,
                    ),
                    IaiCallgrindMeasure::ReadsBytes(json_metric) => {
                        (built_in::iai_callgrind::ReadsBytes::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::WritesBytes(json_metric) => {
                        (built_in::iai_callgrind::WritesBytes::name_id(), json_metric)
                    },
                    IaiCallgrindMeasure::Unknown(_) => {
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

    pub fn get(&self, key: &str) -> Option<&AdapterMetrics> {
        self.inner.get(&BenchmarkName::from_str(key).ok()?)
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
