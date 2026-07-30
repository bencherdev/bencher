//! Firecracker error types.

use thiserror::Error;

/// Errors from the Firecracker integration.
#[derive(Debug, Error)]
pub enum FirecrackerError {
    /// Failed to spawn the jailer that starts the Firecracker process.
    #[error("Failed to spawn {path}: {source}")]
    Spawn {
        /// The binary that could not be spawned.
        path: camino::Utf8PathBuf,
        /// Why it could not be spawned.
        source: std::io::Error,
    },

    /// The spawned process did not provide the stdio the runner asked for.
    #[error("Firecracker process stdio unavailable: {0}")]
    Stdio(&'static str),

    /// A Firecracker API request or response could not be handled.
    #[error("Firecracker API {context}: {source}")]
    ApiEncoding {
        /// What was being encoded or decoded.
        context: &'static str,
        /// The underlying serialization failure.
        source: serde_json::Error,
    },

    /// The Firecracker API returned a response that could not be parsed.
    #[error("Firecracker API response malformed: {0}")]
    MalformedResponse(&'static str),

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

    /// The jailed process exited before Firecracker started serving its API.
    ///
    /// Named for what the runner actually knows. The jailer `exec`s Firecracker
    /// in place, so this one pid is the jailer up to that moment and Firecracker
    /// afterwards, and the runner cannot tell from the outside which of the two
    /// died. Blaming the jailer by name was right about half the time.
    ///
    /// Distinct from a timeout: the process is gone, so waiting cannot help,
    /// and the reason is on stderr under the `[firecracker]` prefix.
    #[error(
        "The jailed process exited ({status}) before the Firecracker API socket appeared. The jailer execs Firecracker in place, so this is the jailer or the VMM it became; its diagnostics are above, prefixed [firecracker]"
    )]
    JailedProcessExited {
        /// How it exited.
        status: std::process::ExitStatus,
    },

    /// The API socket address itself cannot be used.
    ///
    /// Distinct from [`Self::SocketNotReady`]: waiting will not help, so the
    /// error names the path and the cause instead of a timeout.
    #[error("Firecracker API socket {path} is unusable: {source}")]
    SocketUnusable {
        /// The socket path the runner tried to reach.
        ///
        /// The checked type, not a string: every value has been measured
        /// against `sun_path`, which is the limit this error is most often
        /// about.
        path: crate::jail::SocketPath,
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
