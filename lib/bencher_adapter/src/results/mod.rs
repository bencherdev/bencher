use std::collections::HashMap;

use bencher_json::{
    project::{
        metric::Mean,
        report::{JsonAdapter, JsonFold},
    },
    ResourceId,
};

use crate::{Adapter, AdapterError, Settings};

pub mod adapter_metrics;
pub mod adapter_results;
pub mod results_reducer;

use adapter_results::{AdapterResults, ResultsMap};
use results_reducer::ResultsReducer;

pub type MetricKind = ResourceId;

#[derive(Debug, Clone)]
pub struct AdapterResultsArray {
    pub inner: ResultsArray,
}

pub type ResultsArray = Vec<AdapterResults>;

impl From<ResultsArray> for AdapterResultsArray {
    fn from(inner: ResultsArray) -> Self {
        Self { inner }
    }
}

impl AdapterResultsArray {
    pub fn new(
        results_array: &[&str],
        adapter: JsonAdapter,
        settings: Settings,
    ) -> Result<Self, AdapterError> {
        let mut parsed_results_array = Vec::new();
        for results in results_array {
            let parsed_results = adapter
                .convert(results, settings)
                .ok_or_else(|| AdapterError::Convert((*results).to_string()))?;
            parsed_results_array.push(parsed_results);
        }
        Ok(parsed_results_array.into())
    }

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
