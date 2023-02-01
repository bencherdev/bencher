use thiserror::Error;

#[derive(Error, Debug)]
pub enum AdapterError {
    #[error("Failed to validate: {0}")]
    Valid(#[from] bencher_json::ValidError),
    #[error("Failed to parse benchmark units: {0}")]
    BenchmarkUnits(String),
    #[error("Failed to convert results: {0}")]
    Convert(String),
}
