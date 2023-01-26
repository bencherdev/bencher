use std::{collections::HashMap, str::FromStr};

use bencher_json::{
    project::{benchmark::BenchmarkName, metric::Mean},
    JsonMetric,
};
use literally::hmap;
use serde::{Deserialize, Serialize};

use crate::AdapterError;

use super::{adapter_metrics::AdapterMetrics, CombinedKind, LATENCY_RESOURCE_ID};

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

impl TryFrom<Vec<(String, JsonMetric)>> for AdapterResults {
    type Error = AdapterError;

    fn try_from(benchmark_metrics: Vec<(String, JsonMetric)>) -> Result<Self, Self::Error> {
        let mut results_map = HashMap::new();
        for (benchmark_name, metric) in benchmark_metrics {
            results_map.insert(
                benchmark_name
                    .as_str()
                    .parse()
                    .map_err(AdapterError::BenchmarkName)?,
                AdapterMetrics {
                    inner: hmap! {
                        LATENCY_RESOURCE_ID.clone() => metric
                    },
                },
            );
        }
        Ok(results_map.into())
    }
}

impl AdapterResults {
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
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
