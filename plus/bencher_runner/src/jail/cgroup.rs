//! Cgroup v2 management for resource limits.

#![expect(clippy::print_stderr, reason = "cgroup setup prints diagnostics")]

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
        }

        // Enable controllers in the parent. Always attempted (idempotent):
        // the parent may have been created without controllers, e.g. by
        // BencherPartition at startup.
        Self::enable_controllers(&parent)?;

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
    /// (optional, for I/O throttling and CPU pinning). The verification read is the
    /// real gate: write failures are tolerated when the required controllers are
    /// already enabled (e.g., pre-configured by an admin for an unprivileged runner).
    fn enable_controllers(path: &Utf8Path) -> Result<(), RunnerError> {
        let subtree_control = path.join("cgroup.subtree_control");

        // Try to enable all controllers at once, falling back to smaller sets
        let write_result = fs::write(&subtree_control, "+cpu +memory +pids +io +cpuset")
            .or_else(|_| fs::write(&subtree_control, "+cpu +memory +pids +io"))
            .or_else(|_| fs::write(&subtree_control, "+cpu +memory +pids"));

        // Verify that required controllers are enabled
        let enabled = fs::read_to_string(&subtree_control).unwrap_or_default();
        if let Some(missing) = missing_required_controller(&enabled) {
            return Err(match write_result {
                Err(e) => JailError::EnableControllers {
                    path: subtree_control,
                    source: e,
                }
                .into(),
                Ok(()) => JailError::MissingController {
                    controller: missing.to_owned(),
                    path: subtree_control,
                    enabled,
                }
                .into(),
            });
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
            drop(self.disable_swap());
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

    /// Disable swap for this cgroup.
    ///
    /// Keeps benchmark memory resident: swap thrashing adds run-to-run
    /// variance and distorts memory measurements.
    pub fn disable_swap(&self) -> Result<(), RunnerError> {
        self.write_file("memory.swap.max", "0")
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

/// Manages the parent `bencher` cgroup as a cpuset scheduler partition.
///
/// Turning `/sys/fs/cgroup/bencher` into an `isolated` cpuset partition
/// removes the benchmark cores from the root scheduling domain: the kernel
/// load balancer can no longer pull other tasks onto them. This is the
/// runtime equivalent of the `isolcpus=` boot argument. Per-run cgroups
/// created under the partition inherit it automatically.
///
/// The partition must be a child of a valid partition root; the cgroup
/// root always qualifies, which is why this lives on the parent `bencher`
/// cgroup and not on a per-run child.
pub struct BencherPartition {
    /// The parent `bencher` cgroup path.
    path: Utf8PathBuf,
    /// The cgroup v2 mount point.
    root: Utf8PathBuf,
}

/// Achieved cpuset partition level, in decreasing order of isolation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PartitionLevel {
    /// Scheduler domain isolation: no load balancing onto benchmark cores.
    Isolated,
    /// A separate scheduler domain with load balancing inside it.
    Root,
    /// A plain cgroup (no partition); cpuset pinning still applies.
    Member,
}

impl BencherPartition {
    /// Create a partition manager for the given cgroup v2 mount point
    /// (`/sys/fs/cgroup` in production; tests pass a tempdir tree).
    #[must_use]
    pub fn new(cgroup_root: &Utf8Path) -> Self {
        Self {
            path: cgroup_root.join(BENCHER_CGROUP_BASE),
            root: cgroup_root.to_owned(),
        }
    }

    /// Turn the `bencher` cgroup into a cpuset partition on the benchmark
    /// cores, verifying the kernel accepted it.
    ///
    /// Falls back `isolated` -> `root` -> `member` when the kernel reports
    /// the partition as invalid (read-back verification). All writes are
    /// recorded on the guard and restored in reverse order on drop.
    /// Best-effort: any failure degrades to [`PartitionLevel::Member`],
    /// which matches the behavior before partitions were introduced.
    pub fn apply(
        &self,
        layout: &CpuLayout,
        guard: &mut crate::tuning::TuningGuard,
    ) -> PartitionLevel {
        // The bencher cgroup only gets cpuset files once the cpuset
        // controller is enabled in the root subtree. Additive and
        // idempotent, so it is not saved for restore.
        if let Err(e) = fs::write(self.root.join("cgroup.subtree_control"), "+cpuset") {
            eprintln!("Warning: failed to enable cpuset controller in cgroup root: {e}");
            return PartitionLevel::Member;
        }

        if !self.path.exists()
            && let Err(e) = fs::create_dir_all(&self.path)
        {
            eprintln!("Warning: failed to create cgroup {}: {e}", self.path);
            return PartitionLevel::Member;
        }

        // A partition needs explicit cpus and mems.
        if !save_and_write(
            guard,
            &self.path.join("cpuset.cpus"),
            &layout.benchmark_cpuset(),
            "bencher cpuset.cpus",
        ) {
            return PartitionLevel::Member;
        }
        if !save_and_write(
            guard,
            &self.path.join("cpuset.mems"),
            "0",
            "bencher cpuset.mems",
        ) {
            return PartitionLevel::Member;
        }

        let partition_path = self.path.join("cpuset.cpus.partition");
        let original = match fs::read_to_string(&partition_path) {
            Ok(value) => partition_mode_token(&value).to_owned(),
            Err(e) => {
                // Missing on kernels without cpuset partition support.
                eprintln!("Warning: cpuset partitions unavailable ({partition_path}: {e})");
                return PartitionLevel::Member;
            },
        };

        // Save the restore entry before attempting any mode write: a write
        // can succeed at the syscall level while the kernel rejects the
        // partition in the read-back, and the file must still revert on
        // drop. Restoring an unchanged value is a harmless no-op.
        guard.save_restore(
            partition_path.clone(),
            original,
            "bencher cpuset partition".to_owned(),
        );

        for (mode, level) in [
            ("isolated", PartitionLevel::Isolated),
            ("root", PartitionLevel::Root),
        ] {
            match try_partition_mode(&partition_path, mode) {
                Ok(()) => return level,
                Err(e) => {
                    eprintln!("Warning: cpuset partition mode '{mode}' not achieved: {e}");
                },
            }
        }

        PartitionLevel::Member
    }
}

impl std::fmt::Display for PartitionLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Isolated => write!(f, "isolated"),
            Self::Root => write!(f, "root"),
            Self::Member => write!(f, "member"),
        }
    }
}

/// Write a partition mode and verify the kernel accepted it.
///
/// The kernel reports a rejected partition in the read-back value
/// (e.g., `isolated invalid (Cpu list in cpuset.cpus not exclusive)`),
/// so a successful write is not enough.
fn try_partition_mode(path: &Utf8Path, mode: &str) -> Result<(), JailError> {
    fs::write(path, mode).map_err(|e| JailError::WriteCgroup {
        path: path.to_owned(),
        source: e,
    })?;

    verify_partition_state(path, mode)
}

/// Extract the mode token from a `cpuset.cpus.partition` read-back.
///
/// A healthy partition reads back as just the mode (`member`, `root`,
/// `isolated`); a rejected one as `<mode> invalid (<reason>)`. Only the
/// mode token is a valid value to write back on restore. An empty or
/// unreadable value falls back to the kernel default `member`.
fn partition_mode_token(state: &str) -> &str {
    state.split_whitespace().next().unwrap_or("member")
}

/// Read back a partition file and check the kernel reports exactly `mode`.
fn verify_partition_state(path: &Utf8Path, mode: &str) -> Result<(), JailError> {
    let state = fs::read_to_string(path).map_err(|e| JailError::WriteCgroup {
        path: path.to_owned(),
        source: e,
    })?;
    let state = state.trim();

    if state == mode {
        Ok(())
    } else {
        Err(JailError::PartitionInvalid {
            mode: mode.to_owned(),
            state: state.to_owned(),
        })
    }
}

/// Save the current value of a cgroup file on the guard, then write `value`.
///
/// An empty current value is saved as a newline so the restore write is
/// not a zero-byte no-op (clearing `cpuset.cpus` requires writing `"\n"`).
/// Returns false (with a warning) when the file cannot be read or written.
fn save_and_write(
    guard: &mut crate::tuning::TuningGuard,
    path: &Utf8Path,
    value: &str,
    label: &str,
) -> bool {
    let current = match fs::read_to_string(path) {
        Ok(current) => current.trim().to_owned(),
        Err(e) => {
            eprintln!("Warning: failed to read {path}: {e}");
            return false;
        },
    };

    if current == value {
        return true;
    }

    if let Err(e) = fs::write(path, value) {
        eprintln!("Warning: failed to write {path}: {e}");
        return false;
    }

    let restore_value = if current.is_empty() {
        "\n".to_owned()
    } else {
        current
    };
    guard.save_restore(path.to_owned(), restore_value, label.to_owned());
    true
}

/// Return the first required controller missing from a
/// `cgroup.subtree_control` listing, or `None` when all are enabled.
///
/// Matches whole tokens: `cpuset` alone must not satisfy `cpu`.
fn missing_required_controller(enabled: &str) -> Option<&'static str> {
    ["cpu", "memory", "pids"]
        .into_iter()
        .find(|required| !enabled.split_whitespace().any(|token| token == *required))
}

/// Check if cgroup v2 is available.
#[expect(dead_code, reason = "utility for future cgroup v2 feature detection")]
#[must_use]
pub fn is_cgroup_v2_available() -> bool {
    Utf8Path::new(CGROUP_ROOT)
        .join("cgroup.controllers")
        .exists()
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;

    use crate::cpu::CpuLayout;
    use crate::tuning::{TuningConfig, TuningGuard};

    use super::*;

    fn empty_guard() -> TuningGuard {
        crate::tuning::apply(&TuningConfig::disabled())
    }

    /// A fake cgroup v2 tree mirroring what the kernel exposes.
    fn fake_cgroup_root() -> (tempfile::TempDir, Utf8PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

        fs::write(root.join("cgroup.subtree_control"), "").unwrap();
        fs::create_dir_all(root.join("bencher")).unwrap();
        fs::write(root.join("bencher/cpuset.cpus"), "").unwrap();
        fs::write(root.join("bencher/cpuset.mems"), "").unwrap();
        fs::write(root.join("bencher/cpuset.cpus.partition"), "member\n").unwrap();

        (dir, root)
    }

    #[test]
    fn partition_applies_isolated() {
        let (_dir, root) = fake_cgroup_root();
        let layout = CpuLayout::with_core_count(8);
        let mut guard = empty_guard();

        let level = BencherPartition::new(&root).apply(&layout, &mut guard);

        assert_eq!(level, PartitionLevel::Isolated);
        assert_eq!(
            fs::read_to_string(root.join("bencher/cpuset.cpus")).unwrap(),
            "2-7"
        );
        assert_eq!(
            fs::read_to_string(root.join("bencher/cpuset.mems")).unwrap(),
            "0"
        );
        assert_eq!(
            fs::read_to_string(root.join("bencher/cpuset.cpus.partition")).unwrap(),
            "isolated"
        );
    }

    #[test]
    fn partition_restores_on_guard_drop() {
        let (_dir, root) = fake_cgroup_root();
        let layout = CpuLayout::with_core_count(8);

        {
            let mut guard = empty_guard();
            let level = BencherPartition::new(&root).apply(&layout, &mut guard);
            assert_eq!(level, PartitionLevel::Isolated);
        }

        // Partition demoted back, cpus and mems cleared (newline write).
        assert_eq!(
            fs::read_to_string(root.join("bencher/cpuset.cpus.partition")).unwrap(),
            "member"
        );
        assert_eq!(
            fs::read_to_string(root.join("bencher/cpuset.cpus")).unwrap(),
            "\n"
        );
        assert_eq!(
            fs::read_to_string(root.join("bencher/cpuset.mems")).unwrap(),
            "\n"
        );
    }

    #[test]
    fn partition_falls_back_to_member_when_unwritable() {
        let (_dir, root) = fake_cgroup_root();
        // A directory in place of the partition file forces every mode
        // write to fail, exercising the full fallback chain.
        fs::remove_file(root.join("bencher/cpuset.cpus.partition")).unwrap();
        fs::create_dir_all(root.join("bencher/cpuset.cpus.partition")).unwrap();
        let layout = CpuLayout::with_core_count(8);
        let mut guard = empty_guard();

        let level = BencherPartition::new(&root).apply(&layout, &mut guard);

        assert_eq!(level, PartitionLevel::Member);
    }

    #[test]
    fn partition_member_without_cgroup_v2() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let layout = CpuLayout::with_core_count(8);
        let mut guard = empty_guard();

        let level = BencherPartition::new(&root).apply(&layout, &mut guard);

        assert_eq!(level, PartitionLevel::Member);
    }

    #[test]
    fn partition_restore_entry_saved_before_mode_writes() {
        let (_dir, root) = fake_cgroup_root();
        // A previous run (or crash) left the partition file dirty: the
        // kernel reports rejected partitions inline in the read-back.
        fs::write(
            root.join("bencher/cpuset.cpus.partition"),
            "isolated invalid (Cpu list not exclusive)\n",
        )
        .unwrap();
        let layout = CpuLayout::with_core_count(8);

        {
            let mut guard = empty_guard();
            let level = BencherPartition::new(&root).apply(&layout, &mut guard);
            assert_eq!(level, PartitionLevel::Isolated);
        }

        // The restore writes back the normalized mode token, never the
        // full dirty state (which is not a valid value to write).
        assert_eq!(
            fs::read_to_string(root.join("bencher/cpuset.cpus.partition")).unwrap(),
            "isolated"
        );
    }

    #[test]
    fn partition_mode_token_normalizes_states() {
        assert_eq!(partition_mode_token("member\n"), "member");
        assert_eq!(partition_mode_token("isolated\n"), "isolated");
        assert_eq!(
            partition_mode_token("isolated invalid (Cpu list not exclusive)\n"),
            "isolated"
        );
        assert_eq!(partition_mode_token(""), "member");
    }

    #[test]
    fn missing_required_controller_matches_whole_tokens() {
        assert_eq!(missing_required_controller("cpu memory pids"), None);
        assert_eq!(
            missing_required_controller("cpuset cpu memory pids io"),
            None
        );
        assert_eq!(missing_required_controller(""), Some("cpu"));
        // "cpuset" alone must not satisfy the "cpu" controller
        assert_eq!(
            missing_required_controller("cpuset memory pids"),
            Some("cpu")
        );
        assert_eq!(missing_required_controller("cpu memory"), Some("pids"));
    }

    #[test]
    fn verify_partition_state_accepts_exact_mode() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let path = root.join("cpuset.cpus.partition");
        fs::write(&path, "isolated\n").unwrap();

        verify_partition_state(&path, "isolated").unwrap();
    }

    #[test]
    fn verify_partition_state_rejects_invalid() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let path = root.join("cpuset.cpus.partition");
        // The kernel reports a rejected partition in the read-back value.
        fs::write(&path, "isolated invalid (Cpu list not exclusive)\n").unwrap();

        let err = verify_partition_state(&path, "isolated").unwrap_err();
        assert!(err.to_string().contains("invalid"));
    }

    #[test]
    fn partition_level_display() {
        assert_eq!(PartitionLevel::Isolated.to_string(), "isolated");
        assert_eq!(PartitionLevel::Root.to_string(), "root");
        assert_eq!(PartitionLevel::Member.to_string(), "member");
    }
}
