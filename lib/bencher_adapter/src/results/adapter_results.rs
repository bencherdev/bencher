use std::{collections::HashMap, str::FromStr};

use bencher_json::{project::metric::Mean, BenchmarkName, JsonMetric};
use literally::hmap;
use serde::{Deserialize, Serialize};

use super::{
    adapter_metrics::AdapterMetrics, CombinedKind, CYCLES_RESOURCE_ID, INSTRUCTIONS_RESOURCE_ID,
    L1_ACCESSES_RESOURCE_ID, L2_ACCESSES_RESOURCE_ID, LATENCY_RESOURCE_ID,
    RAM_ACCESSES_RESOURCE_ID, THROUGHPUT_RESOURCE_ID,
};

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
    Instructions(JsonMetric),
    Cycles(JsonMetric),
    L1Accesses(JsonMetric),
    L2Accesses(JsonMetric),
    RamAccesses(JsonMetric),
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
                    AdapterMetricKind::Instructions(json_metric) => {
                        hmap! {
                            INSTRUCTIONS_RESOURCE_ID.clone() => json_metric,
                        }
                    },
                    AdapterMetricKind::Cycles(json_metric) => {
                        hmap! {
                            CYCLES_RESOURCE_ID.clone() => json_metric,
                        }
                    },
                    AdapterMetricKind::L1Accesses(json_metric) => {
                        hmap! {
                            L1_ACCESSES_RESOURCE_ID.clone() => json_metric,
                        }
                    },
                    AdapterMetricKind::L2Accesses(json_metric) => {
                        hmap! {
                            L2_ACCESSES_RESOURCE_ID.clone() => json_metric,
                        }
                    },
                    AdapterMetricKind::RamAccesses(json_metric) => {
                        hmap! {
                            RAM_ACCESSES_RESOURCE_ID.clone() => json_metric,
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
