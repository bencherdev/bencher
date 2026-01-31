use thiserror::Error;

#[derive(Debug, Error)]
pub enum RootfsError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("OCI error: {0}")]
    Oci(#[from] bencher_oci::OciError),

    #[error("Squashfs error: {0}")]
    Squashfs(String),

    #[error("Path error: {0}")]
    Path(String),
}

impl From<backhand::BackhandError> for RootfsError {
    fn from(err: backhand::BackhandError) -> Self {
        Self::Squashfs(err.to_string())
    }
}
