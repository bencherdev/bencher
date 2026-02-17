//! Cgroup v2 management for resource limits.

#![expect(clippy::print_stderr)]

use std::fs;

use camino::{Utf8Path, Utf8PathBuf};

use crate::RunnerError;
use crate::cpu::CpuLayout;
use crate::error::JailError;
use crate::jail::ResourceLimits;

/// Default cgroup v2 mount point.
const CGROUP_ROOT: &str = "/sys/fs/cgroup";

/// Bencher cgroup hierarchy base.
const BENCHER_CGROUP_BASE: &str = "bencher";

/// A cgroup manager for a single run.
pub struct CgroupManager {
    cgroup_path: Utf8PathBuf,
    created: bool,
}

impl CgroupManager {
    /// Create a new cgroup for the given run ID.
    pub fn new(run_id: &str) -> Result<Self, RunnerError> {
        let cgroup_path = Utf8PathBuf::from(CGROUP_ROOT)
            .join(BENCHER_CGROUP_BASE)
            .join(run_id);

        // Ensure parent bencher cgroup exists
        let parent = Utf8PathBuf::from(CGROUP_ROOT).join(BENCHER_CGROUP_BASE);
        if !parent.exists() {
            fs::create_dir_all(&parent).map_err(|e| JailError::CreateCgroup {
                path: parent.clone(),
                source: e,
            })?;

            // Enable controllers in parent
            Self::enable_controllers(&parent)?;
        }

        // Create this run's cgroup
        if !cgroup_path.exists() {
            fs::create_dir_all(&cgroup_path).map_err(|e| JailError::CreateCgroup {
                path: cgroup_path.clone(),
                source: e,
            })?;
        }

        Ok(Self {
            cgroup_path,
            created: true,
        })
    }

    /// Enable controllers in a cgroup.
    ///
    /// Enables cpu, memory, and pids controllers (required), and io/cpuset controllers
    /// (optional, for I/O throttling and CPU pinning). Returns an error if required
    /// controllers cannot be enabled.
    fn enable_controllers(path: &Utf8Path) -> Result<(), RunnerError> {
        let subtree_control = path.join("cgroup.subtree_control");

        // Try to enable all controllers at once (most efficient)
        if fs::write(&subtree_control, "+cpu +memory +pids +io +cpuset").is_err() {
            // Fall back to enabling without cpuset
            if fs::write(&subtree_control, "+cpu +memory +pids +io").is_err() {
                // Fall back to enabling required controllers without io or cpuset
                fs::write(&subtree_control, "+cpu +memory +pids").map_err(|e| {
                    JailError::EnableControllers {
                        path: subtree_control.clone(),
                        source: e,
                    }
                })?;
            }
        }

        // Verify that required controllers are enabled
        let enabled = fs::read_to_string(&subtree_control).unwrap_or_default();
        for required in ["cpu", "memory", "pids"] {
            if !enabled.contains(required) {
                return Err(JailError::MissingController {
                    controller: required.to_owned(),
                    path: subtree_control.clone(),
                    enabled: enabled.clone(),
                }
                .into());
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
            drop(self.write_file("memory.swap.max", "0"));
        }

        // OOM group kill: when the cgroup hits its memory limit, kill ALL processes
        // in the group together. This prevents partial kills that leave orphan processes.
        drop(self.write_file("memory.oom.group", "1"));

        // PIDs limit
        self.write_file("pids.max", &limits.max_procs.to_string())?;

        // I/O limits - applied to all block devices
        // Note: This requires knowing the device major:minor. We attempt to
        // discover common devices, but this may not work in all configuration.
        if limits.io_read_bps.is_some() || limits.io_write_bps.is_some() {
            self.apply_io_limits(limits);
        }

        Ok(())
    }

    /// Apply CPU pinning via cpuset controller.
    ///
    /// Restricts processes in this cgroup to run only on the specified CPUs.
    /// This is used to pin Firecracker VMs to benchmark cores, isolating them
    /// from housekeeping tasks.
    ///
    /// # Arguments
    ///
    /// * `layout` - CPU layout with benchmark cores to pin to
    ///
    /// # Errors
    ///
    /// Returns Ok even if cpuset is not available (logs a warning).
    /// CPU pinning is best-effort for isolation but not required for correctness.
    pub fn apply_cpuset(&self, layout: &CpuLayout) -> Result<(), RunnerError> {
        if !layout.has_isolation() {
            // No meaningful isolation possible (single core or overlapping sets)
            return Ok(());
        }

        let cpuset = layout.benchmark_cpuset();
        if cpuset.is_empty() {
            return Ok(());
        }

        // Try to write cpuset.cpus - may fail if cpuset controller is not available
        let path = self.cgroup_path.join("cpuset.cpus");
        if let Err(e) = fs::write(&path, &cpuset) {
            // Log warning but don't fail - cpuset is optional for isolation
            eprintln!(
                "Warning: failed to set cpuset.cpus to '{cpuset}' (cpuset controller may not be available): {e}"
            );
        } else {
            // Also need to set cpuset.mems for cpuset to work
            // Use all memory nodes (typically just "0" on most systems)
            let mems_path = self.cgroup_path.join("cpuset.mems");
            if let Err(e) = fs::write(&mems_path, "0") {
                eprintln!("Warning: failed to set cpuset.mems: {e}");
            }
        }

        Ok(())
    }

    /// Apply I/O bandwidth limits.
    ///
    /// Attempts to apply io.max limits to discovered block devices.
    /// The io.max format is: "MAJ:MIN rbps=BYTES wbps=BYTES"
    fn apply_io_limits(&self, limits: &ResourceLimits) {
        use std::fmt::Write as _;

        // Try to find block devices to apply limits to
        let devices = Self::discover_block_devices();

        if devices.is_empty() {
            // No devices found, skip I/O limits silently
            return;
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
            let _unused = writeln!(
                io_max_content,
                "{major}:{minor} rbps={read_limit} wbps={write_limit}"
            );
        }

        // Try to write io.max - may fail if io controller is not available
        let path = self.cgroup_path.join("io.max");
        if let Err(e) = fs::write(&path, &io_max_content) {
            // Log warning but don't fail - io controller may not be available
            eprintln!("Warning: failed to set io.max (io controller may not be available): {e}");
        }
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
                if let Ok(content) = fs::read_to_string(&dev_path)
                    && let Some((major_str, minor_str)) = content.trim().split_once(':')
                    && let (Ok(major), Ok(minor)) =
                        (major_str.parse::<u32>(), minor_str.parse::<u32>())
                {
                    devices.push((major, minor));
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

    /// Add a process by PID to this cgroup.
    pub fn add_pid(&self, pid: u32) -> Result<(), RunnerError> {
        self.write_file("cgroup.procs", &pid.to_string())
    }

    /// Write to a cgroup file.
    fn write_file(&self, name: &str, value: &str) -> Result<(), RunnerError> {
        let path = self.cgroup_path.join(name);
        fs::write(&path, value).map_err(|e| JailError::WriteCgroup { path, source: e })?;
        Ok(())
    }

    /// Get the cgroup path.
    #[must_use]
    pub fn path(&self) -> &Utf8Path {
        &self.cgroup_path
    }

    /// Clean up the cgroup.
    pub fn cleanup(&mut self) -> Result<(), RunnerError> {
        if self.created && self.cgroup_path.exists() {
            if let Err(e) = fs::remove_dir(&self.cgroup_path) {
                // Log but don't fail - cgroup might still have processes
                eprintln!("Warning: failed to remove cgroup {}: {e}", self.cgroup_path);
            } else {
                self.created = false;
            }
        }
        Ok(())
    }
}

impl Drop for CgroupManager {
    fn drop(&mut self) {
        drop(self.cleanup());
    }
}

/// Check if cgroup v2 is available.
#[expect(dead_code)]
#[must_use]
pub fn is_cgroup_v2_available() -> bool {
    Utf8Path::new(CGROUP_ROOT)
        .join("cgroup.controllers")
        .exists()
}
