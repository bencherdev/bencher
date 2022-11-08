use std::collections::HashMap;

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::{mean::Mean, metrics::JsonMetrics, metrics_map::JsonMetricsMap};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBenchmarks {
    #[serde(rename = "benchmarks")]
    pub inner: Vec<JsonBenchmarksMap>,
}

impl From<Vec<JsonBenchmarksMap>> for JsonBenchmarks {
    fn from(inner: Vec<JsonBenchmarksMap>) -> Self {
        Self { inner }
    }
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum OrdKind {
    Min,
    Max,
}

impl JsonBenchmarks {
    pub fn min(self) -> Self {
        self.ord(OrdKind::Min)
    }

    pub fn max(self) -> Self {
        self.ord(OrdKind::Max)
    }

    fn ord(self, ord_kind: OrdKind) -> Self {
        let map = self.inner.into_iter().fold(
            HashMap::new().into(),
            |ord_map: JsonBenchmarksMap, next_map| {
                ord_map.combined(next_map, CombinedKind::Ord(ord_kind))
            },
        );
        vec![map].into()
    }

    pub fn mean(self) -> Self {
        JsonBenchmarksMap::mean(self.inner)
            .map(|mean| vec![mean])
            .unwrap_or_default()
            .into()
    }

    pub fn median(self) -> Self {
        vec![JsonMetricsMap::from(self)
            .inner
            .into_iter()
            .map(|(benchmark_name, json_metrics_map)| (benchmark_name, json_metrics_map.median()))
            .collect::<HashMap<String, JsonMetrics>>()
            .into()]
        .into()
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBenchmarksMap {
    #[serde(flatten)]
    pub inner: HashMap<String, JsonMetrics>,
}

impl From<HashMap<String, JsonMetrics>> for JsonBenchmarksMap {
    fn from(inner: HashMap<String, JsonMetrics>) -> Self {
        Self { inner }
    }
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum CombinedKind {
    Ord(OrdKind),
    Add,
}

impl JsonBenchmarksMap {
    fn combined(self, mut other: Self, kind: CombinedKind) -> Self {
        let mut benchmarks_map = HashMap::new();
        for (benchmark_name, metrics) in self.inner.into_iter() {
            let other_metrics = other.inner.remove(&benchmark_name);
            let combined_metrics = if let Some(other_metrics) = other_metrics {
                metrics.combined(other_metrics, kind)
            } else {
                metrics
            };
            benchmarks_map.insert(benchmark_name, combined_metrics);
        }
        for (benchmark_name, other_metrics) in other.inner.into_iter() {
            benchmarks_map.insert(benchmark_name, other_metrics);
        }
        benchmarks_map.into()
    }
}

impl std::ops::Add for JsonBenchmarksMap {
    type Output = Self;

    fn add(self, other: Self) -> Self {
        self.combined(other, CombinedKind::Add)
    }
}

impl std::iter::Sum for JsonBenchmarksMap {
    fn sum<I>(iter: I) -> Self
    where
        I: Iterator<Item = Self>,
    {
        iter.into_iter().fold(
            HashMap::new().into(),
            |acc_map: JsonBenchmarksMap, next_map| acc_map + next_map,
        )
    }
}

impl std::ops::Div<usize> for JsonBenchmarksMap {
    type Output = Self;

    fn div(self, rhs: usize) -> Self::Output {
        self.inner
            .into_iter()
            .map(|(name, metrics)| (name, metrics / rhs))
            .collect::<HashMap<String, JsonMetrics>>()
            .into()
    }
}

impl Mean for JsonBenchmarksMap {}
