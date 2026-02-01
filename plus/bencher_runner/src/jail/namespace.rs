//! Linux namespace management.

use nix::sched::{unshare, CloneFlags};

use crate::RunnerError;

/// Create new namespaces for isolation.
///
/// Creates: user, mount, network, UTS, IPC namespaces.
/// Does NOT create PID namespace (not needed for VMM, would require fork).
pub fn create_namespaces() -> Result<(), RunnerError> {
    let flags = CloneFlags::CLONE_NEWUSER
        | CloneFlags::CLONE_NEWNS
        | CloneFlags::CLONE_NEWNET
        | CloneFlags::CLONE_NEWUTS
        | CloneFlags::CLONE_NEWIPC;

    unshare(flags).map_err(|e| RunnerError::Jail(format!("unshare failed: {e}")))?;

    Ok(())
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

/// Set up UID/GID mapping for user namespace.
///
/// Maps UID 0 inside to the current user outside.
pub fn setup_uid_gid_mapping() -> Result<(), RunnerError> {
    use std::fs;

    let pid = std::process::id();
    let uid = nix::unistd::getuid();
    let gid = nix::unistd::getgid();

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
