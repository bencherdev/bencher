use std::{collections::HashMap, str::FromStr};

use bencher_json::{
    project::{
        metric::Mean,
        metric_kind::{
            ESTIMATED_CYCLES_SLUG_STR, INSTRUCTIONS_SLUG_STR, L1_ACCESSES_SLUG_STR,
            L2_ACCESSES_SLUG_STR, LATENCY_SLUG_STR, RAM_ACCESSES_SLUG_STR, THROUGHPUT_SLUG_STR,
        },
    },
    BenchmarkName, JsonMetric, ResourceId,
};
use literally::hmap;
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};

use super::{adapter_metrics::AdapterMetrics, CombinedKind};

const METRIC_KIND_SLUG_ERROR: &str = "Failed to parse metric kind slug.";

#[allow(clippy::expect_used)]
pub static LATENCY_RESOURCE_ID: Lazy<ResourceId> =
    Lazy::new(|| LATENCY_SLUG_STR.parse().expect(METRIC_KIND_SLUG_ERROR));

#[allow(clippy::expect_used)]
pub static THROUGHPUT_RESOURCE_ID: Lazy<ResourceId> =
    Lazy::new(|| THROUGHPUT_SLUG_STR.parse().expect(METRIC_KIND_SLUG_ERROR));

#[allow(clippy::expect_used)]
pub static INSTRUCTIONS_RESOURCE_ID: Lazy<ResourceId> =
    Lazy::new(|| INSTRUCTIONS_SLUG_STR.parse().expect(METRIC_KIND_SLUG_ERROR));

#[allow(clippy::expect_used)]
pub static L1_ACCESSES_RESOURCE_ID: Lazy<ResourceId> =
    Lazy::new(|| L1_ACCESSES_SLUG_STR.parse().expect(METRIC_KIND_SLUG_ERROR));

#[allow(clippy::expect_used)]
pub static L2_ACCESSES_RESOURCE_ID: Lazy<ResourceId> =
    Lazy::new(|| L2_ACCESSES_SLUG_STR.parse().expect(METRIC_KIND_SLUG_ERROR));

#[allow(clippy::expect_used)]
pub static RAM_ACCESSES_RESOURCE_ID: Lazy<ResourceId> =
    Lazy::new(|| RAM_ACCESSES_SLUG_STR.parse().expect(METRIC_KIND_SLUG_ERROR));

#[allow(clippy::expect_used)]
pub static ESTIMATED_CYCLES_RESOURCE_ID: Lazy<ResourceId> = Lazy::new(|| {
    ESTIMATED_CYCLES_SLUG_STR
        .parse()
        .expect(METRIC_KIND_SLUG_ERROR)
});

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
pub enum AdapterMetricKind {
    Latency(JsonMetric),
    Throughput(JsonMetric),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IaiMetricKind {
    Instructions(JsonMetric),
    L1Accesses(JsonMetric),
    L2Accesses(JsonMetric),
    RamAccesses(JsonMetric),
    EstimatedCycles(JsonMetric),
}

impl AdapterResults {
    pub fn new(benchmark_metrics: Vec<(BenchmarkName, AdapterMetricKind)>) -> Option<Self> {
        if benchmark_metrics.is_empty() {
            return None;
        }

        let mut results_map = HashMap::new();
        for (benchmark_name, metric_kind) in benchmark_metrics {
            let adapter_metrics = AdapterMetrics {
                inner: match metric_kind {
                    AdapterMetricKind::Latency(json_metric) => {
                        hmap! {
                            LATENCY_RESOURCE_ID.clone() => json_metric
                        }
                    },
                    AdapterMetricKind::Throughput(json_metric) => {
                        hmap! {
                            THROUGHPUT_RESOURCE_ID.clone() => json_metric
                        }
                    },
                },
            };
            results_map.insert(benchmark_name, adapter_metrics);
        }

        Some(results_map.into())
    }

    pub fn new_latency(benchmark_metrics: Vec<(BenchmarkName, JsonMetric)>) -> Option<Self> {
        Self::new(
            benchmark_metrics
                .into_iter()
                .map(|(benchmark_name, json_metric)| {
                    (benchmark_name, AdapterMetricKind::Latency(json_metric))
                })
                .collect(),
        )
    }

    pub fn new_throughput(benchmark_metrics: Vec<(BenchmarkName, JsonMetric)>) -> Option<Self> {
        Self::new(
            benchmark_metrics
                .into_iter()
                .map(|(benchmark_name, json_metric)| {
                    (benchmark_name, AdapterMetricKind::Throughput(json_metric))
                })
                .collect(),
        )
    }

    pub fn new_iai(benchmark_metrics: Vec<(BenchmarkName, Vec<IaiMetricKind>)>) -> Option<Self> {
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
                    IaiMetricKind::Instructions(json_metric) => {
                        (INSTRUCTIONS_RESOURCE_ID.clone(), json_metric)
                    },
                    IaiMetricKind::L1Accesses(json_metric) => {
                        (L1_ACCESSES_RESOURCE_ID.clone(), json_metric)
                    },
                    IaiMetricKind::L2Accesses(json_metric) => {
                        (L2_ACCESSES_RESOURCE_ID.clone(), json_metric)
                    },
                    IaiMetricKind::RamAccesses(json_metric) => {
                        (RAM_ACCESSES_RESOURCE_ID.clone(), json_metric)
                    },
                    IaiMetricKind::EstimatedCycles(json_metric) => {
                        (ESTIMATED_CYCLES_RESOURCE_ID.clone(), json_metric)
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
        results_map.extend(other.inner.into_iter());
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
    #[allow(clippy::arithmetic_side_effects)]
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

    #[allow(clippy::arithmetic_side_effects)]
    fn div(self, rhs: usize) -> Self::Output {
        self.inner
            .into_iter()
            .map(|(benchmark_name, metrics)| (benchmark_name, metrics / rhs))
            .collect::<ResultsMap>()
            .into()
    }
}

impl Mean for AdapterResults {}
