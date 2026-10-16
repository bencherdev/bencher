use thiserror::Error;

#[derive(Debug, Error)]
pub enum OciError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Invalid OCI layout: {0}")]
    InvalidLayout(String),

    #[error("Missing manifest: {0}")]
    MissingManifest(String),

    #[error("Missing blob: {0}")]
    MissingBlob(String),

    #[error("Digest mismatch: expected {expected}, got {actual}")]
    DigestMismatch { expected: String, actual: String },

    #[error("Unsupported media type: {0}")]
    UnsupportedMediaType(String),

    #[error("Layer extraction error: {0}")]
    LayerExtraction(String),

    #[error("Registry error: {0}")]
    Registry(String),

    #[cfg(feature = "registry")]
    #[error("Image pull timed out ({0} timeout)")]
    PullTimeout(ureq::Timeout),

    #[error("Invalid image reference: {0}")]
    InvalidReference(String),

    #[error("Path traversal detected: {0}")]
    PathTraversal(String),

    #[error("Unsupported digest algorithm: {0}")]
    UnsupportedDigestAlgorithm(String),

    #[error(
        "The blob store directory {} is not a real directory owned by the store's user and closed to its group and others, so somebody else could have written a blob there, and the store's blobs are trusted when read. Remove it, and the store fills again.",
        .0.display()
    )]
    ForeignBlobStore(std::path::PathBuf),
}
