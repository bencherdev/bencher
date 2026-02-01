//! Resource limits (rlimit) management.

use nix::sys::resource::{setrlimit, Resource};

use crate::jail::ResourceLimits;
use crate::RunnerError;

/// Apply rlimits to the current process.
pub fn apply_rlimits(limits: &ResourceLimits) -> Result<(), RunnerError> {
    // Max open file descriptors
    set_limit(Resource::RLIMIT_NOFILE, limits.max_fds)?;

    // Max number of processes
    set_limit(Resource::RLIMIT_NPROC, limits.max_procs)?;

    // Core dump size (disable)
    set_limit(Resource::RLIMIT_CORE, 0)?;

    Ok(())
}

/// Set a single resource limit.
fn set_limit(resource: Resource, limit: u64) -> Result<(), RunnerError> {
    setrlimit(resource, limit, limit)
        .map_err(|e| RunnerError::Jail(format!("setrlimit({resource:?}, {limit}) failed: {e}")))
}
