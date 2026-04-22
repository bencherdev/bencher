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
}

#[derive(Debug, Error)]
pub enum ApiClientError {
    #[error("Failed to parse URL: {0}")]
    Url(#[from] url::ParseError),

    #[error("Invalid runner key format")]
    InvalidKey,

    #[error("Unauthorized: invalid or expired key")]
    Unauthorized,
}

#[derive(Debug, Error)]
pub enum WebSocketError {
    #[error("Failed to build WebSocket request: {0}")]
    ConnectionHttp(tungstenite::http::Error),

    #[error("WebSocket connection failed: {0}")]
    ConnectionWebSocket(tungstenite::Error),

    #[error("Connection I/O error: {0}")]
    ConnectionIo(std::io::Error),

    #[error("Unsupported TLS stream type")]
    UnsupportedTlsStream,

    #[error("Failed to serialize message: {0}")]
    Serialize(serde_json::Error),

    #[error("WebSocket send failed: {0}")]
    SendWebSocket(tungstenite::Error),

    #[error("WebSocket not connected")]
    NotConnected,

    #[error("WebSocket lock poisoned")]
    LockPoisoned,

    #[error("WebSocket receive failed: {0}")]
    ReceiveWebSocket(tungstenite::Error),

    #[error("Timed out waiting for server message")]
    ReceiveTimeout,

    #[error("Server closed connection: {0:?}")]
    ServerClosed(Option<bencher_json::runner::CloseReason>),

    #[error("Failed to parse server message: {0}")]
    Deserialize(serde_json::Error),

    #[error("Unexpected server message: {0}")]
    UnexpectedServerMessage(String),
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
