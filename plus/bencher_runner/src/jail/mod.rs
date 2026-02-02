//! Jail isolation for VMM processes.
//!
//! This module provides Linux namespace and filesystem isolation
//! for running VMM processes securely.

#[cfg(target_os = "linux")]
mod cgroup;
#[cfg(target_os = "linux")]
mod chroot;
#[cfg(target_os = "linux")]
mod namespace;
#[cfg(target_os = "linux")]
mod rlimit;

#[cfg(target_os = "linux")]
pub use cgroup::CgroupManager;
#[cfg(target_os = "linux")]
pub use chroot::{create_jail_root, do_pivot_root, mount_essential_filesystems};
#[cfg(target_os = "linux")]
pub use namespace::{
    create_other_namespaces, create_user_namespace, drop_capabilities, set_no_new_privs,
    setup_uid_gid_mapping,
};
#[cfg(target_os = "linux")]
pub use rlimit::apply_rlimits;

use serde::{Deserialize, Serialize};

/// Resource limits for the jailed VMM process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum CPU time in microseconds per second.
    /// E.g., 100000 = 100ms per 100ms period = 1 full CPU.
    #[serde(default)]
    pub cpu_quota_us: Option<u64>,

    /// CPU period in microseconds (default: 100000 = 100ms).
    #[serde(default = "default_cpu_period")]
    pub cpu_period_us: u64,

    /// Maximum memory in bytes.
    #[serde(default)]
    pub memory_bytes: Option<u64>,

    /// Maximum number of open file descriptors.
    #[serde(default = "default_max_fds")]
    pub max_fds: u64,

    /// Maximum number of processes/threads.
    #[serde(default = "default_max_procs")]
    pub max_procs: u64,
}

const fn default_cpu_period() -> u64 {
    100_000 // 100ms
}

const fn default_max_fds() -> u64 {
    1024
}

const fn default_max_procs() -> u64 {
    64
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            cpu_quota_us: None,
            cpu_period_us: default_cpu_period(),
            memory_bytes: None,
            max_fds: default_max_fds(),
            max_procs: default_max_procs(),
        }
    }
}

impl ResourceLimits {
    /// Set CPU limit as a fraction of CPUs (e.g., 0.5 = half a CPU, 2.0 = 2 CPUs).
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss
    )]
    pub fn with_cpu_limit(mut self, cpus: f64) -> Self {
        let quota = (cpus * self.cpu_period_us as f64) as u64;
        self.cpu_quota_us = Some(quota);
        self
    }

    /// Set memory limit in bytes.
    #[must_use]
    pub fn with_memory_limit(mut self, bytes: u64) -> Self {
        self.memory_bytes = Some(bytes);
        self
    }
}
