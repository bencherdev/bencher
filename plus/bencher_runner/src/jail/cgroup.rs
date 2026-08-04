//! Cgroup v2 management for resource limits.

#![expect(clippy::print_stderr, reason = "cgroup setup prints diagnostics")]

use std::fs;

use camino::{Utf8Path, Utf8PathBuf};

use crate::RunnerError;
use crate::cpu::CpuLayout;
use crate::error::JailError;
use crate::jail::{JailSignals, VmId};

/// Default cgroup v2 mount point.
const CGROUP_ROOT: &str = "/sys/fs/cgroup";

/// Bencher cgroup hierarchy base.
pub(crate) const BENCHER_CGROUP_BASE: &str = "bencher";

/// A cgroup manager for a single run.
pub struct CgroupManager {
    cgroup_path: Utf8PathBuf,
    created: bool,
    /// Raised when this cgroup could not be removed, which holds the chroot that
    /// names it and earns a later job another sweep.
    signals: JailSignals,
}

impl CgroupManager {
    /// Create a new cgroup for the given microVM.
    ///
    /// The signal is shared with the chroot of the same id: a cgroup that
    /// cannot be removed has to keep that directory alive, because the
    /// directory name is the only handle a later sweep has for finding this
    /// cgroup again.
    pub fn new(vm_id: &VmId, signals: JailSignals) -> Result<Self, RunnerError> {
        let cgroup_path = Utf8PathBuf::from(CGROUP_ROOT)
            .join(BENCHER_CGROUP_BASE)
            .join(vm_id.as_str());

        // Unconditional, because `create_dir_all` on a directory that is
        // already there is a success. The `exists` check this replaces was a
        // read that could only mislead: an error from it would have been read as
        // an absent parent, and it gated nothing the create does not gate
        // itself.
        let parent = Utf8PathBuf::from(CGROUP_ROOT).join(BENCHER_CGROUP_BASE);
        fs::create_dir_all(&parent).map_err(|e| JailError::CreateCgroup {
            path: parent.clone(),
            source: e,
        })?;

        // Enable controllers in the parent. Always attempted (idempotent):
        // the parent may have been created without controllers, e.g. by
        // the tuning cpuset partition at startup.
        Self::enable_controllers(&parent)?;

        // Create this run's cgroup, and remember whether we are the ones who
        // made it. `Drop` removes what this created, so claiming ownership of
        // a cgroup that was already there would have it rmdir something
        // belonging to whoever did create it. Fresh ids make that unlikely,
        // but this branch exists precisely for when the id is not fresh.
        // `try_exists`, because this decides whether `Drop` may remove the
        // directory. An error read as "not there" would have this claim a cgroup
        // somebody else owns and then delete it on the way out, which is the one
        // outcome the branch exists to prevent.
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
            signals,
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
            signals: JailSignals::unwatched(),
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

        // Verify that required controllers are enabled. A read that failed is
        // not an empty list: reporting one would blame the host for a
        // controller it may well have enabled, on the strength of a question
        // nobody answered.
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

        // `try_exists`, not `exists`: the latter reports an error as an absent
        // file, so a cgroup directory the runner cannot stat would be declared
        // an undelegated controller. That is a claim about the host made from a
        // question that failed, and it is the difference between a host with no
        // isolation to offer and a host nobody could ask.
        let path = self.cgroup_path.join("cpuset.cpus");
        match path.try_exists() {
            Ok(true) => {},
            Ok(false) => return Ok(Cpuset::Unavailable(UNDELEGATED)),
            Err(e) => return Err(JailError::ReadCgroup { path, source: e }.into()),
        }
        // Every failure, absence included. Delegation was settled by the check
        // above, so a `cpuset.cpus` that has gone missing between that stat and
        // this write is a cgroup disappearing underneath the runner, not a host
        // that never had the controller. `verify_cpuset` reasons the same way
        // about the same question one step later; the two used to disagree.
        if let Err(e) = fs::write(&path, &cpuset) {
            return Err(JailError::WriteCgroup { path, source: e }.into());
        }

        // Also need to set cpuset.mems for cpuset to work. Use the parent's
        // effective memory nodes so multi-node NUMA hosts are not forced onto
        // node 0. Applied cpus without mems is the half-applied case.
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

    /// Confirm the kernel actually gave the cgroup the cores that were asked
    /// for.
    ///
    /// A successful write proves nothing here. Under cgroup v2 the effective
    /// set is the written set intersected with the parent's effective set, so
    /// a `cpuset.cpus` that reaches past the parent is accepted and then
    /// silently narrowed, possibly to nothing at all, in which case the VMM
    /// simply inherits the parent's CPUs. (An exclusive sibling narrows a set
    /// the same way, though these cgroups never claim exclusivity.) The whole
    /// point of separating applied from half-applied is lost if the applied
    /// case is taken on trust, so both effective sets are read back and have
    /// to match. Every other fidelity mechanism here already reads back: the
    /// partition mode does, and so does cgroup placement.
    fn verify_cpuset(&self, cpus: &str, mems: &str) -> Result<Cpuset, RunnerError> {
        // Memory nodes as well as cpus. The mems value is derived from the
        // parent's effective set, so narrowing is unlikely, but it is written
        // exactly the same way and would otherwise be the last write on this
        // path still taken on trust.
        for (file, requested) in [
            ("cpuset.cpus.effective", cpus),
            ("cpuset.mems.effective", mems),
        ] {
            let path = self.cgroup_path.join(file);
            // Every failure, absence included. Reaching this function means the
            // write to `cpuset.cpus` succeeded, which is proof the controller is
            // delegated, so a missing effective file here cannot mean it is not:
            // reporting an undelegated controller would tell the operator
            // something this code just observed to be false, while the run went
            // ahead on a cpuset that was never verified. The read is the whole
            // mechanism, so a read that did not happen is a failure of it.
            let effective = fs::read_to_string(&path).map_err(|e| JailError::ReadCgroup {
                path: path.clone(),
                source: e,
            })?;

            if parse_cpuset(&effective) != parse_cpuset(requested) {
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
    ///
    /// Best effort is sound here only because something else catches it:
    /// whatever this fails to kill is exactly what makes [`Self::cleanup`]'s
    /// `rmdir` fail, and that arms the reclaim signal. See [`crate::jail`].
    pub fn kill_all(&self) {
        if let Err(e) = self.write_file("cgroup.kill", "1") {
            eprintln!("Warning: failed to kill cgroup subtree: {e}");
        }
    }

    /// Clean up the cgroup.
    ///
    /// A `rmdir` the kernel refuses is raised on the reclaim signal, not just
    /// logged: it means something is still in this cgroup, and the only way to
    /// get to it later is through the chroot of the same id, so the signal both
    /// holds that directory and earns the next job a sweep.
    ///
    /// Returns nothing, because the signal is where a failure goes. A `Result`
    /// here would be a channel with nothing in it that every caller, `Drop`
    /// included, would have to discard.
    pub fn cleanup(&mut self) {
        if !self.created {
            return;
        }
        // A stat that failed is not a cgroup that is gone. Reading it as one
        // would skip both the removal and the signal, so nothing would be armed
        // and nothing would ever come back for it.
        match self.cgroup_path.try_exists() {
            Ok(false) => self.created = false,
            Ok(true) => {
                if let Err(e) = fs::remove_dir(&self.cgroup_path) {
                    eprintln!(
                        "Warning: failed to remove cgroup {}: {e}. Something is still in it, so the next job sweeps it along with the jail that names it.",
                        self.cgroup_path
                    );
                    self.signals.cgroup_survived();
                } else {
                    self.created = false;
                }
            },
            Err(e) => {
                eprintln!(
                    "Warning: cannot tell whether cgroup {} is still there: {e}. It is treated as still there, so the next job sweeps it along with the jail that names it.",
                    self.cgroup_path
                );
                self.signals.cgroup_survived();
            },
        }
    }
}

impl Drop for CgroupManager {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// The `cpuset.mems.effective` file, read to mirror a parent's memory nodes.
pub(crate) const MEMS_EFFECTIVE: &str = "cpuset.mems.effective";

/// The single memory node a host without a readable node set is assumed to have.
const NODE_ZERO: &str = "0";

/// Read a cgroup's effective memory nodes (`cpuset.mems.effective`).
///
/// Falls back to node `0` when the file is missing or empty, which is what the
/// cpuset controller not being enabled looks like. Using effective mems instead
/// of a hardcoded node keeps multi-node NUMA hosts from forcing all benchmark
/// memory onto node 0.
///
/// Which is exactly why any other failure is an error rather than the fallback.
/// This value is written to `cpuset.mems`, so answering node 0 to a read that
/// did not happen confines the guest's memory to one node on a host that may
/// have several, and the run then reports a number measured under a constraint
/// nobody chose. An absent file says the host has nothing to tell; a failed read
/// says nobody asked it.
pub(crate) fn effective_mems(cgroup: &Utf8Path) -> Result<String, std::io::Error> {
    match fs::read_to_string(cgroup.join(MEMS_EFFECTIVE)) {
        Ok(mems) if !mems.trim().is_empty() => Ok(mems.trim().to_owned()),
        Ok(_) => Ok(NODE_ZERO.to_owned()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(NODE_ZERO.to_owned()),
        Err(e) => Err(e),
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
/// that survives is worth reporting, not because it claims anything (these
/// cgroups set no exclusive cpuset, so a leftover blocks nothing) but because
/// the usual reason `rmdir` fails is that something is still running in it.
pub(crate) fn remove_stale_cgroup(vm_id: &VmId) -> Result<(), JailError> {
    remove_stale_cgroup_at(
        Utf8PathBuf::from(CGROUP_ROOT)
            .join(BENCHER_CGROUP_BASE)
            .join(vm_id.as_str()),
    )
}

/// The removal, against the directory it is given.
///
/// A parameter because the retry policy is the part worth testing, and the path
/// the caller builds reaches into `/sys/fs/cgroup` on the machine running the
/// tests.
fn remove_stale_cgroup_at(path: Utf8PathBuf) -> Result<(), JailError> {
    // The caller deletes the chroot that names this cgroup once this returns
    // `Ok`, and that chroot is the only handle any later sweep has for finding
    // the cgroup again. A stat error read as "already gone" would strand the
    // cgroup permanently and delete the one thing that could have found it,
    // which is exactly what the caller's ordering exists to prevent.
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
            // Someone else got there first, which is the outcome either way.
            // Only a stat that succeeded and said absent counts: anything else
            // falls through to the retry and, in the end, to the error.
            Err(_) if path.try_exists().is_ok_and(|exists| !exists) => return Ok(()),
            // Waiting only helps what waiting is for. `EBUSY` is the kernel
            // saying the cgroup still holds a process, which is exactly what a
            // reap that has just landed is about to clear. Every other refusal
            // is settled before the first retry and stays settled, and spending
            // the budget on it costs the full five seconds on every job rather
            // than once: a failure here keeps the chroot, which re-arms the
            // sweep, which fails here again.
            Err(e) if !is_contended(&e) || std::time::Instant::now() >= deadline => {
                // Reported rather than warned, because failing here means the
                // next job sweeps again rather than inheriting a host nobody
                // is looking at.
                return Err(JailError::StaleCgroup { path, source: e });
            },
            Err(_) => std::thread::sleep(REMOVE_INTERVAL),
        }
    }
}

/// Whether a failed `rmdir` is the kernel saying the cgroup is still occupied.
///
/// `EBUSY` is what a cgroup that still holds a process or a live child cgroup
/// refuses with, and it is the only one of these failures that a moment's
/// waiting can change.
fn is_contended(e: &std::io::Error) -> bool {
    e.raw_os_error() == Some(libc::EBUSY)
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
        // The mems written here derive from a parent with no
        // `cpuset.mems.effective`, which falls back to node 0.
        cpuset_tree_with_mems(effective, "0")
    }

    /// A stand-in tree where the effective memory nodes can also be chosen.
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
        // The last write on this path that used to be taken on trust.
        let (_dir, manager) = cpuset_tree_with_mems("2-7\n", "\n");
        let layout = CpuLayout::with_core_count(8);

        manager.apply_cpuset(&layout).unwrap_err();
    }

    #[test]
    fn a_memory_node_set_that_cannot_be_read_back_fails_the_job() {
        // This asserted a degrade until the rule was written down. The cpus were
        // applied and verified, so the controller is demonstrably delegated;
        // reporting it undelegated because the mems could not be read back would
        // tell the operator something this function just disproved, and the run
        // would go ahead on a memory binding nobody confirmed.
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
        // The kernel intersects the written set with the parent's effective
        // set and reports the result. A successful write proves nothing.
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
    fn a_cgroup_that_already_existed_is_not_ours_to_remove() {
        // Drop removes what this created. Claiming a cgroup that was already
        // there would have it rmdir something belonging to whoever did.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

        let ours = CgroupManager {
            cgroup_path: root.join("ours"),
            created: true,
            signals: JailSignals::unwatched(),
        };
        let theirs = CgroupManager {
            cgroup_path: root.join("theirs"),
            created: false,
            signals: JailSignals::unwatched(),
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
        // A `rmdir` refused for anything but contention is refused the same way
        // five seconds later, and the failure keeps the chroot that names the
        // cgroup, which earns the next job another sweep that fails the same
        // way. Retrying every error kind costs the whole budget per job on a
        // host where nothing is going to change. A non-empty ordinary directory
        // refuses with `ENOTEMPTY`, the way a read-only or unwritable parent
        // refuses with `EROFS` or `EPERM`.
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
    fn a_cgroup_that_will_not_go_away_raises_the_reclaim_signal() {
        // The kernel refuses `rmdir` while a cgroup still holds a process, and
        // a non-empty ordinary directory refuses it the same way. Warning alone
        // would let the chroot that names this cgroup be removed, leaving
        // nothing for a later sweep to find it by.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let signals = JailSignals::unwatched();
        let mut manager = CgroupManager {
            cgroup_path: root.join("stuck"),
            created: true,
            signals: signals.clone(),
        };
        fs::create_dir_all(manager.path()).unwrap();
        fs::write(manager.path().join("cgroup.procs"), "42\n").unwrap();

        manager.cleanup();

        assert!(
            signals.must_keep_chroot(),
            "a cgroup that outlives its job holds the chroot that names it"
        );
        assert!(manager.path().exists());
    }

    #[test]
    fn a_removed_cgroup_leaves_the_signal_alone() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let signals = JailSignals::unwatched();
        let mut manager = CgroupManager {
            cgroup_path: root.join("gone"),
            created: true,
            signals: signals.clone(),
        };
        fs::create_dir_all(manager.path()).unwrap();

        manager.cleanup();

        assert!(!manager.path().exists());
        assert!(
            !signals.must_keep_chroot(),
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
        // The value is written to `cpuset.mems`, so answering node 0 to a read
        // that failed would confine the guest to one node on a host that may
        // have several, and the run would report a number measured under a
        // constraint nobody chose. A file in place of the cgroup directory reads
        // back `ENOTDIR`, the same way an unlistable one reads back `EACCES`.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let not_a_dir = root.join("cgroup");
        fs::write(&not_a_dir, b"in the way").unwrap();

        effective_mems(&not_a_dir).unwrap_err();
    }

    #[test]
    fn a_cpuset_that_cannot_be_verified_is_not_a_degrade() {
        // The write proves the controller is delegated, so a missing effective
        // file cannot mean it is not. Degrading here would tell the operator the
        // controller was never delegated while the run went ahead on a cpuset
        // nobody read back.
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
