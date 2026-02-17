//! Resource management for Firecracker microVMs.
//!
//! This module provides cgroup-based resource limits
//! for controlling Firecracker microVM processes.

#[cfg(target_os = "linux")]
mod cgroup;

#[cfg(target_os = "linux")]
pub use cgroup::CgroupManager;

use serde::{Deserialize, Serialize};

/// Resource limits for the Firecracker microVM process.
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

    /// Maximum I/O read bandwidth in bytes per second.
    /// Applied via cgroup v2 io.max.
    #[serde(default)]
    pub io_read_bps: Option<u64>,

    /// Maximum I/O write bandwidth in bytes per second.
    /// Applied via cgroup v2 io.max.
    #[serde(default)]
    pub io_write_bps: Option<u64>,
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
            io_read_bps: None,
            io_write_bps: None,
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

    /// Set I/O bandwidth limits in bytes per second.
    #[must_use]
    pub fn with_io_limits(mut self, read_bps: u64, write_bps: u64) -> Self {
        self.io_read_bps = Some(read_bps);
        self.io_write_bps = Some(write_bps);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resource_limits_defaults() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.cpu_quota_us, None);
        assert_eq!(limits.cpu_period_us, 100_000);
        assert_eq!(limits.memory_bytes, None);
        assert_eq!(limits.max_fds, 1024);
        assert_eq!(limits.max_procs, 64);
        assert_eq!(limits.io_read_bps, None);
        assert_eq!(limits.io_write_bps, None);
    }

    #[test]
    fn with_cpu_limit_one_cpu() {
        let limits = ResourceLimits::default().with_cpu_limit(1.0);
        assert_eq!(limits.cpu_quota_us, Some(100_000));
    }

    #[test]
    fn with_cpu_limit_half_cpu() {
        let limits = ResourceLimits::default().with_cpu_limit(0.5);
        assert_eq!(limits.cpu_quota_us, Some(50_000));
    }

    #[test]
    fn with_cpu_limit_two_cpus() {
        let limits = ResourceLimits::default().with_cpu_limit(2.0);
        assert_eq!(limits.cpu_quota_us, Some(200_000));
    }

    #[test]
    fn with_memory_limit() {
        let limits = ResourceLimits::default().with_memory_limit(1024 * 1024 * 512);
        assert_eq!(limits.memory_bytes, Some(0x2000_0000));
    }

    #[test]
    fn with_io_limits() {
        let limits = ResourceLimits::default().with_io_limits(1_000_000, 500_000);
        assert_eq!(limits.io_read_bps, Some(1_000_000));
        assert_eq!(limits.io_write_bps, Some(500_000));
    }

    #[test]
    fn builder_chain() {
        let limits = ResourceLimits::default()
            .with_cpu_limit(2.0)
            .with_memory_limit(1024)
            .with_io_limits(100, 200);
        assert_eq!(limits.cpu_quota_us, Some(200_000));
        assert_eq!(limits.memory_bytes, Some(1024));
        assert_eq!(limits.io_read_bps, Some(100));
        assert_eq!(limits.io_write_bps, Some(200));
    }

    #[test]
    fn serde_round_trip() {
        let limits = ResourceLimits::default()
            .with_cpu_limit(1.5)
            .with_memory_limit(2048);
        let json = serde_json::to_string(&limits).unwrap();
        let parsed: ResourceLimits = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.cpu_quota_us, limits.cpu_quota_us);
        assert_eq!(parsed.memory_bytes, Some(2048));
        assert_eq!(parsed.cpu_period_us, 100_000);
    }

    #[test]
    fn serde_deserialize_minimal() {
        let json = "{}";
        let limits: ResourceLimits = serde_json::from_str(json).unwrap();
        assert_eq!(limits.cpu_quota_us, None);
        assert_eq!(limits.cpu_period_us, 100_000);
        assert_eq!(limits.max_procs, 64);
    }
}
