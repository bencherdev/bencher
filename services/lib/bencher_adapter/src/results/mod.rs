use std::collections::HashMap;

use bencher_json::project::metric::Mean;
use serde::{Deserialize, Serialize};

pub mod adapter_metrics;
pub mod adapter_results;
pub mod results_reducer;

use adapter_results::{AdapterResults, ResultsMap};
use results_reducer::ResultsReducer;

pub type BenchmarkName = String;
pub type MetricKind = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterResultsArray {
    pub inner: Vec<AdapterResults>,
}

impl From<Vec<AdapterResults>> for AdapterResultsArray {
    fn from(inner: Vec<AdapterResults>) -> Self {
        Self { inner }
    }
}

impl AdapterResultsArray {
    pub fn min(self) -> Self {
        self.ord(OrdKind::Min)
    }

    pub fn max(self) -> Self {
        self.ord(OrdKind::Max)
    }

    fn ord(self, ord_kind: OrdKind) -> Self {
        let map = self.inner.into_iter().fold(
            HashMap::new().into(),
            |ord_map: AdapterResults, next_map| {
                ord_map.combined(next_map, CombinedKind::Ord(ord_kind))
            },
        );
        vec![map].into()
    }

    pub fn mean(self) -> Self {
        AdapterResults::mean(self.inner)
            .map(|mean| vec![mean])
            .unwrap_or_default()
            .into()
    }

    pub fn median(self) -> Self {
        vec![ResultsReducer::from(self)
            .inner
            .into_iter()
            .map(|(benchmark_name, json_metrics_map)| (benchmark_name, json_metrics_map.median()))
            .collect::<ResultsMap>()
            .into()]
        .into()
    }
}

#[derive(Debug, Copy, Clone)]
pub(crate) enum OrdKind {
    Min,
    Max,
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum CombinedKind {
    Ord(OrdKind),
    Add,
}
