//! Jailer error types.

use std::io;

/// Errors that can occur during jail setup or execution.
#[derive(Debug, thiserror::Error)]
pub enum JailerError {
    /// Failed to create namespace.
    #[error("failed to create namespace: {0}")]
    Namespace(String),

    /// Failed to set up cgroup.
    #[error("failed to set up cgroup: {0}")]
    Cgroup(String),

    /// Failed to set up chroot.
    #[error("failed to set up chroot: {0}")]
    Chroot(String),

    /// Failed to drop privileges.
    #[error("failed to drop privileges: {0}")]
    Privileges(String),

    /// Failed to execute the target process.
    #[error("failed to execute target: {0}")]
    Exec(String),

    /// I/O error.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Configuration error.
    #[error("configuration error: {0}")]
    Config(String),

    /// Platform not supported.
    #[error("jailer requires Linux")]
    UnsupportedPlatform,
}
