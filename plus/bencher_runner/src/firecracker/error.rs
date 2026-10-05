//! Firecracker error types.

use thiserror::Error;

/// What the forked child did before the exec, since a failure there surfaces
/// only as a failed spawn.
#[derive(Debug, Clone, Copy)]
pub enum PreExec {
    Nothing,
    CgroupPlacement,
}

impl std::fmt::Display for PreExec {
    /// Appended to a spawn failure, so the empty case has to print nothing.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Nothing => Ok(()),
            Self::CgroupPlacement => f.write_str(
                ". Cgroup placement runs in the forked child before the exec and reports only its errno, so this may be the write to cgroup.procs rather than the binary",
            ),
        }
    }
}

/// Errors from the Firecracker integration.
#[derive(Debug, Error)]
pub enum FirecrackerError {
    /// Also covers a failed `pre_exec` step, whose errno is all that crosses back.
    #[error("Failed to spawn {path}: {source}{pre_exec}")]
    Spawn {
        path: camino::Utf8PathBuf,
        pre_exec: PreExec,
        source: std::io::Error,
    },

    #[error("Firecracker API {context}: {source}")]
    ApiEncoding {
        context: &'static str,
        source: serde_json::Error,
    },

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

    #[error(
        "The jailed process exited ({status}) before the Firecracker API socket appeared. The jailer execs Firecracker in place, so this is the jailer or the VMM it became; its diagnostics are above, prefixed [firecracker]"
    )]
    JailedProcessExited { status: std::process::ExitStatus },

    /// Unlike [`Self::SocketNotReady`], waiting will not help.
    #[error("Firecracker API socket {path} is unusable: {source}")]
    SocketUnusable {
        /// The checked type, since an over-long `sun_path` is the usual cause.
        path: crate::jail::SocketPath,
        source: std::io::Error,
    },

    /// A path already taken lands here, since nothing is unlinked first.
    #[error("Failed to bind the vsock {stream} listener (port {port}): {source}")]
    BindVsock {
        stream: &'static str,
        port: u32,
        source: std::io::Error,
    },

    #[error("Failed to set the vsock {stream} listener (port {port}) non-blocking: {source}")]
    VsockNonblocking {
        stream: &'static str,
        port: u32,
        source: std::io::Error,
    },

    #[error("Failed to poll the vsock listeners: {source}")]
    PollVsock { source: nix::errno::Errno },

    #[error("Failed to decode the output files the guest sent: {source}")]
    DecodeOutputFiles {
        source: bencher_output_protocol::DecodeError,
    },

    /// Job was cancelled.
    #[error("Job cancelled")]
    Cancelled,

    /// The VMM could not be placed in, or verified against, its cgroup.
    #[error("Cgroup placement failed: {0}")]
    CgroupPlacement(#[source] crate::error::JailError),

    /// Fatal, since a VMM outside its cgroup runs unconfined and unseen by the
    /// occupancy check; see the failure policy table in [`crate::jail`].
    /// Boxed because [`crate::error::RunnerError`] in turn contains this type.
    #[error("Failed to create the cgroup the VMM runs in: {0}")]
    Cgroup(#[source] Box<crate::error::RunnerError>),

    /// Fatal, since a cgroup without the VMM is a silent lie about which cores
    /// the benchmark ran on.
    #[error("Firecracker (pid {pid}) is not in its cgroup {cgroup}")]
    CgroupMissingPid {
        pid: u32,
        cgroup: camino::Utf8PathBuf,
    },

    /// Boxed because [`crate::error::RunnerError`] in turn contains this type.
    #[error("Failed to confine Firecracker to the benchmark cores: {0}")]
    CpusetFailed(#[source] Box<crate::error::RunnerError>),

    #[error("Failed to pin the Firecracker API socket: {0}")]
    PinApiSocket(#[source] crate::error::JailError),

    #[error(transparent)]
    CoresOccupied(crate::error::JailError),

    #[error("Jail ownership failed: {0}")]
    Chown(#[source] crate::error::JailError),
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a `cgroup.procs` write refused by a partition constraint returns.
    const EBUSY: i32 = 16;

    const JAILER: &str = "/var/lib/bencher-runner/work/jailer";

    fn spawn_failure(pre_exec: PreExec) -> FirecrackerError {
        FirecrackerError::Spawn {
            path: camino::Utf8PathBuf::from(JAILER),
            pre_exec,
            source: std::io::Error::from_raw_os_error(EBUSY),
        }
    }

    #[test]
    fn a_spawn_that_placed_the_vmm_first_says_so() {
        // Prevents a refused `cgroup.procs` write reading as a fault in the jailer binary alone.
        let message = spawn_failure(PreExec::CgroupPlacement).to_string();

        assert!(message.contains(JAILER), "{message}");
        assert!(
            message.contains("cgroup.procs"),
            "a spawn that ran cgroup placement must name it: {message}"
        );
    }

    #[test]
    fn a_spawn_with_nothing_before_the_exec_blames_only_the_binary() {
        // Prevents blaming a cgroup placement that never ran.
        let message = spawn_failure(PreExec::Nothing).to_string();

        assert!(message.contains(JAILER), "{message}");
        assert!(!message.contains("cgroup"), "{message}");
    }
}
