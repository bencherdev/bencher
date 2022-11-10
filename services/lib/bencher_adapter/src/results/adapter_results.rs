use std::collections::HashMap;

use bencher_json::project::metric::Mean;
use serde::{Deserialize, Serialize};

use super::{adapter_metrics::AdapterMetrics, BenchmarkName, CombinedKind};

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

impl AdapterResults {
    pub fn combined(self, mut other: Self, kind: CombinedKind) -> Self {
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
            |acc_map: AdapterResults, next_map| acc_map + next_map,
        )
    }
}

impl std::ops::Div<usize> for AdapterResults {
    type Output = Self;

    fn div(self, rhs: usize) -> Self::Output {
        self.inner
            .into_iter()
            .map(|(name, metrics)| (name, metrics / rhs))
            .collect::<ResultsMap>()
            .into()
    }
}

impl Mean for AdapterResults {}
