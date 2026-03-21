#[derive(Debug, thiserror::Error)]
pub enum RunnerCliError {
    #[cfg(feature = "plus")]
    #[error(transparent)]
    Runner(#[from] bencher_runner::RunnerError),

    #[cfg(all(feature = "plus", any(target_os = "linux", debug_assertions)))]
    #[error(transparent)]
    Up(#[from] bencher_runner::up::UpError),

    #[cfg(feature = "plus")]
    #[error(transparent)]
    Valid(#[from] bencher_json::ValidError),

    #[cfg(all(feature = "plus", any(target_os = "linux", debug_assertions)))]
    #[error("Invalid memory size: {0} MiB")]
    InvalidMemory(u64),

    #[cfg(all(feature = "plus", any(target_os = "linux", debug_assertions)))]
    #[error("Invalid disk size: {0} MiB")]
    InvalidDisk(u64),

    #[cfg(not(all(feature = "plus", any(target_os = "linux", debug_assertions))))]
    #[error("bencher-runner requires Linux with the `plus` feature")]
    Unsupported,
}
