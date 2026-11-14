use bencher_json::{BmfVersion, project::report::Adapter};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum AdapterError {
    #[error("Failed to validate: {0}")]
    Valid(#[from] bencher_json::ValidError),
    #[error("Failed to parse benchmark units: {0}")]
    BenchmarkUnits(String),
    #[error("Failed to convert results: {0}")]
    Convert(String),
    #[error("Results parsed as BMF version {parsed} but were expected at version {expected}")]
    BmfVersion {
        expected: BmfVersion,
        parsed: BmfVersion,
    },
    #[error("{0} reads results at BMF version 1 only")]
    V1Only(Adapter),
}
