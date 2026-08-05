//! Firecracker error types.

use thiserror::Error;

/// What the runner asks the forked child to do before it execs the jailer.
///
/// Carried by [`FirecrackerError::Spawn`] and printed as part of it: a step
/// that fails between the fork and the exec is reported as a failed spawn of
/// the binary that never ran, and this is the only thing the runner still knows
/// about the real cause.
#[derive(Debug, Clone, Copy)]
pub enum PreExec {
    /// Nothing. The child execs the moment it is forked.
    Nothing,
    /// The child writes itself into the VMM's cgroup.
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
    /// Failed to spawn the jailer that starts the Firecracker process.
    ///
    /// Everything the runner asks the forked child to do before it execs fails
    /// as a failed spawn too, and only the errno crosses back over the CLOEXEC
    /// pipe, so the runner cannot tell those apart from a binary that could not
    /// be executed. The error names what ran instead of leaving the operator to
    /// debug the only thing it mentions.
    #[error("Failed to spawn {path}: {source}{pre_exec}")]
    Spawn {
        /// The binary that could not be spawned.
        path: camino::Utf8PathBuf,
        /// What ran in the forked child first, and may be the real cause.
        pre_exec: PreExec,
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

    /// A vsock listener socket could not be bound.
    ///
    /// Nothing is unlinked before the bind, so a path already taken lands
    /// here. The stream name is what an operator recognizes; the port is what
    /// the guest side and the socket file on disk are named by.
    #[error("Failed to bind the vsock {stream} listener (port {port}): {source}")]
    BindVsock {
        /// The stream the listener carries.
        stream: &'static str,
        /// The vsock port the guest connects to.
        port: u32,
        /// Why it could not be bound.
        source: std::io::Error,
    },

    /// A vsock listener could not be made non-blocking.
    ///
    /// The collection loop polls all four listeners at once, so a listener
    /// left blocking would stall it on whichever port the guest writes last.
    #[error("Failed to set the vsock {stream} listener (port {port}) non-blocking: {source}")]
    VsockNonblocking {
        /// The stream the listener carries.
        stream: &'static str,
        /// The vsock port the guest connects to.
        port: u32,
        /// Why it could not be set.
        source: std::io::Error,
    },

    /// The poll across the vsock listeners failed.
    #[error("Failed to poll the vsock listeners: {source}")]
    PollVsock {
        /// Why the poll failed.
        source: nix::errno::Errno,
    },

    /// The output files the guest sent could not be decoded.
    #[error("Failed to decode the output files the guest sent: {source}")]
    DecodeOutputFiles {
        /// Why the decoding failed.
        source: bencher_output_protocol::DecodeError,
    },

    /// Job was cancelled.
    #[error("Job cancelled")]
    Cancelled,

    /// The VMM could not be placed in, or verified against, its cgroup.
    #[error("Cgroup placement failed: {0}")]
    CgroupPlacement(#[source] crate::error::JailError),

    /// A cgroup this run could not be given, for a reason nobody established.
    ///
    /// Distinct from a host that does not delegate the controllers, which the
    /// job degrades on: that absence was declared, and a declared absence of
    /// isolation is a limitation an operator can read. A cgroup that could not
    /// be read establishes nothing, so degrading on it would produce benchmark
    /// numbers with no core confinement and report a host limitation nobody
    /// observed. See the failure policy table in [`crate::jail`].
    #[error("The cgroup for CPU isolation could not be read: {0}")]
    CgroupUnreadable(#[source] crate::error::JailError),

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

#[cfg(test)]
mod tests {
    use super::*;

    /// What a `cgroup.procs` write refused by a partition constraint returns.
    const EBUSY: i32 = 16;

    /// The jailer path an operator would read in the message.
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
        // The failure this prevents: the `pre_exec` write to `cgroup.procs` is
        // refused, only the errno crosses the CLOEXEC pipe, and the operator
        // reads "Failed to spawn .../jailer: Device or resource busy" and goes
        // looking at the binary. The misattribution is inherent; saying which
        // step ran is not.
        let message = spawn_failure(PreExec::CgroupPlacement).to_string();

        assert!(message.contains(JAILER), "{message}");
        assert!(
            message.contains("cgroup.procs"),
            "a spawn that ran cgroup placement must name it: {message}"
        );
    }

    #[test]
    fn a_spawn_with_nothing_before_the_exec_blames_only_the_binary() {
        // The other half: with no cgroup there is no placement to run, so
        // pointing at one would send the operator after a step that never
        // happened.
        let message = spawn_failure(PreExec::Nothing).to_string();

        assert!(message.contains(JAILER), "{message}");
        assert!(!message.contains("cgroup"), "{message}");
    }
}
