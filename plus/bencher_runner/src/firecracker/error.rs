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

    #[error("Failed to write the VM configuration {path}: {source}")]
    WriteVmConfig {
        path: camino::Utf8PathBuf,
        source: std::io::Error,
    },

    /// Timeout waiting for the VM to power off.
    #[error("Timeout: {0}")]
    Timeout(String),

    /// The jailer execs Firecracker in place, so this is either of them, and
    /// either one refusing what it was given.
    #[error(
        "Firecracker exited ({status}) before it booted the VM, so the jailer refused the jail or Firecracker refused the VM's configuration. Its last line on stderr: {}",
        .line.as_deref().unwrap_or("(none)")
    )]
    ExitedBeforeBoot {
        status: std::process::ExitStatus,
        line: Option<String>,
    },

    #[error(
        "Firecracker started {started} of the VM's {vcpus} vCPU threads within {waited:?}, so it has not booted the VM"
    )]
    VcpusNotStarted {
        started: usize,
        vcpus: u8,
        waited: std::time::Duration,
    },

    #[error(
        "A results drive for a --max-output-size of {max_output_size} bytes would be larger than a file can be"
    )]
    ResultsDriveTooLarge { max_output_size: usize },

    #[error("Failed to create the results drive {path}: {source}")]
    CreateResultsDrive {
        path: camino::Utf8PathBuf,
        source: std::io::Error,
    },

    #[error("Failed to wait for the VMM to exit: {0}")]
    WaitVmm(#[source] std::io::Error),

    /// Fatal, since a process left in the jail can still write the drive.
    #[error("The jail could not be emptied before its results were read: {0}")]
    JailNotEmptied(#[source] crate::error::JailError),

    #[error(
        "The guest stopped without results: the VM powered off before the guest wrote them, so the benchmark rebooted or crashed the guest, or the guest's init failed before it could report"
    )]
    NoResults,

    #[error("Failed to read the guest's results: {0}")]
    ReadResults(#[source] bencher_output_protocol::results::ReadError),

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

    /// Fatal, since a VMM outside its cgroup runs unconfined and beyond the kill
    /// that reaps it; see the failure policy table in [`crate::jail`].
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
