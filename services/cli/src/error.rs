#[derive(thiserror::Error, Debug)]
pub enum CliError {
    #[error("{0}")]
    Backend(#[from] crate::bencher::BackendError),
    #[error("{0}")]
    Run(#[from] crate::bencher::sub::RunError),
    #[error("{0}")]
    Mock(#[from] crate::bencher::sub::MockError),
    #[error("{0}")]
    Docker(#[from] crate::bencher::sub::DockerError),

    #[error("Invalid threshold statistic: {0}")]
    Statistic(bencher_boundary::BoundaryError),
    #[error("Failed to serialize config: {0}")]
    SerializeConfig(serde_json::Error),
}
