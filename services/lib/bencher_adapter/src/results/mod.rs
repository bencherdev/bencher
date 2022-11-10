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
    pub results: Vec<AdapterResults>,
}

impl From<Vec<AdapterResults>> for AdapterResultsArray {
    fn from(results: Vec<AdapterResults>) -> Self {
        Self { results }
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
        let map = self.results.into_iter().fold(
            HashMap::new().into(),
            |results: AdapterResults, other_results| {
                results.combined(other_results, CombinedKind::Ord(ord_kind))
            },
        );
        vec![map].into()
    }

    pub fn mean(self) -> Self {
        AdapterResults::mean(self.results)
            .map(|mean| vec![mean])
            .unwrap_or_default()
            .into()
    }

    pub fn median(self) -> Self {
        vec![ResultsReducer::from(self)
            .inner
            .into_iter()
            .map(|(benchmark_name, results)| (benchmark_name, results.median()))
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
