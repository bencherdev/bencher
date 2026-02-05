//! Cgroup v2 management for resource limits.

use std::fs;
use std::path::{Path, PathBuf};

use crate::RunnerError;
use crate::jail::ResourceLimits;

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
    ///
    /// Enables cpu, memory, and pids controllers (required), and io controller
    /// (optional, for I/O throttling). Returns an error if required controllers
    /// cannot be enabled.
    fn enable_controllers(path: &Path) -> Result<(), RunnerError> {
        let subtree_control = path.join("cgroup.subtree_control");

        // Try to enable all controllers at once (most efficient)
        if fs::write(&subtree_control, "+cpu +memory +pids +io").is_err() {
            // Fall back to enabling required controllers without io
            fs::write(&subtree_control, "+cpu +memory +pids").map_err(|e| {
                RunnerError::Jail(format!(
                    "failed to enable required cgroup controllers (cpu, memory, pids) at {}: {e}",
                    subtree_control.display()
                ))
            })?;
        }

        // Verify that required controllers are enabled
        let enabled = fs::read_to_string(&subtree_control).unwrap_or_default();
        for required in ["cpu", "memory", "pids"] {
            if !enabled.contains(required) {
                return Err(RunnerError::Jail(format!(
                    "required cgroup controller '{required}' not enabled at {}. Enabled: {enabled}",
                    subtree_control.display()
                )));
            }
        }

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

            // Disable swap to ensure benchmark memory measurements are accurate
            // and to prevent swap thrashing from affecting benchmark results.
            let _ = self.write_file("memory.swap.max", "0");
        }

        // OOM group kill: when the cgroup hits its memory limit, kill ALL processes
        // in the group together. This prevents partial kills that leave orphan processes.
        let _ = self.write_file("memory.oom.group", "1");

        // PIDs limit
        self.write_file("pids.max", &limits.max_procs.to_string())?;

        // I/O limits - applied to all block devices
        // Note: This requires knowing the device major:minor. We attempt to
        // discover common devices, but this may not work in all configurations.
        if limits.io_read_bps.is_some() || limits.io_write_bps.is_some() {
            self.apply_io_limits(limits)?;
        }

        Ok(())
    }

    /// Apply I/O bandwidth limits.
    ///
    /// Attempts to apply io.max limits to discovered block devices.
    /// The io.max format is: "MAJ:MIN rbps=BYTES wbps=BYTES"
    fn apply_io_limits(&self, limits: &ResourceLimits) -> Result<(), RunnerError> {
        // Try to find block devices to apply limits to
        let devices = Self::discover_block_devices();

        if devices.is_empty() {
            // No devices found, skip I/O limits silently
            return Ok(());
        }

        let read_limit = limits
            .io_read_bps
            .map_or("max".to_owned(), |v| v.to_string());
        let write_limit = limits
            .io_write_bps
            .map_or("max".to_owned(), |v| v.to_string());

        let mut io_max_content = String::new();
        for (major, minor) in devices {
            // Format: "MAJ:MIN rbps=BYTES wbps=BYTES"
            io_max_content.push_str(&format!(
                "{major}:{minor} rbps={read_limit} wbps={write_limit}\n"
            ));
        }

        // Try to write io.max - may fail if io controller is not available
        let path = self.cgroup_path.join("io.max");
        if let Err(e) = fs::write(&path, &io_max_content) {
            // Log warning but don't fail - io controller may not be available
            eprintln!("Warning: failed to set io.max (io controller may not be available): {e}");
        }

        Ok(())
    }

    /// Discover block devices on the system.
    ///
    /// Returns a list of (major, minor) device numbers for block devices.
    fn discover_block_devices() -> Vec<(u32, u32)> {
        let mut devices = Vec::new();

        // Try to read /sys/block to find block devices
        if let Ok(entries) = fs::read_dir("/sys/block") {
            for entry in entries.flatten() {
                let dev_path = entry.path().join("dev");
                if let Ok(content) = fs::read_to_string(&dev_path) {
                    // Format is "MAJ:MIN"
                    if let Some((major_str, minor_str)) = content.trim().split_once(':') {
                        if let (Ok(major), Ok(minor)) =
                            (major_str.parse::<u32>(), minor_str.parse::<u32>())
                        {
                            devices.push((major, minor));
                        }
                    }
                }
            }
        }

        devices
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
