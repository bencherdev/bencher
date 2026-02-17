#[derive(Debug, thiserror::Error)]
pub enum RunnerCliError {
    #[cfg(feature = "plus")]
    #[error(transparent)]
    Runner(#[from] bencher_runner::RunnerError),

    #[cfg(all(feature = "plus", target_os = "linux"))]
    #[error(transparent)]
    Daemon(#[from] bencher_runner::daemon::DaemonError),

    #[cfg(feature = "plus")]
    #[error(transparent)]
    Valid(#[from] bencher_json::ValidError),

    #[cfg(all(feature = "plus", target_os = "linux"))]
    #[error("Invalid memory size: {0} MiB")]
    InvalidMemory(u64),

    #[cfg(all(feature = "plus", target_os = "linux"))]
    #[error("Invalid disk size: {0} MiB")]
    InvalidDisk(u64),

    #[error("bencher-runner requires Linux with the `plus` feature")]
    Unsupported,
}
