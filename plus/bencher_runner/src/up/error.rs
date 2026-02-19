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

    #[error("Invalid runner token format")]
    InvalidToken,

    #[error("Unauthorized: invalid or expired token")]
    Unauthorized,

    #[error("Runner is locked by another session")]
    RunnerLocked,

    #[error("Unexpected HTTP status {status}: {body}")]
    UnexpectedStatus { status: u16, body: String },
}

#[derive(Debug, Error)]
pub enum WebSocketError {
    #[error("WebSocket connection failed: {0}")]
    Connection(String),

    #[error("WebSocket send failed: {0}")]
    Send(String),

    #[error("WebSocket receive failed: {0}")]
    Receive(String),

    #[error("Job was canceled by server")]
    Canceled,

    #[error("Unexpected WebSocket message: {0}")]
    UnexpectedMessage(String),
}
