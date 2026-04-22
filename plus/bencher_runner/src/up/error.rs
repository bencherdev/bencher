use thiserror::Error;

#[derive(Debug, Error)]
pub enum UpError {
    #[error("API client error: {0}")]
    ApiClient(#[from] ApiClientError),

    #[error("WebSocket error: {0}")]
    WebSocket(#[from] WebSocketError),

    #[error("Runner error: {0}")]
    Runner(#[from] crate::RunnerError),

    #[error("Runner received shutdown signal")]
    Shutdown,

    #[error("Configuration error: {0}")]
    Config(String),
}

#[derive(Debug, Error)]
pub enum ApiClientError {
    #[error("HTTP error: {0}")]
    Http(String),

    #[error("Invalid runner key format")]
    InvalidKey,

    #[error("Unauthorized: invalid or expired key")]
    Unauthorized,
}

#[derive(Debug, Error)]
pub enum WebSocketError {
    #[error("WebSocket connection failed: {0}")]
    Connection(String),

    #[error("WebSocket send failed: {0}")]
    Send(String),

    #[error("WebSocket receive failed: {0}")]
    Receive(String),

    #[error("Unexpected WebSocket message: {0}")]
    UnexpectedMessage(String),

    #[error("Protocol error: {0}")]
    Protocol(String),
}

#[derive(Debug, Error)]
pub enum SelfUpdateError {
    #[error("Failed to determine current binary path: {0}")]
    CurrentExe(std::io::Error),

    #[error("Download request failed: {0}")]
    Http(ureq::Error),

    #[error("Download I/O failed: {0}")]
    Download(std::io::Error),

    #[error("Download too large: {downloaded} bytes exceeds {limit} byte limit")]
    DownloadTooLarge { limit: u64, downloaded: u64 },

    #[error("Failed to parse computed checksum: {0}")]
    ChecksumParse(bencher_valid::ValidError),

    #[error("Checksum mismatch: expected {expected}, got {actual}")]
    Checksum {
        expected: bencher_valid::Sha256,
        actual: bencher_valid::Sha256,
    },

    #[error("File operation failed: {0}")]
    FileOp(std::io::Error),

    #[error("Exec failed: {0}")]
    Exec(std::io::Error),

    #[error("Self-update not supported on this platform")]
    #[cfg(not(unix))]
    UnsupportedPlatform,
}
