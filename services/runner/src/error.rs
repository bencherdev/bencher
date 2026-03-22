#[derive(Debug, thiserror::Error)]
pub enum RunnerCliError {
    #[cfg(feature = "plus")]
    #[error(transparent)]
    Runner(#[from] bencher_runner::RunnerError),

    #[cfg(feature = "plus")]
    #[error(transparent)]
    Up(#[from] bencher_runner::up::UpError),

    #[cfg(feature = "plus")]
    #[error(transparent)]
    Valid(#[from] bencher_json::ValidError),

    #[cfg(feature = "plus")]
    #[error("Invalid memory size: {0} MiB")]
    InvalidMemory(u64),

    #[cfg(feature = "plus")]
    #[error("Invalid disk size: {0} MiB")]
    InvalidDisk(u64),

    #[cfg(not(feature = "plus"))]
    #[error("Runner requires the `plus` feature")]
    NoPlusFeature,
}
