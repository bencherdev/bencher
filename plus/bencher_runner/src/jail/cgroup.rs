//! Cgroup v2 management for resource limits.

#![expect(clippy::print_stderr, reason = "cgroup setup prints diagnostics")]

use std::fs;

use camino::{Utf8Path, Utf8PathBuf};

use crate::RunnerError;
use crate::cpu::CpuLayout;
use crate::error::JailError;
use crate::jail::{ResourceLimits, VmId};

/// Default cgroup v2 mount point.
const CGROUP_ROOT: &str = "/sys/fs/cgroup";

/// Bencher cgroup hierarchy base.
pub(crate) const BENCHER_CGROUP_BASE: &str = "bencher";

/// A cgroup manager for a single run.
pub struct CgroupManager {
    cgroup_path: Utf8PathBuf,
    created: bool,
}

impl CgroupManager {
    /// Create a new cgroup for the given microVM.
    pub fn new(vm_id: &VmId) -> Result<Self, RunnerError> {
        let cgroup_path = Utf8PathBuf::from(CGROUP_ROOT)
            .join(BENCHER_CGROUP_BASE)
            .join(vm_id.as_str());

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
        // the tuning cpuset partition at startup.
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

    /// Wrap an existing cgroup directory without creating or removing it.
    ///
    /// For tests that exercise the placement logic against a stand-in tree
    /// rather than the real cgroup filesystem.
    #[cfg(test)]
    #[must_use]
    pub fn detached(cgroup_path: Utf8PathBuf) -> Self {
        Self {
            cgroup_path,
            created: false,
        }
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
    /// Returns an error when the cpuset controller is present but rejects the
    /// write. That is a half-applied fidelity mechanism: the cgroup would
    /// exist without confining the VMM to the benchmark cores, so the run
    /// would report a number measured somewhere other than where it claims.
    ///
    /// A controller that is not there at all is a different thing and is not
    /// an error. `enable_controllers` falls back as far as `+cpu +memory
    /// +pids`, and only those three are required, so a host that does not
    /// delegate `cpuset` (a containerized runner, or a cgroup namespace
    /// without it in `subtree_control`) creates its cgroup successfully and
    /// then has no `cpuset.cpus` to write. That is a declared absence of
    /// isolation, which the caller degrades on rather than failing.
    pub fn apply_cpuset(&self, layout: &CpuLayout) -> Result<Cpuset, RunnerError> {
        if !layout.has_isolation() {
            // Single core, or overlapping sets: there is nothing to confine to.
            return Ok(Cpuset::Unavailable("the CPU layout offers no isolation"));
        }

        let cpuset = layout.benchmark_cpuset();
        if cpuset.is_empty() {
            return Ok(Cpuset::Unavailable("the benchmark core set is empty"));
        }

        let path = self.cgroup_path.join("cpuset.cpus");
        if !path.exists() {
            return Ok(Cpuset::Unavailable(UNDELEGATED));
        }
        if let Err(e) = fs::write(&path, &cpuset) {
            return classify_cpuset_error(path, e);
        }

        // Also need to set cpuset.mems for cpuset to work. Use the parent's
        // effective memory nodes so multi-node NUMA hosts are not forced onto
        // node 0. Applied cpus without mems is the half-applied case.
        let mems = self
            .cgroup_path
            .parent()
            .map_or_else(|| "0".to_owned(), effective_mems);
        let mems_path = self.cgroup_path.join("cpuset.mems");
        if let Err(e) = fs::write(&mems_path, &mems) {
            return classify_cpuset_error(mems_path, e);
        }

        self.verify_cpuset(&cpuset)
    }

    /// Confirm the kernel actually gave the cgroup the cores that were asked
    /// for.
    ///
    /// A successful write proves nothing here. Under cgroup v2 a `cpuset.cpus`
    /// that overlaps a sibling's exclusive set, or reaches past the parent's
    /// effective set, is accepted and then silently narrowed, possibly to
    /// nothing at all, in which case the VMM simply inherits the parent's
    /// CPUs. The whole point of separating applied from half-applied is lost
    /// if the applied case is taken on trust, so the effective set is read
    /// back and has to match exactly. Every other fidelity mechanism here
    /// already reads back: the partition mode does, and so does cgroup
    /// placement.
    fn verify_cpuset(&self, requested: &str) -> Result<Cpuset, RunnerError> {
        let path = self.cgroup_path.join("cpuset.cpus.effective");
        let effective = match fs::read_to_string(&path) {
            Ok(effective) => effective,
            // Nothing to read back means nothing was delegated, the same
            // conclusion the write path draws.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Cpuset::Unavailable(UNDELEGATED));
            },
            Err(e) => return Err(JailError::ReadCgroup { path, source: e }.into()),
        };

        if parse_cpuset(&effective) == parse_cpuset(requested) {
            Ok(Cpuset::Applied)
        } else {
            Err(JailError::CpusetNarrowed {
                path,
                requested: requested.to_owned(),
                effective: effective.trim().to_owned(),
            }
            .into())
        }
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

    /// Open this cgroup's `cgroup.procs` for writing.
    ///
    /// The descriptor is opened before the fork so the `pre_exec` closure that
    /// joins the cgroup performs only a `write` of a fixed byte on an existing
    /// descriptor: no allocation, no path resolution, nothing that is not
    /// async-signal-safe.
    pub fn open_procs(&self) -> Result<fs::File, JailError> {
        let path = self.cgroup_path.join("cgroup.procs");
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .map_err(|e| JailError::OpenCgroupProcs { path, source: e })
    }

    /// Whether `pid` is a member of this cgroup.
    ///
    /// A failed `pre_exec` write already surfaces as a failed spawn; this is
    /// for the different case of a write that succeeded against the wrong
    /// destination. A cgroup that exists but does not contain the VMM is a
    /// silent lie about where the benchmark ran.
    pub fn contains_pid(&self, pid: u32) -> Result<bool, JailError> {
        let path = self.cgroup_path.join("cgroup.procs");
        let procs = fs::read_to_string(&path).map_err(|e| JailError::ReadCgroup {
            path: path.clone(),
            source: e,
        })?;
        Ok(procs_contains_pid(&procs, pid))
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

    /// SIGKILL every process in this cgroup's subtree (best-effort).
    ///
    /// Writes `1` to `cgroup.kill` (Linux 5.14+). Reaps grandchildren
    /// that survive a direct-child kill, e.g. on timeout or cancellation,
    /// so no stray work lingers on benchmark cores and the cgroup can be
    /// removed.
    pub fn kill_all(&self) {
        if let Err(e) = self.write_file("cgroup.kill", "1") {
            eprintln!("Warning: failed to kill cgroup subtree: {e}");
        }
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

/// Read a cgroup's effective memory nodes (`cpuset.mems.effective`).
///
/// Falls back to node `0` when the file is missing or empty (e.g., the
/// cpuset controller is not enabled). Using effective mems instead of a
/// hardcoded node keeps multi-node NUMA hosts from forcing all benchmark
/// memory onto node 0.
pub(crate) fn effective_mems(cgroup: &Utf8Path) -> String {
    match fs::read_to_string(cgroup.join("cpuset.mems.effective")) {
        Ok(mems) if !mems.trim().is_empty() => mems.trim().to_owned(),
        _ => "0".to_owned(),
    }
}

/// Why a run has no CPU isolation, when it has none.
const UNDELEGATED: &str = "the cpuset controller is not delegated to this cgroup";

/// Whether the cpuset actually confined the VMM to the benchmark cores.
///
/// Every variant that is not [`Self::Applied`] carries the reason, so no
/// variant can claim confinement without a verified write behind it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cpuset {
    /// The cgroup confines the VMM to the benchmark cores, read back and
    /// confirmed.
    Applied,
    /// There is no CPU isolation to be had, for the reason given.
    Unavailable(&'static str),
}

/// Decide whether a failed cpuset write is an absent controller or a refusal.
///
/// A file that is not there is the controller not being delegated, which is a
/// limitation. Anything else is the kernel refusing a cpuset it does
/// understand, which would leave the cgroup claiming an isolation it does not
/// have.
fn classify_cpuset_error(path: Utf8PathBuf, error: std::io::Error) -> Result<Cpuset, RunnerError> {
    if error.kind() == std::io::ErrorKind::NotFound {
        Ok(Cpuset::Unavailable(UNDELEGATED))
    } else {
        Err(JailError::WriteCgroup {
            path,
            source: error,
        }
        .into())
    }
}

/// Parse a kernel cpu list (`0-3,5,7-9`) into the set of cpus it names.
///
/// Compared as sets rather than as strings, because the kernel is free to
/// render the same set differently from the way it was written.
fn parse_cpuset(cpuset: &str) -> std::collections::BTreeSet<usize> {
    let mut cpus = std::collections::BTreeSet::new();
    for group in cpuset.trim().split(',').filter(|group| !group.is_empty()) {
        match group.split_once('-') {
            Some((start, end)) => {
                if let (Ok(start), Ok(end)) = (start.trim().parse(), end.trim().parse::<usize>()) {
                    cpus.extend(start..=end);
                }
            },
            None => {
                if let Ok(cpu) = group.trim().parse() {
                    cpus.insert(cpu);
                }
            },
        }
    }
    cpus
}

/// How long to keep trying to remove a stale cgroup.
///
/// `rmdir` fails while the cgroup still holds a process, and the reap that
/// precedes it may need a moment to land.
const REMOVE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

/// How often to retry.
const REMOVE_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);

/// Remove the cgroup a swept jail left behind.
///
/// The cgroup and the chroot are named by the same VM id by construction, so
/// the id read off the chroot directory names the cgroup exactly.
///
/// Must run after the VMM in it has been reaped: `rmdir` on a cgroup that
/// still holds a process fails, which is what forces that ordering. A cgroup
/// that survives is worth shouting about, because it holds the exclusive
/// benchmark CPUs and the next job's cpuset will be rejected because of it.
pub(crate) fn remove_stale_cgroup(vm_id: &VmId) -> Result<(), JailError> {
    let path = Utf8PathBuf::from(CGROUP_ROOT)
        .join(BENCHER_CGROUP_BASE)
        .join(vm_id.as_str());
    if !path.exists() {
        return Ok(());
    }

    let deadline = std::time::Instant::now() + REMOVE_TIMEOUT;
    loop {
        match fs::remove_dir(&path) {
            Ok(()) => {
                eprintln!("Warning: removed stale cgroup {path} left by a previous runner");
                return Ok(());
            },
            // Someone else got there first, which is the outcome either way.
            Err(_) if !path.exists() => return Ok(()),
            Err(e) if std::time::Instant::now() >= deadline => {
                // Reported rather than warned. The leftover still owns the
                // exclusive benchmark CPUs, so every later job's cpuset would
                // be rejected; failing here means the next job sweeps again
                // instead of inheriting a host that can never isolate.
                return Err(JailError::StaleCgroup { path, source: e });
            },
            Err(_) => std::thread::sleep(REMOVE_INTERVAL),
        }
    }
}

/// Whether a `cgroup.procs` listing contains `pid`.
///
/// Matches whole lines: pid `7` must not be satisfied by pid `70`.
fn procs_contains_pid(procs: &str, pid: u32) -> bool {
    procs
        .lines()
        .any(|line| line.trim().parse::<u32>() == Ok(pid))
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

    use super::*;

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

    /// A stand-in cgroup tree with the cpuset controller delegated.
    ///
    /// `effective` is what the kernel would report back after the write, which
    /// is the whole point: a real kernel may narrow it silently.
    fn cpuset_tree(effective: &str) -> (tempfile::TempDir, CgroupManager) {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        fs::write(root.join("cpuset.cpus"), "").unwrap();
        fs::write(root.join("cpuset.mems"), "").unwrap();
        fs::write(root.join("cpuset.cpus.effective"), effective).unwrap();
        (dir, CgroupManager::detached(root))
    }

    #[test]
    fn a_delegated_cpuset_that_the_kernel_honors_is_applied() {
        let (_dir, manager) = cpuset_tree("2-7\n");
        let layout = CpuLayout::with_core_count(8);

        assert_eq!(manager.apply_cpuset(&layout).unwrap(), Cpuset::Applied);
        assert_eq!(
            fs::read_to_string(manager.path().join("cpuset.cpus")).unwrap(),
            "2-7"
        );
    }

    #[test]
    fn an_equivalent_rendering_still_counts_as_applied() {
        // The kernel is free to render the same set differently from the way
        // it was written, so the comparison is over sets and not strings.
        let (_dir, manager) = cpuset_tree("2,3,4,5,6,7\n");
        let layout = CpuLayout::with_core_count(8);

        assert_eq!(manager.apply_cpuset(&layout).unwrap(), Cpuset::Applied);
    }

    #[test]
    fn an_undelegated_cpuset_controller_degrades() {
        // A host that does not delegate cpuset creates its cgroup fine and
        // then has no cpuset.cpus to write. That is a declared absence of
        // isolation, not a failure, and no CI runner reaches it.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let manager = CgroupManager::detached(root);
        let layout = CpuLayout::with_core_count(8);

        assert_eq!(
            manager.apply_cpuset(&layout).unwrap(),
            Cpuset::Unavailable(UNDELEGATED)
        );
    }

    #[test]
    fn a_silently_narrowed_cpuset_is_an_error() {
        // The kernel accepts a cpuset that overlaps a sibling's exclusive set
        // and then narrows it. A successful write proves nothing.
        let (_dir, manager) = cpuset_tree("2-3\n");
        let layout = CpuLayout::with_core_count(8);

        let err = manager.apply_cpuset(&layout).unwrap_err().to_string();

        assert!(err.contains("2-7"), "names what was asked for: {err}");
        assert!(err.contains("2-3"), "names what was granted: {err}");
    }

    #[test]
    fn an_emptied_cpuset_is_an_error() {
        // The worst case: narrowed to nothing, so the VMM inherits the
        // parent's CPUs and the run silently measures the whole machine.
        let (_dir, manager) = cpuset_tree("\n");
        let layout = CpuLayout::with_core_count(8);

        manager.apply_cpuset(&layout).unwrap_err();
    }

    #[test]
    fn a_layout_with_no_isolation_claims_nothing() {
        // Every variant that is not Applied has to carry a reason, so no
        // path can report confinement without a verified write behind it.
        let (_dir, manager) = cpuset_tree("0\n");
        let layout = CpuLayout::with_core_count(1);

        assert!(matches!(
            manager.apply_cpuset(&layout).unwrap(),
            Cpuset::Unavailable(_)
        ));
    }

    #[test]
    fn parse_cpuset_reads_kernel_cpu_lists() {
        assert_eq!(parse_cpuset("2-7"), (2..=7).collect());
        assert_eq!(parse_cpuset("2,3,4,5,6,7\n"), (2..=7).collect());
        assert_eq!(parse_cpuset("0-1,4,6-7"), [0, 1, 4, 6, 7].into());
        assert_eq!(parse_cpuset("3"), [3].into());
        assert!(parse_cpuset("").is_empty());
        assert!(parse_cpuset("\n").is_empty());
    }

    #[test]
    fn procs_contains_pid_matches_whole_lines() {
        assert!(procs_contains_pid("7\n70\n701\n", 7));
        assert!(procs_contains_pid("7\n70\n701\n", 701));
        // A prefix match must not count: pid 7 is not pid 70.
        assert!(!procs_contains_pid("70\n701\n", 7));
        assert!(!procs_contains_pid("", 7));
        assert!(!procs_contains_pid("\n", 7));
    }

    #[test]
    fn open_procs_reports_a_missing_cgroup() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let manager = CgroupManager {
            cgroup_path: root.join("absent"),
            created: false,
        };

        manager.open_procs().unwrap_err();
        manager.contains_pid(1).unwrap_err();
    }

    #[test]
    fn contains_pid_reads_the_listing() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        fs::write(root.join("cgroup.procs"), "123\n456\n").unwrap();
        let manager = CgroupManager {
            cgroup_path: root,
            created: false,
        };

        assert!(manager.contains_pid(456).unwrap());
        assert!(!manager.contains_pid(789).unwrap());
    }

    #[test]
    fn effective_mems_reads_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        fs::write(root.join("cpuset.mems.effective"), "0-1\n").unwrap();

        assert_eq!(effective_mems(&root), "0-1");
    }

    #[test]
    fn effective_mems_missing_falls_back_to_node_zero() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

        assert_eq!(effective_mems(&root), "0");
    }

    #[test]
    fn effective_mems_empty_falls_back_to_node_zero() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        fs::write(root.join("cpuset.mems.effective"), "\n").unwrap();

        assert_eq!(effective_mems(&root), "0");
    }
}
