use thiserror::Error;

#[derive(Debug, Error)]
pub enum RunnerError {
    #[error("OCI error: {0}")]
    Oci(#[from] bencher_oci::OciError),

    #[error("Rootfs error: {0}")]
    Rootfs(#[from] bencher_rootfs::RootfsError),

    #[error("VMM error: {0}")]
    Vmm(#[from] bencher_vmm::VmmError),

    #[error("Config error: {0}")]
    Config(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
}
