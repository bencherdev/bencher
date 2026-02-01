//! Linux namespace management.
//!
//! This module handles creating and configuring Linux namespaces for process isolation.

use nix::sched::{unshare, CloneFlags};
use nix::unistd::{setgid, setuid, Gid, Uid};

use crate::config::NamespaceConfig;
use crate::error::JailerError;

/// Create new namespaces based on the configuration.
///
/// This should be called before fork/clone to set up the namespace isolation.
pub fn create_namespaces(config: &NamespaceConfig) -> Result<(), JailerError> {
    let mut flags = CloneFlags::empty();

    if config.user {
        flags |= CloneFlags::CLONE_NEWUSER;
    }
    if config.pid {
        flags |= CloneFlags::CLONE_NEWPID;
    }
    if config.mount {
        flags |= CloneFlags::CLONE_NEWNS;
    }
    if config.network {
        flags |= CloneFlags::CLONE_NEWNET;
    }
    if config.uts {
        flags |= CloneFlags::CLONE_NEWUTS;
    }
    if config.ipc {
        flags |= CloneFlags::CLONE_NEWIPC;
    }
    if config.cgroup {
        flags |= CloneFlags::CLONE_NEWCGROUP;
    }

    if !flags.is_empty() {
        unshare(flags).map_err(|e| JailerError::Namespace(format!("unshare failed: {e}")))?;
    }

    Ok(())
}

/// Set up UID/GID mapping for user namespace.
///
/// This maps the specified UID/GID inside the namespace to the current user outside.
pub fn setup_uid_gid_mapping(uid: u32, gid: u32) -> Result<(), JailerError> {
    use std::fs;

    let pid = std::process::id();

    // Write uid_map: map uid inside to current uid outside
    let uid_map = format!("{uid} {} 1\n", nix::unistd::getuid());
    fs::write(format!("/proc/{pid}/uid_map"), &uid_map)
        .map_err(|e| JailerError::Namespace(format!("failed to write uid_map: {e}")))?;

    // Disable setgroups (required before writing gid_map in unprivileged user ns)
    fs::write(format!("/proc/{pid}/setgroups"), "deny\n")
        .map_err(|e| JailerError::Namespace(format!("failed to write setgroups: {e}")))?;

    // Write gid_map: map gid inside to current gid outside
    let gid_map = format!("{gid} {} 1\n", nix::unistd::getgid());
    fs::write(format!("/proc/{pid}/gid_map"), &gid_map)
        .map_err(|e| JailerError::Namespace(format!("failed to write gid_map: {e}")))?;

    Ok(())
}

/// Drop to the specified UID/GID.
///
/// This should be called after all privileged setup is complete.
pub fn drop_to_uid_gid(uid: u32, gid: u32) -> Result<(), JailerError> {
    // Set GID first (we need privileges to do this)
    setgid(Gid::from_raw(gid))
        .map_err(|e| JailerError::Privileges(format!("setgid failed: {e}")))?;

    // Then set UID (this drops privileges)
    setuid(Uid::from_raw(uid))
        .map_err(|e| JailerError::Privileges(format!("setuid failed: {e}")))?;

    Ok(())
}

/// Set PR_SET_NO_NEW_PRIVS to prevent privilege escalation.
pub fn set_no_new_privs() -> Result<(), JailerError> {
    // SAFETY: prctl with PR_SET_NO_NEW_PRIVS is safe and takes no pointer arguments
    let ret = unsafe { libc::prctl(libc::PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0) };
    if ret != 0 {
        return Err(JailerError::Privileges(format!(
            "PR_SET_NO_NEW_PRIVS failed: {}",
            std::io::Error::last_os_error()
        )));
    }
    Ok(())
}

/// Set hostname inside UTS namespace.
pub fn set_hostname(hostname: &str) -> Result<(), JailerError> {
    nix::unistd::sethostname(hostname)
        .map_err(|e| JailerError::Namespace(format!("sethostname failed: {e}")))?;
    Ok(())
}
