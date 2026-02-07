//! Firecracker error types.

use thiserror::Error;

/// Errors from the Firecracker integration.
#[derive(Debug, Error)]
pub enum FirecrackerError {
    /// Failed to start the Firecracker process.
    #[error("Failed to start Firecracker process: {0}")]
    ProcessStart(String),

    /// Firecracker API returned an error.
    #[error("Firecracker API error: {status} {body}")]
    Api {
        /// HTTP status code.
        status: u16,
        /// Response body.
        body: String,
    },

    /// Timeout waiting for Firecracker to be ready or VM to complete.
    #[error("Timeout: {0}")]
    Timeout(String),

    /// I/O error communicating with Firecracker.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Firecracker API socket not ready.
    #[error("Firecracker API socket not ready after {0:?}")]
    SocketNotReady(std::time::Duration),

    /// Failed to collect results via vsock.
    #[error("Vsock result collection failed: {0}")]
    VsockCollection(String),

    /// Job was cancelled.
    #[error("Job cancelled")]
    Cancelled,
}
