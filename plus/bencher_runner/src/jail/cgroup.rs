//! Cgroup v2 management for resource limits.

#![expect(clippy::print_stderr, reason = "cgroup setup prints diagnostics")]

use std::fs;

use camino::{Utf8Path, Utf8PathBuf};

use crate::RunnerError;
use crate::cpu::CpuLayout;
use crate::error::JailError;
use crate::jail::{CgroupSurvived, VmId};

/// Default cgroup v2 mount point.
const CGROUP_ROOT: &str = "/sys/fs/cgroup";

/// Bencher cgroup hierarchy base.
pub(crate) const BENCHER_CGROUP_BASE: &str = "bencher";

/// A cgroup manager for a single run.
pub struct CgroupManager {
    cgroup_path: Utf8PathBuf,
    created: bool,
    /// Raised when this cgroup could not be removed, which holds the chroot that
    /// names it for the next job's sweep.
    cgroup_survived: CgroupSurvived,
    cpuset_control: CpusetControl,
}

impl CgroupManager {
    /// Create a new cgroup for the given microVM.
    pub fn new(vm_id: &VmId, cgroup_survived: CgroupSurvived) -> Result<Self, RunnerError> {
        let cgroup_path = vm_cgroup(vm_id.as_str());

        let parent = Utf8PathBuf::from(CGROUP_ROOT).join(BENCHER_CGROUP_BASE);
        fs::create_dir_all(&parent).map_err(|e| JailError::CreateCgroup {
            path: parent.clone(),
            source: e,
        })?;

        // Enable controllers in the parent. Always attempted (idempotent):
        // the parent may have been created without controllers, e.g. by
        // the tuning cpuset partition at startup.
        let cpuset_control = Self::enable_controllers(&parent)?;

        // Create this run's cgroup, claiming it only when a stat shows it was
        // absent, because `Drop` removes whatever this claims.
        let created = match cgroup_path.try_exists() {
            Ok(true) => false,
            Ok(false) => {
                fs::create_dir_all(&cgroup_path).map_err(|e| JailError::CreateCgroup {
                    path: cgroup_path.clone(),
                    source: e,
                })?;
                true
            },
            Err(e) => {
                return Err(JailError::ReadCgroup {
                    path: cgroup_path,
                    source: e,
                }
                .into());
            },
        };

        Ok(Self {
            cgroup_path,
            created,
            cgroup_survived,
            cpuset_control,
        })
    }

    /// Wrap an existing directory without owning it, with `cpuset` answered
    /// because a stand-in tree's missing `cpuset.cpus` is one its test chose.
    #[cfg(test)]
    #[must_use]
    pub fn detached(cgroup_path: Utf8PathBuf) -> Self {
        Self {
            cgroup_path,
            created: false,
            cgroup_survived: CgroupSurvived::default(),
            cpuset_control: CpusetControl::Answered,
        }
    }

    /// Enable controllers in a cgroup.
    ///
    /// Enables cpu, memory, and pids controllers (required), and io/cpuset controllers
    /// (optional, for I/O throttling and CPU pinning). The verification read is the
    /// real gate: write failures are tolerated when the required controllers are
    /// already enabled (e.g., pre-configured by an admin for an unprivileged runner).
    ///
    /// Returns what the widest write established about `cpuset`, since only its
    /// refusal can tell a host with nothing to delegate from a write that failed.
    fn enable_controllers(path: &Utf8Path) -> Result<CpusetControl, RunnerError> {
        let subtree_control = path.join("cgroup.subtree_control");

        // Try to enable all controllers at once, falling back to smaller sets
        let (cpuset_control, write_result) =
            match fs::write(&subtree_control, "+cpu +memory +pids +io +cpuset") {
                Ok(()) => (CpusetControl::Answered, Ok(())),
                Err(e) => (
                    CpusetControl::from_refusal(e),
                    fs::write(&subtree_control, "+cpu +memory +pids +io")
                        .or_else(|_| fs::write(&subtree_control, "+cpu +memory +pids")),
                ),
            };

        // Verify that required controllers are enabled
        let enabled = fs::read_to_string(&subtree_control).map_err(|e| JailError::ReadCgroup {
            path: subtree_control.clone(),
            source: e,
        })?;
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

        Ok(cpuset_control)
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
    /// Returns an error when a delegated `cpuset` cannot be applied and
    /// verified; an undelegated one degrades to [`Cpuset::Unavailable`].
    pub fn apply_cpuset(&self, layout: &CpuLayout) -> Result<Cpuset, RunnerError> {
        if !layout.has_isolation() {
            // No meaningful isolation possible (single core or overlapping sets)
            return Ok(Cpuset::Unavailable("the CPU layout offers no isolation"));
        }

        let cpuset = layout.benchmark_cpuset();
        if cpuset.is_empty() {
            return Ok(Cpuset::Unavailable("the benchmark core set is empty"));
        }

        // A stat that failed is not an undelegated controller.
        let path = self.cgroup_path.join("cpuset.cpus");
        match path.try_exists() {
            Ok(true) => {},
            Ok(false) => return Ok(self.no_cpuset()),
            Err(e) => return Err(JailError::ReadCgroup { path, source: e }.into()),
        }
        // Delegation is settled by now, so every write failure, absence
        // included, is an error.
        if let Err(e) = fs::write(&path, &cpuset) {
            return Err(JailError::WriteCgroup { path, source: e }.into());
        }

        // Also need to set cpuset.mems for cpuset to work. Use the parent's
        // effective memory nodes so multi-node NUMA hosts are not forced onto
        // node 0.
        let mems = match self.cgroup_path.parent() {
            Some(parent) => effective_mems(parent).map_err(|e| JailError::ReadCgroup {
                path: parent.join(MEMS_EFFECTIVE),
                source: e,
            })?,
            None => NODE_ZERO.to_owned(),
        };
        let mems_path = self.cgroup_path.join("cpuset.mems");
        if let Err(e) = fs::write(&mems_path, &mems) {
            return Err(JailError::WriteCgroup {
                path: mems_path,
                source: e,
            }
            .into());
        }

        self.verify_cpuset(&cpuset, &mems)
    }

    /// Why this run has no cpuset when the cgroup has no `cpuset.cpus`, blaming
    /// the host only when the kernel said so.
    fn no_cpuset(&self) -> Cpuset {
        match &self.cpuset_control {
            CpusetControl::Answered => Cpuset::Unavailable(UNDELEGATED),
            CpusetControl::Unanswered(e) => {
                eprintln!(
                    "Warning: the cpuset controller could not be enabled on the parent of {}: {e}. Whether this host delegates it was never established.",
                    self.cgroup_path
                );
                Cpuset::Unavailable(UNENABLED)
            },
        }
    }

    /// Confirm the kernel granted the requested sets, because cgroup v2 silently
    /// narrows a write to the parent's effective set, possibly to nothing.
    fn verify_cpuset(&self, cpus: &str, mems: &str) -> Result<Cpuset, RunnerError> {
        for (file, requested) in [
            ("cpuset.cpus.effective", cpus),
            ("cpuset.mems.effective", mems),
        ] {
            let path = self.cgroup_path.join(file);
            // The `cpuset.cpus` write proved delegation, so a missing effective
            // file is a failed verification, not an undelegated controller.
            let effective = fs::read_to_string(&path).map_err(|e| JailError::ReadCgroup {
                path: path.clone(),
                source: e,
            })?;

            // Both sides must parse, so two unparseable strings cannot verify
            // each other as equal.
            let (Some(requested_set), Some(effective_set)) =
                (parse_cpuset(requested), parse_cpuset(&effective))
            else {
                return Err(JailError::CpusetUnparseable {
                    path,
                    requested: requested.to_owned(),
                    effective: effective.trim().to_owned(),
                }
                .into());
            };
            if effective_set != requested_set {
                return Err(JailError::CpusetNarrowed {
                    path,
                    requested: requested.to_owned(),
                    effective: effective.trim().to_owned(),
                }
                .into());
            }
        }

        Ok(Cpuset::Applied)
    }

    /// Disable swap for this cgroup.
    ///
    /// Keeps benchmark memory resident: swap thrashing adds run-to-run
    /// variance and distorts memory measurements.
    pub fn disable_swap(&self) -> Result<(), RunnerError> {
        self.write_file("memory.swap.max", "0")
    }

    /// Opened before the fork so the `pre_exec` join is a single
    /// async-signal-safe `write` on an existing descriptor.
    pub fn open_procs(&self) -> Result<fs::File, JailError> {
        let path = self.cgroup_path.join("cgroup.procs");
        fs::OpenOptions::new()
            .write(true)
            .open(&path)
            .map_err(|e| JailError::OpenCgroupProcs { path, source: e })
    }

    /// Catches a `pre_exec` join that succeeded against the wrong cgroup, which
    /// the spawn itself would not report.
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
    ///
    /// Best effort is sound because anything this misses makes
    /// [`Self::cleanup`]'s `rmdir` fail, which keeps the chroot for the next
    /// job's sweep.
    pub fn kill_all(&self) {
        if let Err(e) = self.write_file("cgroup.kill", "1") {
            eprintln!("Warning: failed to kill cgroup subtree: {e}");
        }
    }

    /// Clean up the cgroup.
    ///
    /// A refused `rmdir` raises `cgroup_survived` instead of returning an error,
    /// so the chroot that names the cgroup is kept for the next job's sweep.
    pub fn cleanup(&mut self) {
        if !self.created {
            return;
        }
        // A stat that failed is not a cgroup that is gone, so it raises the
        // signal too.
        match self.cgroup_path.try_exists() {
            Ok(false) => self.created = false,
            Ok(true) => {
                if let Err(e) = fs::remove_dir(&self.cgroup_path) {
                    eprintln!(
                        "Warning: failed to remove cgroup {}: {e}. Something is still in it, so the next job sweeps it along with the jail that names it.",
                        self.cgroup_path
                    );
                    self.cgroup_survived.set();
                } else {
                    self.created = false;
                }
            },
            Err(e) => {
                eprintln!(
                    "Warning: cannot tell whether cgroup {} is still there: {e}. It is treated as still there, so the next job sweeps it along with the jail that names it.",
                    self.cgroup_path
                );
                self.cgroup_survived.set();
            },
        }
    }
}

impl Drop for CgroupManager {
    fn drop(&mut self) {
        self.cleanup();
    }
}

pub(crate) const MEMS_EFFECTIVE: &str = "cpuset.mems.effective";

const NODE_ZERO: &str = "0";

/// Read a cgroup's effective memory nodes (`cpuset.mems.effective`).
///
/// Falls back to node `0` when the file is missing or empty (e.g., the
/// cpuset controller is not enabled). Using effective mems instead of a
/// hardcoded node keeps multi-node NUMA hosts from forcing all benchmark
/// memory onto node 0.
///
/// Any other read failure is an error, since node `0` written for a read that
/// did not happen would pin a multi-node host's guest memory to one node.
pub(crate) fn effective_mems(cgroup: &Utf8Path) -> Result<String, std::io::Error> {
    match fs::read_to_string(cgroup.join(MEMS_EFFECTIVE)) {
        Ok(mems) if !mems.trim().is_empty() => Ok(mems.trim().to_owned()),
        Ok(_) => Ok(NODE_ZERO.to_owned()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(NODE_ZERO.to_owned()),
        Err(e) => Err(e),
    }
}

const UNDELEGATED: &str = "the cpuset controller is not delegated to this cgroup";

const UNENABLED: &str = "the cpuset controller could not be enabled on the parent cgroup, so this run has none and whether this host delegates it is unknown";

/// What asking the kernel to delegate `cpuset` established, which decides
/// whether an absent `cpuset.cpus` may be blamed on the host.
enum CpusetControl {
    /// The kernel either enabled `cpuset` or said it has none to enable here.
    Answered,
    /// The write failed for a reason that says nothing about `cpuset`.
    Unanswered(std::io::Error),
}

impl CpusetControl {
    /// `ENOENT` (a controller the parent does not offer) and `EINVAL` (one the
    /// kernel lacks) are answers; any other refusal is the question failing.
    fn from_refusal(e: std::io::Error) -> Self {
        match e.raw_os_error() {
            Some(libc::ENOENT | libc::EINVAL) => Self::Answered,
            _ => Self::Unanswered(e),
        }
    }
}

/// Whether the cpuset actually confined the VMM to the benchmark cores.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cpuset {
    /// The cgroup confines the VMM to the benchmark cores, read back and
    /// confirmed.
    Applied,
    Unavailable(&'static str),
}

/// Parse a kernel cpu list (`0-3,5,7-9`), returning `None` rather than a
/// partial set when any component will not parse.
fn parse_cpuset(cpuset: &str) -> Option<std::collections::BTreeSet<usize>> {
    let mut cpus = std::collections::BTreeSet::new();
    for group in cpuset.trim().split(',').filter(|group| !group.is_empty()) {
        match group.split_once('-') {
            Some((start, end)) => {
                let start = start.trim().parse().ok()?;
                let end = end.trim().parse::<usize>().ok()?;
                cpus.extend(start..=end);
            },
            None => {
                cpus.insert(group.trim().parse().ok()?);
            },
        }
    }
    Some(cpus)
}

/// How long to retry a stale cgroup's `rmdir` while the reap before it lands.
const REMOVE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(5);

const REMOVE_INTERVAL: std::time::Duration = std::time::Duration::from_millis(50);

/// Remove the cgroup a swept jail left behind, only after its VMM has been
/// reaped, since `rmdir` fails on a cgroup that still holds a process.
pub(crate) fn remove_stale_cgroup(vm_id: &VmId) -> Result<(), JailError> {
    remove_stale_cgroup_at(vm_cgroup(vm_id.as_str()))
}

/// Refuse to measure while any Bencher cgroup holds a process, called before
/// this job's own cgroup exists so every one it finds is somebody else's.
pub(crate) fn refuse_occupied_cgroups() -> Result<(), JailError> {
    refuse_occupied_cgroups_at(&Utf8PathBuf::from(CGROUP_ROOT).join(BENCHER_CGROUP_BASE))
}

fn refuse_occupied_cgroups_at(base: &Utf8Path) -> Result<(), JailError> {
    let read_failed = |source| JailError::ReadCgroup {
        path: base.to_owned(),
        source,
    };
    let entries = match fs::read_dir(base) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(read_failed(e)),
    };
    for entry in entries {
        let entry = entry.map_err(read_failed)?;
        if !entry.file_type().map_err(read_failed)?.is_dir() {
            continue;
        }
        let cgroup = entry.path();
        let pids = subtree_pids(&cgroup)?;
        if !pids.is_empty() {
            return Err(JailError::CgroupOccupied {
                cgroup: Utf8PathBuf::from(cgroup.to_string_lossy().into_owned()),
                pids: pids
                    .iter()
                    .map(u32::to_string)
                    .collect::<Vec<_>>()
                    .join(", "),
            });
        }
    }
    Ok(())
}

/// A cgroup gone since it was listed holds nothing.
fn subtree_pids(cgroup: &std::path::Path) -> Result<Vec<u32>, JailError> {
    let read_failed = |source| JailError::ReadCgroup {
        path: Utf8PathBuf::from(cgroup.to_string_lossy().into_owned()),
        source,
    };
    let procs = match fs::read_to_string(cgroup.join("cgroup.procs")) {
        Ok(procs) => procs,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(read_failed(e)),
    };
    let mut pids = procs
        .lines()
        .map(|line| line.trim().parse::<u32>())
        .collect::<Result<Vec<_>, _>>()
        .map_err(|e| read_failed(std::io::Error::new(std::io::ErrorKind::InvalidData, e)))?;
    let entries = match fs::read_dir(cgroup) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(pids),
        Err(e) => return Err(read_failed(e)),
    };
    for entry in entries {
        let entry = entry.map_err(read_failed)?;
        if entry.file_type().map_err(read_failed)?.is_dir() {
            pids.extend(subtree_pids(&entry.path())?);
        }
    }
    Ok(pids)
}

/// The cgroup and the chroot share this id, so a sweep holding either can
/// find the other.
pub(crate) fn vm_cgroup(vm_id: &str) -> Utf8PathBuf {
    Utf8PathBuf::from(CGROUP_ROOT)
        .join(BENCHER_CGROUP_BASE)
        .join(vm_id)
}

/// Takes the path so tests can exercise the retry policy outside
/// `/sys/fs/cgroup`.
fn remove_stale_cgroup_at(path: Utf8PathBuf) -> Result<(), JailError> {
    // The caller deletes the chroot, the only handle a later sweep has, on
    // `Ok`, so a failed stat must not pass for "already gone".
    match path.try_exists() {
        Ok(false) => return Ok(()),
        Ok(true) => {},
        Err(e) => return Err(JailError::StaleCgroup { path, source: e }),
    }

    let deadline = std::time::Instant::now() + REMOVE_TIMEOUT;
    loop {
        match fs::remove_dir(&path) {
            Ok(()) => {
                eprintln!("Removed stale cgroup {path} left by a previous runner");
                return Ok(());
            },
            // Someone else removed it first; only a stat that succeeded counts.
            Err(_) if path.try_exists().is_ok_and(|exists| !exists) => return Ok(()),
            // Only `EBUSY` clears with waiting, and retrying anything else would
            // cost the full timeout on every job, since the kept chroot brings
            // each sweep back here.
            Err(e) if !is_contended(&e) || std::time::Instant::now() >= deadline => {
                // An error rather than a warning, so the next job sweeps again.
                return Err(JailError::StaleCgroup { path, source: e });
            },
            Err(_) => std::thread::sleep(REMOVE_INTERVAL),
        }
    }
}

/// `EBUSY` is a cgroup still holding a process or a live child, the only
/// `rmdir` refusal that waiting can clear.
fn is_contended(e: &std::io::Error) -> bool {
    e.raw_os_error() == Some(libc::EBUSY)
}

/// Matches whole lines: pid `7` must not be satisfied by pid `70`.
pub(crate) fn procs_contains_pid(procs: &str, pid: u32) -> bool {
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

    fn cpuset_tree(effective: &str) -> (tempfile::TempDir, CgroupManager) {
        // The mems written here derive from a parent with no
        // `cpuset.mems.effective`, which falls back to node 0.
        cpuset_tree_with_mems(effective, "0")
    }

    fn cpuset_tree_with_mems(
        effective_cpus: &str,
        effective_mems: &str,
    ) -> (tempfile::TempDir, CgroupManager) {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        fs::write(root.join("cpuset.cpus"), "").unwrap();
        fs::write(root.join("cpuset.mems"), "").unwrap();
        fs::write(root.join("cpuset.cpus.effective"), effective_cpus).unwrap();
        fs::write(root.join("cpuset.mems.effective"), effective_mems).unwrap();
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
    fn a_narrowed_memory_node_set_is_an_error() {
        let (_dir, manager) = cpuset_tree_with_mems("2-7\n", "\n");
        let layout = CpuLayout::with_core_count(8);

        manager.apply_cpuset(&layout).unwrap_err();
    }

    #[test]
    fn a_memory_node_set_that_cannot_be_read_back_fails_the_job() {
        // The cpus write proved delegation, so an unreadable mems read-back must
        // fail rather than degrade to an undelegated controller.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        fs::write(root.join("cpuset.cpus"), "").unwrap();
        fs::write(root.join("cpuset.mems"), "").unwrap();
        fs::write(root.join("cpuset.cpus.effective"), "2-7\n").unwrap();
        let manager = CgroupManager::detached(root);
        let layout = CpuLayout::with_core_count(8);

        let err = manager.apply_cpuset(&layout).unwrap_err().to_string();

        assert!(
            err.contains("cpuset.mems.effective"),
            "names the read that did not happen: {err}"
        );
    }

    #[test]
    fn an_undelegated_cpuset_controller_degrades() {
        // A host without cpuset has no cpuset.cpus, which is a declared absence
        // of isolation rather than a failure.
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
    fn a_cpuset_nobody_could_ask_about_is_not_an_undelegated_one() {
        // Blaming the host for an enable write that failed on its own would send
        // the operator looking in the wrong place.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let manager = CgroupManager {
            cgroup_path: root,
            created: false,
            cgroup_survived: CgroupSurvived::default(),
            cpuset_control: CpusetControl::Unanswered(std::io::Error::from_raw_os_error(
                libc::EROFS,
            )),
        };
        let layout = CpuLayout::with_core_count(8);

        assert_eq!(
            manager.apply_cpuset(&layout).unwrap(),
            Cpuset::Unavailable(UNENABLED)
        );
    }

    #[test]
    fn only_the_kernel_saying_it_has_no_cpuset_counts_as_an_answer() {
        let errno = std::io::Error::from_raw_os_error;

        assert!(matches!(
            CpusetControl::from_refusal(errno(libc::ENOENT)),
            CpusetControl::Answered
        ));
        assert!(matches!(
            CpusetControl::from_refusal(errno(libc::EINVAL)),
            CpusetControl::Answered
        ));
        assert!(matches!(
            CpusetControl::from_refusal(errno(libc::EPERM)),
            CpusetControl::Unanswered(_)
        ));
        assert!(matches!(
            CpusetControl::from_refusal(errno(libc::EBUSY)),
            CpusetControl::Unanswered(_)
        ));
    }

    #[test]
    fn a_silently_narrowed_cpuset_is_an_error() {
        // The kernel narrows a written set to the parent's without failing the
        // write.
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
        let (_dir, manager) = cpuset_tree("0\n");
        let layout = CpuLayout::with_core_count(1);

        assert!(matches!(
            manager.apply_cpuset(&layout).unwrap(),
            Cpuset::Unavailable(_)
        ));
    }

    #[test]
    fn parse_cpuset_reads_kernel_cpu_lists() {
        assert_eq!(parse_cpuset("2-7"), Some((2..=7).collect()));
        assert_eq!(parse_cpuset("2,3,4,5,6,7\n"), Some((2..=7).collect()));
        assert_eq!(parse_cpuset("0-1,4,6-7"), Some([0, 1, 4, 6, 7].into()));
        assert_eq!(parse_cpuset("3"), Some([3].into()));
        // An empty list is a real answer: a cpuset narrowed to nothing.
        assert_eq!(parse_cpuset(""), Some([].into()));
        assert_eq!(parse_cpuset("\n"), Some([].into()));
    }

    #[test]
    fn parse_cpuset_does_not_turn_garbage_into_a_set() {
        // Dropping an unparseable component would hand the caller a partial set
        // nobody read.
        for garbage in ["x", "2-x", "x-7", "2-", "-7", "1,x,3", "0xff"] {
            assert_eq!(parse_cpuset(garbage), None, "'{garbage}' is not a set");
        }
    }

    #[test]
    fn an_effective_set_that_cannot_be_parsed_fails_the_job() {
        // An effective set this runner cannot read is no evidence the kernel
        // honored the request.
        let (_dir, manager) = cpuset_tree("not-a-cpu-list\n");
        let layout = CpuLayout::with_core_count(8);

        let err = manager.apply_cpuset(&layout).unwrap_err().to_string();

        assert!(
            err.contains("not-a-cpu-list"),
            "names what could not be read: {err}"
        );
    }

    #[test]
    fn a_cgroup_that_already_existed_is_not_ours_to_remove() {
        // Removing a cgroup this did not create would delete someone else's.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

        let ours = CgroupManager {
            cgroup_path: root.join("ours"),
            created: true,
            cgroup_survived: CgroupSurvived::default(),
            cpuset_control: CpusetControl::Answered,
        };
        let theirs = CgroupManager {
            cgroup_path: root.join("theirs"),
            created: false,
            cgroup_survived: CgroupSurvived::default(),
            cpuset_control: CpusetControl::Answered,
        };
        fs::create_dir_all(ours.path()).unwrap();
        fs::create_dir_all(theirs.path()).unwrap();
        let theirs_path = theirs.path().to_owned();

        drop(ours);
        drop(theirs);

        assert!(!root.join("ours").exists(), "we remove what we created");
        assert!(theirs_path.exists(), "we leave what we did not create");
    }

    #[test]
    fn only_a_contended_cgroup_is_worth_waiting_out() {
        let errno = std::io::Error::from_raw_os_error;

        assert!(is_contended(&errno(libc::EBUSY)));
        assert!(!is_contended(&errno(libc::EPERM)));
        assert!(!is_contended(&errno(libc::EROFS)));
        assert!(!is_contended(&errno(libc::ENOTEMPTY)));
        assert!(!is_contended(&std::io::Error::other("no errno at all")));
    }

    #[test]
    fn a_removal_that_will_never_succeed_does_not_spend_the_budget() {
        // Retrying a refusal other than `EBUSY` would cost the full timeout on
        // every job's sweep of a host where nothing is going to change.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let stuck = root.join("stuck");
        fs::create_dir_all(stuck.join("occupant")).unwrap();

        let start = std::time::Instant::now();
        remove_stale_cgroup_at(stuck.clone()).unwrap_err();

        assert!(
            start.elapsed() < REMOVE_TIMEOUT,
            "a refusal that will not change is not waited out"
        );
        assert!(stuck.exists(), "and the cgroup is left for the next sweep");
    }

    #[test]
    fn a_stale_cgroup_that_is_already_gone_is_nothing_to_remove() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

        remove_stale_cgroup_at(root.join("absent")).unwrap();
    }

    #[test]
    fn an_empty_stale_cgroup_is_removed() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let stale = root.join("stale");
        fs::create_dir_all(&stale).unwrap();

        remove_stale_cgroup_at(stale.clone()).unwrap();

        assert!(!stale.exists());
    }

    fn cgroup_base(cgroups: &[(&str, &str)]) -> (tempfile::TempDir, Utf8PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let base = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        for (cgroup, procs) in cgroups {
            let path = base.join(cgroup);
            fs::create_dir_all(&path).unwrap();
            fs::write(path.join("cgroup.procs"), procs).unwrap();
        }
        (dir, base)
    }

    #[test]
    fn a_process_in_another_bencher_cgroup_fails_the_job() {
        // Measuring beside another job's process, nested or not, would report
        // contended numbers.
        for (cgroup, pid) in [("vm-a", "vm-a"), ("vm-a/nested", "vm-a")] {
            let (_dir, base) = cgroup_base(&[("vm-a", ""), (cgroup, "4242\n")]);

            let err = refuse_occupied_cgroups_at(&base).unwrap_err().to_string();

            assert!(err.contains(pid), "names the cgroup: {err}");
            assert!(err.contains("4242"), "names the pid: {err}");
        }
    }

    #[test]
    fn the_base_itself_and_empty_cgroups_do_not_fail_the_job() {
        let (_dir, base) = cgroup_base(&[("vm-b", ""), ("local-c", "")]);
        fs::write(base.join("cgroup.procs"), "1\n").unwrap();

        refuse_occupied_cgroups_at(&base).unwrap();
        refuse_occupied_cgroups_at(&base.join("absent")).unwrap();
    }

    #[test]
    fn a_base_that_cannot_be_listed_is_not_an_empty_one() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let not_a_dir = root.join("bencher");
        fs::write(&not_a_dir, b"in the way").unwrap();

        refuse_occupied_cgroups_at(&not_a_dir).unwrap_err();
    }

    #[test]
    fn procs_contains_pid_matches_whole_lines() {
        assert!(procs_contains_pid("7\n70\n701\n", 7));
        assert!(procs_contains_pid("7\n70\n701\n", 701));
        assert!(!procs_contains_pid("70\n701\n", 7));
        assert!(!procs_contains_pid("", 7));
        assert!(!procs_contains_pid("\n", 7));
    }

    #[test]
    fn a_cgroup_that_will_not_go_away_holds_its_chroot() {
        // Warning alone would let the chroot that names this cgroup go, leaving
        // a later sweep nothing to find it by.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let survived = CgroupSurvived::default();
        let mut manager = CgroupManager {
            cgroup_path: root.join("stuck"),
            created: true,
            cgroup_survived: survived.clone(),
            cpuset_control: CpusetControl::Answered,
        };
        fs::create_dir_all(manager.path()).unwrap();
        fs::write(manager.path().join("cgroup.procs"), "42\n").unwrap();

        manager.cleanup();

        assert!(
            survived.is_set(),
            "a cgroup that outlives its job holds the chroot that names it"
        );
        assert!(manager.path().exists());
    }

    #[test]
    fn a_removed_cgroup_leaves_the_signal_alone() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let survived = CgroupSurvived::default();
        let mut manager = CgroupManager {
            cgroup_path: root.join("gone"),
            created: true,
            cgroup_survived: survived.clone(),
            cpuset_control: CpusetControl::Answered,
        };
        fs::create_dir_all(manager.path()).unwrap();

        manager.cleanup();

        assert!(!manager.path().exists());
        assert!(
            !survived.is_set(),
            "a clean teardown must not hold the chroot back"
        );
    }

    #[test]
    fn open_procs_reports_a_missing_cgroup() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let manager = CgroupManager::detached(root.join("absent"));

        manager.open_procs().unwrap_err();
        manager.contains_pid(1).unwrap_err();
    }

    #[test]
    fn contains_pid_reads_the_listing() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        fs::write(root.join("cgroup.procs"), "123\n456\n").unwrap();
        let manager = CgroupManager::detached(root);

        assert!(manager.contains_pid(456).unwrap());
        assert!(!manager.contains_pid(789).unwrap());
    }

    #[test]
    fn an_unreadable_node_set_is_not_node_zero() {
        // Node 0 for a failed read would pin guest memory to one node of a
        // multi-node host.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let not_a_dir = root.join("cgroup");
        fs::write(&not_a_dir, b"in the way").unwrap();

        effective_mems(&not_a_dir).unwrap_err();
    }

    #[test]
    fn a_cpuset_that_cannot_be_verified_is_not_a_degrade() {
        // The write proved delegation, so a missing effective file must fail
        // rather than degrade to an undelegated controller.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        fs::write(root.join("cpuset.cpus"), "").unwrap();
        fs::write(root.join("cpuset.mems"), "").unwrap();
        let manager = CgroupManager::detached(root);
        let layout = CpuLayout::with_core_count(8);

        manager.apply_cpuset(&layout).unwrap_err();
    }

    #[test]
    fn effective_mems_reads_file() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        fs::write(root.join("cpuset.mems.effective"), "0-1\n").unwrap();

        assert_eq!(effective_mems(&root).unwrap(), "0-1");
    }

    #[test]
    fn effective_mems_missing_falls_back_to_node_zero() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

        assert_eq!(effective_mems(&root).unwrap(), "0");
    }

    #[test]
    fn effective_mems_empty_falls_back_to_node_zero() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        fs::write(root.join("cpuset.mems.effective"), "\n").unwrap();

        assert_eq!(effective_mems(&root).unwrap(), "0");
    }
}
