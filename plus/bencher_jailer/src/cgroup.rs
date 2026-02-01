//! Cgroup v2 management for resource limits.
//!
//! This module handles creating and configuring cgroups for resource isolation.

use std::fs;
use std::path::{Path, PathBuf};

use crate::config::ResourceLimits;
use crate::error::JailerError;

/// Default cgroup v2 mount point.
const CGROUP_ROOT: &str = "/sys/fs/cgroup";

/// Bencher cgroup hierarchy base.
const BENCHER_CGROUP_BASE: &str = "bencher";

/// A cgroup manager for a single jail.
pub struct CgroupManager {
    /// Path to this jail's cgroup.
    cgroup_path: PathBuf,
    /// Whether we successfully created the cgroup.
    created: bool,
}

impl CgroupManager {
    /// Create a new cgroup for the given jail ID.
    pub fn new(jail_id: &str) -> Result<Self, JailerError> {
        let cgroup_path = PathBuf::from(CGROUP_ROOT)
            .join(BENCHER_CGROUP_BASE)
            .join(jail_id);

        // Ensure parent bencher cgroup exists
        let parent = PathBuf::from(CGROUP_ROOT).join(BENCHER_CGROUP_BASE);
        if !parent.exists() {
            fs::create_dir_all(&parent).map_err(|e| {
                JailerError::Cgroup(format!("failed to create parent cgroup {}: {e}", parent.display()))
            })?;

            // Enable controllers in parent
            Self::enable_controllers(&parent)?;
        }

        // Create this jail's cgroup
        if !cgroup_path.exists() {
            fs::create_dir_all(&cgroup_path).map_err(|e| {
                JailerError::Cgroup(format!(
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
    fn enable_controllers(path: &Path) -> Result<(), JailerError> {
        let subtree_control = path.join("cgroup.subtree_control");

        // Try to enable cpu, memory, and io controllers
        // This may fail if controllers aren't available, which is ok
        let _ = fs::write(&subtree_control, "+cpu +memory +io +pids");

        Ok(())
    }

    /// Apply resource limits to this cgroup.
    pub fn apply_limits(&self, limits: &ResourceLimits) -> Result<(), JailerError> {
        // CPU limit (cpu.max)
        if let Some(quota) = limits.cpu_quota_us {
            let cpu_max = format!("{quota} {}", limits.cpu_period_us);
            self.write_file("cpu.max", &cpu_max)?;
        }

        // Memory limit (memory.max)
        if let Some(bytes) = limits.memory_bytes {
            self.write_file("memory.max", &bytes.to_string())?;
        }

        // PIDs limit (pids.max)
        self.write_file("pids.max", &limits.max_procs.to_string())?;

        Ok(())
    }

    /// Add the current process to this cgroup.
    pub fn add_self(&self) -> Result<(), JailerError> {
        let pid = std::process::id();
        self.write_file("cgroup.procs", &pid.to_string())
    }

    /// Add a specific PID to this cgroup.
    pub fn add_pid(&self, pid: u32) -> Result<(), JailerError> {
        self.write_file("cgroup.procs", &pid.to_string())
    }

    /// Write to a cgroup file.
    fn write_file(&self, name: &str, value: &str) -> Result<(), JailerError> {
        let path = self.cgroup_path.join(name);
        fs::write(&path, value).map_err(|e| {
            JailerError::Cgroup(format!("failed to write {}: {e}", path.display()))
        })
    }

    /// Get the cgroup path.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.cgroup_path
    }

    /// Clean up the cgroup.
    ///
    /// This removes the cgroup directory. All processes must have exited first.
    pub fn cleanup(&mut self) -> Result<(), JailerError> {
        if self.created && self.cgroup_path.exists() {
            // Try to remove the cgroup
            // This will fail if there are still processes in it
            if let Err(e) = fs::remove_dir(&self.cgroup_path) {
                // Log but don't fail - the cgroup might still have processes
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
pub fn is_cgroup_v2_available() -> bool {
    // Check for cgroup v2 unified hierarchy
    let cgroup_type = PathBuf::from(CGROUP_ROOT).join("cgroup.controllers");
    cgroup_type.exists()
}

/// Get available controllers.
pub fn available_controllers() -> Result<Vec<String>, JailerError> {
    let path = PathBuf::from(CGROUP_ROOT).join("cgroup.controllers");
    let contents = fs::read_to_string(&path)
        .map_err(|e| JailerError::Cgroup(format!("failed to read controllers: {e}")))?;

    Ok(contents.split_whitespace().map(String::from).collect())
}
