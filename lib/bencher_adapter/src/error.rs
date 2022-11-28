use thiserror::Error;

#[derive(Error, Debug)]
pub enum AdapterError {
    #[error("Failed to (de)serialize JSON: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("Failed to parse: {0}")]
    Nom(#[from] nom::Err<nom::error::Error<String>>),

    #[error("Benchmark failed: {0}")]
    BenchmarkFailed(String),
    #[error("Benchmark thread {thread} panicked at {context}: {location}")]
    Panic {
        thread: String,
        context: String,
        location: String,
    },
}
