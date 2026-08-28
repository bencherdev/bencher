use bencher_json::project::report::Adapter;

use crate::{Adaptable as _, AdapterError, Settings};

pub mod adapter_metrics;
pub mod adapter_results;
pub mod foldable;
pub mod results_reducer;

use adapter_results::AdapterResults;
use foldable::FoldableResultsArray;

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
        adapter: Adapter,
        settings: Settings,
    ) -> Result<Self, AdapterError> {
        let mut parsed_results_array = Vec::new();
        for &results in results_array {
            let parsed_results = adapter
                .convert(results, settings)
                .ok_or_else(|| AdapterError::Convert((results).to_owned()))?;
            parsed_results_array.push(parsed_results);
        }
        Ok(parsed_results_array.into())
    }

    /// How many metrics the per measure cap dropped across every payload.
    pub fn dropped_names(&self) -> usize {
        self.inner.iter().map(|results| results.dropped_names).sum()
    }

    /// Every result as a foldable BMF v0 payload, or the array back untouched if
    /// any member is BMF v1.
    ///
    /// Fold is not supported for BMF v1, so a caller handed its array back
    /// ingests unfolded, one iteration per payload.
    pub fn foldable(self) -> Result<FoldableResultsArray, Self> {
        if !self.inner.iter().all(AdapterResults::is_foldable) {
            return Err(self);
        }

        Ok(FoldableResultsArray {
            inner: self
                .inner
                .into_iter()
                .map(AdapterResults::into_foldable)
                .collect(),
        })
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
