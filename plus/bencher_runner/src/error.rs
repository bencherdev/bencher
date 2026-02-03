use thiserror::Error;

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("OCI error: {0}")]
    Oci(#[from] bencher_oci::OciError),

    #[error("Rootfs error: {0}")]
    Rootfs(#[from] bencher_rootfs::RootfsError),

    #[cfg(target_os = "linux")]
    #[error("Firecracker error: {0}")]
    Firecracker(#[from] crate::firecracker::FirecrackerError),

    #[error("Jail error: {0}")]
    Jail(String),

    #[error("Config error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
