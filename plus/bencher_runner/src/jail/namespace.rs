//! Linux namespace management.

use nix::sched::{unshare, CloneFlags};
use nix::sys::wait::{waitpid, WaitStatus};
use nix::unistd::{fork, ForkResult};

use crate::RunnerError;

/// Create new user namespace for isolation.
///
/// This must be called BEFORE setup_uid_gid_mapping(), and the UID/GID mapping
/// must be set up BEFORE creating other namespaces (mount, network, etc.).
pub fn create_user_namespace() -> Result<(), RunnerError> {
    unshare(CloneFlags::CLONE_NEWUSER)
        .map_err(|e| RunnerError::Jail(format!("unshare user namespace failed: {e}")))?;
    Ok(())
}

/// Create remaining namespaces after UID/GID mapping is set up.
///
/// Creates: mount, network, UTS, IPC, and PID namespaces.
/// PID namespace requires `fork()` after this call — the child process
/// enters the new PID namespace as PID 1.
/// Must be called AFTER setup_uid_gid_mapping().
pub fn create_other_namespaces() -> Result<(), RunnerError> {
    let flags = CloneFlags::CLONE_NEWNS
        | CloneFlags::CLONE_NEWNET
        | CloneFlags::CLONE_NEWUTS
        | CloneFlags::CLONE_NEWIPC
        | CloneFlags::CLONE_NEWPID;

    unshare(flags).map_err(|e| RunnerError::Jail(format!("unshare failed: {e}")))?;

    Ok(())
}

/// Fork into the PID namespace.
///
/// After `unshare(CLONE_NEWPID)`, the calling process is NOT in the new
/// PID namespace — only its children are. This function forks and returns:
/// - `Ok(None)` in the child (which is PID 1 in the new namespace)
/// - `Ok(Some(exit_code))` in the parent after the child exits
///
/// The child should perform the remaining jail setup and VMM execution.
/// The parent should propagate the child's exit code.
///
/// # Safety
///
/// This must be called while the process is single-threaded. No threads
/// should have been spawned yet (the VMM creates threads later).
pub fn fork_into_pid_namespace() -> Result<Option<i32>, RunnerError> {
    // SAFETY: fork() is safe here because we are single-threaded at this point.
    // No threads have been spawned yet — threads are only created later by the VMM.
    match unsafe { fork() } {
        Ok(ForkResult::Parent { child }) => {
            // Parent: wait for the child to exit
            loop {
                match waitpid(child, None) {
                    Ok(WaitStatus::Exited(_, code)) => return Ok(Some(code)),
                    Ok(WaitStatus::Signaled(_, sig, _)) => {
                        return Ok(Some(128 + sig as i32));
                    }
                    Ok(_) => continue, // Other status (stopped, continued), keep waiting
                    Err(nix::errno::Errno::EINTR) => continue,
                    Err(e) => {
                        return Err(RunnerError::Jail(format!("waitpid failed: {e}")));
                    }
                }
            }
        }
        Ok(ForkResult::Child) => {
            // Child: now PID 1 in the new namespace
            Ok(None)
        }
        Err(e) => Err(RunnerError::Jail(format!("fork failed: {e}"))),
    }
}

/// Set `PR_SET_NO_NEW_PRIVS` to prevent privilege escalation.
pub fn set_no_new_privs() -> Result<(), RunnerError> {
    // SAFETY: prctl with PR_SET_NO_NEW_PRIVS is safe and takes no pointer arguments
    let ret = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
    if ret != 0 {
        return Err(RunnerError::Jail(format!(
            "PR_SET_NO_NEW_PRIVS failed: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(())
}

/// Capture the current UID and GID before entering a new user namespace.
///
/// Must be called BEFORE `create_user_namespace()` because `getuid()`/`getgid()`
/// return the overflow UID/GID (65534) after `unshare(CLONE_NEWUSER)` until the
/// mapping is written.
pub fn get_uid_gid() -> (nix::unistd::Uid, nix::unistd::Gid) {
    (nix::unistd::getuid(), nix::unistd::getgid())
}

/// Set up UID/GID mapping for user namespace.
///
/// Maps UID 0 inside to the given UID/GID outside (captured before `unshare`).
pub fn setup_uid_gid_mapping(
    uid: nix::unistd::Uid,
    gid: nix::unistd::Gid,
) -> Result<(), RunnerError> {
    use std::fs;

    let pid = std::process::id();

    // Map root inside to current user outside
    let uid_map = format!("0 {uid} 1\n");
    fs::write(format!("/proc/{pid}/uid_map"), &uid_map)
        .map_err(|e| RunnerError::Jail(format!("failed to write uid_map: {e}")))?;

    // Disable setgroups (required before writing gid_map)
    fs::write(format!("/proc/{pid}/setgroups"), "deny\n")
        .map_err(|e| RunnerError::Jail(format!("failed to write setgroups: {e}")))?;

    // Map root inside to current group outside
    let gid_map = format!("0 {gid} 1\n");
    fs::write(format!("/proc/{pid}/gid_map"), &gid_map)
        .map_err(|e| RunnerError::Jail(format!("failed to write gid_map: {e}")))?;

    Ok(())
}

/// Drop all Linux capabilities.
pub fn drop_capabilities() -> Result<(), RunnerError> {
    use caps::{clear, CapSet};

    clear(None, CapSet::Effective)
        .map_err(|e| RunnerError::Jail(format!("failed to clear effective caps: {e}")))?;
    clear(None, CapSet::Permitted)
        .map_err(|e| RunnerError::Jail(format!("failed to clear permitted caps: {e}")))?;
    clear(None, CapSet::Inheritable)
        .map_err(|e| RunnerError::Jail(format!("failed to clear inheritable caps: {e}")))?;
    clear(None, CapSet::Ambient)
        .map_err(|e| RunnerError::Jail(format!("failed to clear ambient caps: {e}")))?;

    Ok(())
}
