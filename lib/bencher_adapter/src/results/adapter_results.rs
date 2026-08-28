use std::collections::{BTreeMap, HashMap};

use bencher_json::{
    BenchmarkName, BenchmarkNameId, JsonNewMetric, ParameterSet,
    project::measure::built_in::{self, BuiltInMeasure as _},
};

use super::{
    adapter_metrics::{AdapterMetric, AdapterMetrics, MetricsMap},
    foldable::{FoldableMap, FoldableResults},
};

/// Everything one results payload reported.
///
/// A benchmark name maps to its variants rather than straight to its
/// measures, because BMF v1 lets one benchmark report several parameter sets.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AdapterResults {
    pub inner: ResultsMap,
    /// The BMF version these results were parsed from.
    /// Fold is a v0 only operation, so this is what gates it.
    pub version: BmfVersion,
    /// How many named values the per measure cap dropped.
    /// The log line and the counter belong to ingest, where the providers are in scope.
    pub dropped_names: usize,
}

pub type ResultsMap = HashMap<BenchmarkNameId, BenchmarkEntries>;

/// Every variant one benchmark reported, keyed by its canonical parameter set.
///
/// The empty parameter set is the key every BMF v0 adapter uses, since a v0
/// payload only ever reports one variant per benchmark.
pub type BenchmarkEntries = BTreeMap<ParameterSet, AdapterMetrics>;

/// The Bencher Metric Format version a results payload was parsed from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum BmfVersion {
    /// A benchmark name maps to its measures.
    #[default]
    V0,
    /// A benchmark name maps to an array of parameter set entries.
    V1,
}

impl From<ResultsMap> for AdapterResults {
    fn from(inner: ResultsMap) -> Self {
        Self {
            inner,
            version: BmfVersion::V0,
            dropped_names: 0,
        }
    }
}

/// Folded results are BMF v0 again: one variant per benchmark, on the empty
/// parameter set, with each metric triple spelled back out as its conventional names.
///
/// The round trip through [`AdapterResults::into_foldable`] and back is lossless
/// because fold only ever runs on a v0 payload, which is exactly this shape.
impl From<FoldableResults> for AdapterResults {
    fn from(results: FoldableResults) -> Self {
        results
            .inner
            .into_iter()
            .map(|(benchmark, metrics)| {
                let metrics: AdapterMetrics = metrics
                    .into_iter()
                    .map(|(measure, metric)| (measure, AdapterMetric::from(metric)))
                    .collect::<MetricsMap>()
                    .into();
                (
                    benchmark,
                    std::iter::once((ParameterSet::default(), metrics)).collect(),
                )
            })
            .collect::<ResultsMap>()
            .into()
    }
}

/// The metrics of a benchmark's empty parameter set, created if absent.
///
/// Every adapter but `json_v1` reports one variant per benchmark and does not
/// need to know that parameter sets exist.
fn empty_set(results_map: &mut ResultsMap, benchmark_name: BenchmarkName) -> &mut AdapterMetrics {
    results_map
        .entry(BenchmarkNameId::new_name(benchmark_name))
        .or_default()
        .entry(ParameterSet::default())
        .or_default()
}

#[derive(Debug, Clone)]
pub enum AdapterMeasure {
    Latency(JsonNewMetric),
    Throughput(JsonNewMetric),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DotNetMeasure {
    Latency(JsonNewMetric),
    Gen0Collects(JsonNewMetric),
    Gen1Collects(JsonNewMetric),
    Gen2Collects(JsonNewMetric),
    TotalOperations(JsonNewMetric),
    Allocated(JsonNewMetric),
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
    TotalUnits(JsonNewMetric),
    TotalEvents(JsonNewMetric),
    TotalLifetimes(JsonNewMetric),
    AtTGmaxBytes(JsonNewMetric),
    AtTGmaxBlocks(JsonNewMetric),
    AtTEndBytes(JsonNewMetric),
    AtTEndBlocks(JsonNewMetric),
    ReadsBytes(JsonNewMetric),
    WritesBytes(JsonNewMetric),
    MaximumBytes(JsonNewMetric),
    MaximumBlocks(JsonNewMetric),

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

        let mut results_map = ResultsMap::new();
        for (benchmark_name, measure) in benchmark_metrics {
            let (resource_id, json_metric) = match measure {
                AdapterMeasure::Latency(json_metric) => {
                    (built_in::default::Latency::name_id(), json_metric)
                },
                AdapterMeasure::Throughput(json_metric) => {
                    (built_in::default::Throughput::name_id(), json_metric)
                },
            };
            empty_set(&mut results_map, benchmark_name)
                .inner
                .insert(resource_id, json_metric.into());
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

    /// Create results where each benchmark may report multiple default measures
    /// (e.g. both `Latency` and `Throughput`).
    pub fn new_measures(
        benchmark_metrics: Vec<(BenchmarkName, Vec<AdapterMeasure>)>,
    ) -> Option<Self> {
        if benchmark_metrics.is_empty() {
            return None;
        }

        let mut results_map = ResultsMap::new();
        for (benchmark_name, measures) in benchmark_metrics {
            let metrics_value = empty_set(&mut results_map, benchmark_name);
            for measure in measures {
                let (resource_id, metric) = match measure {
                    AdapterMeasure::Latency(json_metric) => {
                        (built_in::default::Latency::name_id(), json_metric)
                    },
                    AdapterMeasure::Throughput(json_metric) => {
                        (built_in::default::Throughput::name_id(), json_metric)
                    },
                };
                metrics_value.inner.insert(resource_id, metric.into());
            }
        }

        Some(results_map.into())
    }

    pub fn new_iai(benchmark_metrics: Vec<(BenchmarkName, Vec<IaiMeasure>)>) -> Option<Self> {
        if benchmark_metrics.is_empty() {
            return None;
        }

        let mut results_map = ResultsMap::new();
        for (benchmark_name, metrics) in benchmark_metrics {
            let metrics_value = empty_set(&mut results_map, benchmark_name);
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
                metrics_value.inner.insert(resource_id, metric.into());
            }
        }

        Some(results_map.into())
    }

    pub fn new_dotnet(benchmark_metrics: Vec<(BenchmarkName, Vec<DotNetMeasure>)>) -> Option<Self> {
        if benchmark_metrics.is_empty() {
            return None;
        }

        let mut results_map = ResultsMap::new();
        for (benchmark_name, measure) in benchmark_metrics {
            let metrics_value = empty_set(&mut results_map, benchmark_name);
            for metric in measure {
                let (resource_id, metric) = match metric {
                    DotNetMeasure::Latency(json_metric) => {
                        (built_in::default::Latency::name_id(), json_metric)
                    },
                    DotNetMeasure::Allocated(json_metric) => {
                        (built_in::dotnet::Allocated::name_id(), json_metric)
                    },
                    DotNetMeasure::Gen0Collects(json_metric) => {
                        (built_in::dotnet::Gen0Collects::name_id(), json_metric)
                    },
                    DotNetMeasure::Gen1Collects(json_metric) => {
                        (built_in::dotnet::Gen1Collects::name_id(), json_metric)
                    },
                    DotNetMeasure::Gen2Collects(json_metric) => {
                        (built_in::dotnet::Gen2Collects::name_id(), json_metric)
                    },
                    DotNetMeasure::TotalOperations(json_metric) => {
                        (built_in::dotnet::TotalOperations::name_id(), json_metric)
                    },
                };
                metrics_value.inner.insert(resource_id, metric.into());
            }
        }

        Some(results_map.into())
    }

    #[expect(
        clippy::too_many_lines,
        reason = "exhaustive match over all GungraunMeasure variants"
    )]
    pub fn new_gungraun(
        benchmark_metrics: Vec<(BenchmarkName, Vec<GungraunMeasure>)>,
    ) -> Option<Self> {
        if benchmark_metrics.is_empty() {
            return None;
        }

        let mut results_map = ResultsMap::new();
        for (benchmark_name, metrics) in benchmark_metrics {
            let metrics_value = empty_set(&mut results_map, benchmark_name);
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
                    GungraunMeasure::TotalUnits(json_metric) => {
                        (built_in::gungraun::TotalUnits::name_id(), json_metric)
                    },
                    GungraunMeasure::TotalEvents(json_metric) => {
                        (built_in::gungraun::TotalEvents::name_id(), json_metric)
                    },
                    GungraunMeasure::TotalLifetimes(json_metric) => {
                        (built_in::gungraun::TotalLifetimes::name_id(), json_metric)
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
                    GungraunMeasure::MaximumBytes(json_metric) => {
                        (built_in::gungraun::MaximumBytes::name_id(), json_metric)
                    },
                    GungraunMeasure::MaximumBlocks(json_metric) => {
                        (built_in::gungraun::MaximumBlocks::name_id(), json_metric)
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
                metrics_value.inner.insert(resource_id, metric.into());
            }
        }

        Some(results_map.into())
    }

    #[cfg(test)]
    pub fn get(&self, key: &str) -> Option<&AdapterMetrics> {
        use std::str::FromStr as _;

        self.entry(&BenchmarkNameId::new_name(
            BenchmarkName::from_str(key).ok()?,
        ))
    }

    /// The metrics a benchmark reported on the empty parameter set.
    #[cfg(test)]
    pub fn entry(&self, benchmark: &BenchmarkNameId) -> Option<&AdapterMetrics> {
        self.inner.get(benchmark)?.get(&ParameterSet::default())
    }

    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Whether these results are a BMF v0 payload, which is what fold operates on.
    pub(crate) fn is_foldable(&self) -> bool {
        self.version == BmfVersion::V0
    }

    /// The BMF v0 view of these results, only ever taken once [`Self::is_foldable`] holds.
    pub(crate) fn into_foldable(self) -> FoldableResults {
        let mut fold_map = FoldableMap::with_capacity(self.inner.len());
        for (benchmark, mut entries) in self.inner {
            let Some(metrics) = entries.remove(&ParameterSet::default()) else {
                continue;
            };
            let metrics = metrics
                .inner
                .into_iter()
                .filter_map(|(measure, metric)| Some((measure, metric.triple()?)))
                .collect();
            fold_map.insert(benchmark, metrics);
        }
        fold_map.into()
    }
}
