use bencher_json::{BmfVersion, project::report::Adapter};

use crate::{
    Adaptable as _, AdapterError, Settings,
    adapters::json::{v0::AdapterJsonV0, v1::AdapterJsonV1},
};

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
                .ok_or_else(|| Self::refusal(results, adapter, settings.bmf_version))?;
            // The declared version is a contract over the whole payload.
            if parsed_results.version != settings.bmf_version {
                return Err(AdapterError::BmfVersion {
                    declared: settings.bmf_version,
                    parsed: parsed_results.version,
                });
            }
            parsed_results_array.push(parsed_results);
        }
        Ok(parsed_results_array.into())
    }

    /// The error for a payload the adapter did not claim, where under `json` or
    /// `magic` the other version's leaf is probed only to name the version and
    /// nothing ingests through it.
    fn refusal(results: &str, adapter: Adapter, declared: BmfVersion) -> AdapterError {
        let probe = if !matches!(adapter, Adapter::Json | Adapter::Magic) {
            None
        } else if declared == BmfVersion::V0 {
            AdapterJsonV1::parse(results, Settings::default())
        } else {
            AdapterJsonV0::parse(results, Settings::default())
        };
        match probe {
            Some(other) => AdapterError::BmfVersion {
                declared,
                parsed: other.version,
            },
            None => AdapterError::Convert(results.to_owned()),
        }
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

#[cfg(test)]
mod test_results_array {
    use bencher_json::{BmfVersion, project::report::Adapter};

    use super::AdapterResultsArray;
    use crate::{AdapterError, Settings};

    const V0: &str = r#"{"bench": {"latency": {"value": 10.0}}}"#;
    const V1: &str = r#"{"bench": [{"measures": {"latency": {"value": 10.0}}}]}"#;

    fn new_at(results: &str, adapter: Adapter, bmf_version: BmfVersion) -> AdapterError {
        AdapterResultsArray::new(&[results], adapter, Settings::new(None, bmf_version))
            .expect_err("expected the results array to refuse the payload")
    }

    fn assert_contract(error: &AdapterError, declared: BmfVersion, parsed: BmfVersion) {
        match error {
            AdapterError::BmfVersion {
                declared: error_declared,
                parsed: error_parsed,
            } => {
                assert_eq!(*error_declared, declared);
                assert_eq!(*error_parsed, parsed);
            },
            AdapterError::Valid(_) | AdapterError::BenchmarkUnits(_) | AdapterError::Convert(_) => {
                panic!("expected the BMF version error: {error}")
            },
        }
    }

    /// The `json` node parses no leaf but the declared one, and the refusal still
    /// names the version the payload is written in.
    #[test]
    fn results_array_json_refuses_the_other_version_by_name() {
        for adapter in [Adapter::Json, Adapter::Magic] {
            let error = new_at(V0, adapter, BmfVersion::V1);
            assert_contract(&error, BmfVersion::V1, BmfVersion::V0);
            let error = new_at(V1, adapter, BmfVersion::V0);
            assert_contract(&error, BmfVersion::V0, BmfVersion::V1);
        }
    }

    /// A payload no leaf claims fails to convert at either version.
    #[test]
    fn results_array_json_unclaimed_payload_fails_to_convert() {
        let results = std::fs::read_to_string("./tool_output/json/report_mixed_versions.json")
            .expect("Failed to read test file");
        for adapter in [Adapter::Json, Adapter::Magic] {
            for bmf_version in [BmfVersion::V0, BmfVersion::V1] {
                let error = new_at(&results, adapter, bmf_version);
                assert!(
                    matches!(error, AdapterError::Convert(_)),
                    "expected the convert error at version {bmf_version}: {error}"
                );
            }
        }
    }

    /// The `json_v1` leaf parses a v1 payload whatever was declared, so declaring
    /// version 0 breaks the contract.
    #[test]
    fn results_array_json_v1_refuses_version_0() {
        let error = new_at(V1, Adapter::JsonV1, BmfVersion::V0);
        assert_contract(&error, BmfVersion::V0, BmfVersion::V1);
    }

    /// The `json_v0` leaf parses a v0 payload whatever was declared, so declaring
    /// version 1 breaks the contract.
    #[test]
    fn results_array_json_v0_refuses_version_1() {
        let error = new_at(V0, Adapter::JsonV0, BmfVersion::V1);
        assert_contract(&error, BmfVersion::V1, BmfVersion::V0);
    }

    /// A non-BMF adapter parses as version 0, so its output at version 1 breaks
    /// the contract.
    #[test]
    fn results_array_non_bmf_adapter_refuses_version_1() {
        let results = std::fs::read_to_string("./tool_output/rust/bench/many.txt")
            .expect("Failed to read test file");
        let error = new_at(&results, Adapter::RustBench, BmfVersion::V1);
        assert_contract(&error, BmfVersion::V1, BmfVersion::V0);
        AdapterResultsArray::new(
            &[&results],
            Adapter::RustBench,
            Settings::new(None, BmfVersion::V0),
        )
        .expect("a non-BMF adapter parses at version 0");
    }

    /// The contract error names both versions, since that is what a client reads.
    #[test]
    fn results_array_contract_error_names_both_versions() {
        let error = new_at(V1, Adapter::JsonV1, BmfVersion::V0);
        assert_eq!(
            error.to_string(),
            "Results parsed as BMF version 1 but the payload declared version 0"
        );
    }
}
