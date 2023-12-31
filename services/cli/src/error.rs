#[derive(thiserror::Error, Debug)]
pub enum CliError {
    #[error("{0}")]
    Mock(#[from] crate::bencher::sub::MockError),
    #[error("{0}")]
    Run(#[from] crate::bencher::sub::RunError),
    #[error("{0}")]
    Backend(#[from] crate::bencher::BackendError),

    #[error("Failed to create date time from seconds: {0}")]
    DateTime(i64),
    #[error("Invalid statistical sample size: {0}")]
    SampleSize(bencher_json::ValidError),
    #[error("Invalid statistical boundary: {0}")]
    Boundary(bencher_json::ValidError),
    #[error("Failed to serialize config: {0}")]
    SerializeConfig(serde_json::Error),

    #[cfg(feature = "docs")]
    #[error("Failed to create docs: {0}")]
    Docs(std::io::Error),
}
