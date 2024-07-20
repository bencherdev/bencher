use std::{collections::HashMap, str::FromStr};

use bencher_json::{
    project::{
        measure::defs::{self, MeasureDefinition},
        metric::Mean,
    },
    BenchmarkName, JsonNewMetric,
};
use literally::hmap;
use serde::{Deserialize, Serialize};

use super::{adapter_metrics::AdapterMetrics, CombinedKind};

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
    RamHits(JsonNewMetric),
    TotalReadWrite(JsonNewMetric),
    EstimatedCycles(JsonNewMetric),

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
                            defs::generic::Latency::name_id() => json_metric
                        }
                    },
                    AdapterMeasure::Throughput(json_metric) => {
                        hmap! {
                            defs::generic::Throughput::name_id() => json_metric
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
                        (defs::iai::Instructions::name_id(), json_metric)
                    },
                    IaiMeasure::L1Accesses(json_metric) => {
                        (defs::iai::L1Accesses::name_id(), json_metric)
                    },
                    IaiMeasure::L2Accesses(json_metric) => {
                        (defs::iai::L2Accesses::name_id(), json_metric)
                    },
                    IaiMeasure::RamAccesses(json_metric) => {
                        (defs::iai::RamAccesses::name_id(), json_metric)
                    },
                    IaiMeasure::EstimatedCycles(json_metric) => {
                        (defs::iai::EstimatedCycles::name_id(), json_metric)
                    },
                };
                metrics_value.inner.insert(resource_id, metric);
            }
        }

        Some(results_map.into())
    }

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
                    /*
                     * Callgrind tool:
                     */
                    IaiCallgrindMeasure::Instructions(json_metric) => (
                        defs::iai_callgrind::callgrind_tool::Instructions::name_id(),
                        json_metric,
                    ),
                    IaiCallgrindMeasure::L1Hits(json_metric) => (
                        defs::iai_callgrind::callgrind_tool::L1Hits::name_id(),
                        json_metric,
                    ),
                    IaiCallgrindMeasure::L2Hits(json_metric) => (
                        defs::iai_callgrind::callgrind_tool::L2Hits::name_id(),
                        json_metric,
                    ),
                    IaiCallgrindMeasure::RamHits(json_metric) => (
                        defs::iai_callgrind::callgrind_tool::RamHits::name_id(),
                        json_metric,
                    ),
                    IaiCallgrindMeasure::TotalReadWrite(json_metric) => (
                        defs::iai_callgrind::callgrind_tool::TotalReadWrite::name_id(),
                        json_metric,
                    ),
                    IaiCallgrindMeasure::EstimatedCycles(json_metric) => (
                        defs::iai_callgrind::callgrind_tool::EstimatedCycles::name_id(),
                        json_metric,
                    ),

                    /*
                     * DHAT tool:
                     */
                    IaiCallgrindMeasure::TotalBytes(json_metric) => (
                        defs::iai_callgrind::dhat_tool::TotalBytes::name_id(),
                        json_metric,
                    ),
                    IaiCallgrindMeasure::TotalBlocks(json_metric) => (
                        defs::iai_callgrind::dhat_tool::TotalBlocks::name_id(),
                        json_metric,
                    ),
                    IaiCallgrindMeasure::AtTGmaxBytes(json_metric) => (
                        defs::iai_callgrind::dhat_tool::AtTGmaxBytes::name_id(),
                        json_metric,
                    ),
                    IaiCallgrindMeasure::AtTGmaxBlocks(json_metric) => (
                        defs::iai_callgrind::dhat_tool::AtTGmaxBlocks::name_id(),
                        json_metric,
                    ),
                    IaiCallgrindMeasure::AtTEndBytes(json_metric) => (
                        defs::iai_callgrind::dhat_tool::AtTEndBytes::name_id(),
                        json_metric,
                    ),
                    IaiCallgrindMeasure::AtTEndBlocks(json_metric) => (
                        defs::iai_callgrind::dhat_tool::AtTEndBlocks::name_id(),
                        json_metric,
                    ),
                    IaiCallgrindMeasure::ReadsBytes(json_metric) => (
                        defs::iai_callgrind::dhat_tool::ReadsBytes::name_id(),
                        json_metric,
                    ),
                    IaiCallgrindMeasure::WritesBytes(json_metric) => (
                        defs::iai_callgrind::dhat_tool::WritesBytes::name_id(),
                        json_metric,
                    ),
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

    fn add(self, other: Self) -> Self {
        self.combined(other, CombinedKind::Add)
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
