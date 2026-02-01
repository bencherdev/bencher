//! Cgroup v2 management for resource limits.

use std::fs;
use std::path::{Path, PathBuf};

use crate::jail::ResourceLimits;
use crate::RunnerError;

/// Default cgroup v2 mount point.
const CGROUP_ROOT: &str = "/sys/fs/cgroup";

/// Bencher cgroup hierarchy base.
const BENCHER_CGROUP_BASE: &str = "bencher";

/// A cgroup manager for a single run.
pub struct CgroupManager {
    cgroup_path: PathBuf,
    created: bool,
}

impl CgroupManager {
    /// Create a new cgroup for the given run ID.
    pub fn new(run_id: &str) -> Result<Self, RunnerError> {
        let cgroup_path = PathBuf::from(CGROUP_ROOT)
            .join(BENCHER_CGROUP_BASE)
            .join(run_id);

        // Ensure parent bencher cgroup exists
        let parent = PathBuf::from(CGROUP_ROOT).join(BENCHER_CGROUP_BASE);
        if !parent.exists() {
            fs::create_dir_all(&parent).map_err(|e| {
                RunnerError::Jail(format!(
                    "failed to create parent cgroup {}: {e}",
                    parent.display()
                ))
            })?;

            // Enable controllers in parent
            Self::enable_controllers(&parent)?;
        }

        // Create this run's cgroup
        if !cgroup_path.exists() {
            fs::create_dir_all(&cgroup_path).map_err(|e| {
                RunnerError::Jail(format!(
                    "failed to create cgroup {}: {e}",
                    cgroup_path.display()
                ))
            })?;
        }

        Ok(Self {
            cgroup_path,
            created: true,
        })
    }

    /// Enable controllers in a cgroup.
    fn enable_controllers(path: &Path) -> Result<(), RunnerError> {
        let subtree_control = path.join("cgroup.subtree_control");
        // Try to enable controllers - may fail if not available
        let _ = fs::write(&subtree_control, "+cpu +memory +pids");
        Ok(())
    }

    /// Apply resource limits to this cgroup.
    pub fn apply_limits(&self, limits: &ResourceLimits) -> Result<(), RunnerError> {
        // CPU limit
        if let Some(quota) = limits.cpu_quota_us {
            let cpu_max = format!("{quota} {}", limits.cpu_period_us);
            self.write_file("cpu.max", &cpu_max)?;
        }

        // Memory limit
        if let Some(bytes) = limits.memory_bytes {
            self.write_file("memory.max", &bytes.to_string())?;
        }

        // PIDs limit
        self.write_file("pids.max", &limits.max_procs.to_string())?;

        Ok(())
    }

    /// Add the current process to this cgroup.
    pub fn add_self(&self) -> Result<(), RunnerError> {
        let pid = std::process::id();
        self.write_file("cgroup.procs", &pid.to_string())
    }

    /// Write to a cgroup file.
    fn write_file(&self, name: &str, value: &str) -> Result<(), RunnerError> {
        let path = self.cgroup_path.join(name);
        fs::write(&path, value)
            .map_err(|e| RunnerError::Jail(format!("failed to write {}: {e}", path.display())))
    }

    /// Get the cgroup path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.cgroup_path
    }

    /// Clean up the cgroup.
    pub fn cleanup(&mut self) -> Result<(), RunnerError> {
        if self.created && self.cgroup_path.exists() {
            if let Err(e) = fs::remove_dir(&self.cgroup_path) {
                // Log but don't fail - cgroup might still have processes
                eprintln!(
                    "Warning: failed to remove cgroup {}: {e}",
                    self.cgroup_path.display()
                );
            } else {
                self.created = false;
            }
        }
        Ok(())
    }
}

impl Drop for CgroupManager {
    fn drop(&mut self) {
        let _ = self.cleanup();
    }
}

/// Check if cgroup v2 is available.
#[must_use]
pub fn is_cgroup_v2_available() -> bool {
    PathBuf::from(CGROUP_ROOT)
        .join("cgroup.controllers")
        .exists()
}
