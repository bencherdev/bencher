use std::collections::HashMap;

use bencher_json::project::{metric::Mean, report::JsonFold};
use serde::{Deserialize, Serialize};

pub mod adapter_metrics;
pub mod adapter_results;
pub mod results_reducer;

use adapter_results::{AdapterResults, ResultsMap};
use results_reducer::ResultsReducer;

use crate::AdapterError;

pub type BenchmarkName = String;
pub type MetricKind = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AdapterResultsArray {
    pub inner: ResultsArray,
}

pub type ResultsArray = Vec<AdapterResults>;

impl From<ResultsArray> for AdapterResultsArray {
    fn from(inner: ResultsArray) -> Self {
        Self { inner }
    }
}

impl TryFrom<&[&str]> for AdapterResultsArray {
    type Error = AdapterError;

    fn try_from(results: &[&str]) -> Result<Self, Self::Error> {
        let mut results_array = Vec::new();
        for result in results {
            results_array.push(serde_json::from_str::<AdapterResults>(result)?);
        }
        Ok(results_array.into())
    }
}

impl AdapterResultsArray {
    pub fn min(self) -> AdapterResults {
        self.ord(OrdKind::Min)
    }

    pub fn max(self) -> AdapterResults {
        self.ord(OrdKind::Max)
    }

    fn ord(self, ord_kind: OrdKind) -> AdapterResults {
        self.inner.into_iter().fold(
            HashMap::new().into(),
            |results: AdapterResults, other_results| {
                results.combined(other_results, CombinedKind::Ord(ord_kind))
            },
        )
    }

    pub fn mean(self) -> AdapterResults {
        AdapterResults::mean(self.inner).unwrap_or_default()
    }

    pub fn median(self) -> AdapterResults {
        ResultsReducer::from(self)
            .inner
            .into_iter()
            .map(|(benchmark_name, results)| (benchmark_name, results.median()))
            .collect::<ResultsMap>()
            .into()
    }

    pub fn fold(self, fold: JsonFold) -> AdapterResults {
        if self.inner.is_empty() {
            return AdapterResults::default();
        }

        match fold {
            JsonFold::Min => self.min(),
            JsonFold::Max => self.max(),
            JsonFold::Mean => self.mean(),
            JsonFold::Median => self.median(),
        }
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
