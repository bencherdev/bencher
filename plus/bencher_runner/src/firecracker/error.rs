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

    /// The API socket address itself cannot be used.
    ///
    /// Distinct from [`Self::SocketNotReady`]: waiting will not help, so the
    /// error names the path and the cause instead of a timeout.
    #[error("Firecracker API socket {path} is unusable: {source}")]
    SocketUnusable {
        /// The socket path the runner tried to reach.
        path: String,
        /// Why it could not be reached.
        source: std::io::Error,
    },

    /// Failed to collect results via vsock.
    #[error("Vsock result collection failed: {0}")]
    VsockCollection(String),

    /// Job was cancelled.
    #[error("Job cancelled")]
    Cancelled,

    /// The VMM could not be placed in, or verified against, its cgroup.
    #[error("Cgroup placement failed: {0}")]
    CgroupPlacement(#[source] crate::error::JailError),

    /// The cgroup exists but the VMM is not in it.
    ///
    /// Fatal: a cgroup that does not contain the VMM is a silent lie about
    /// which cores the benchmark ran on.
    #[error("Firecracker (pid {pid}) is not in its cgroup {cgroup}")]
    CgroupMissingPid {
        /// PID of the Firecracker process.
        pid: u32,
        /// The cgroup it should be in.
        cgroup: camino::Utf8PathBuf,
    },

    /// The cgroup exists but its cpuset could not be applied.
    ///
    /// Boxed because [`crate::error::RunnerError`] contains this type, and it
    /// in turn contains a `RunnerError`.
    #[error("Failed to confine Firecracker to the benchmark cores: {0}")]
    CpusetFailed(#[source] Box<crate::error::RunnerError>),

    /// A jail artifact could not be handed to the jail uid and gid.
    #[error("Jail ownership failed: {0}")]
    Chown(#[source] crate::error::JailError),
}
