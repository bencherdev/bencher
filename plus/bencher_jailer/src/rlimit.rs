//! Resource limits (rlimit) management.
//!
//! This module handles setting process resource limits using setrlimit.

use nix::sys::resource::{setrlimit, Resource};

use crate::config::ResourceLimits;
use crate::error::JailerError;

/// Apply rlimits to the current process.
pub fn apply_rlimits(limits: &ResourceLimits) -> Result<(), JailerError> {
    // Max open file descriptors
    set_limit(Resource::RLIMIT_NOFILE, limits.max_fds)?;

    // Max number of processes
    set_limit(Resource::RLIMIT_NPROC, limits.max_procs)?;

    // Max file size
    if let Some(max_size) = limits.max_file_size {
        set_limit(Resource::RLIMIT_FSIZE, max_size)?;
    }

    // Core dump size (disable)
    set_limit(Resource::RLIMIT_CORE, 0)?;

    Ok(())
}

/// Set a single resource limit.
fn set_limit(resource: Resource, limit: u64) -> Result<(), JailerError> {
    setrlimit(resource, limit, limit).map_err(|e| {
        JailerError::Privileges(format!("setrlimit({resource:?}, {limit}) failed: {e}"))
    })
}
