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
