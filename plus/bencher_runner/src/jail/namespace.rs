//! Linux namespace management.

use nix::sched::{unshare, CloneFlags};

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
/// Creates: mount, network, UTS, IPC namespaces.
/// Does NOT create PID namespace (not needed for VMM, would require fork).
/// Must be called AFTER setup_uid_gid_mapping().
pub fn create_other_namespaces() -> Result<(), RunnerError> {
    let flags = CloneFlags::CLONE_NEWNS
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
