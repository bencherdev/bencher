//! Cgroup v2 management for resource limits.

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

use camino::{Utf8Path, Utf8PathBuf};
use slog::{Logger, info, warn};

use crate::RunnerError;
use crate::cpu::CpuLayout;
use crate::error::JailError;
use crate::jail::VmId;
use crate::runner_lock::RunnerLock;

/// Default cgroup v2 mount point.
pub(crate) const CGROUP_ROOT: &str = "/sys/fs/cgroup";

/// Bencher cgroup hierarchy base.
pub(crate) const BENCHER_CGROUP_BASE: &str = "bencher";

/// A cgroup manager for a single run.
pub struct CgroupManager {
    log: Logger,
    cgroup_path: Utf8PathBuf,
    created: bool,
    controllers: Controllers,
}

impl CgroupManager {
    /// Create a new cgroup for the given microVM.
    pub fn new(log: &Logger, vm_id: &VmId) -> Result<Self, RunnerError> {
        Self::new_at(log, Utf8Path::new(CGROUP_ROOT), vm_id)
    }

    /// Takes the cgroup root so a test can stand a scratch cgroup in for it.
    fn new_at(log: &Logger, cgroup_root: &Utf8Path, vm_id: &VmId) -> Result<Self, RunnerError> {
        let parent = cgroup_root.join(BENCHER_CGROUP_BASE);
        let cgroup_path = parent.join(vm_id.as_str());

        fs::create_dir_all(&parent).map_err(|e| JailError::CreateCgroup {
            path: parent.clone(),
            source: e,
        })?;

        // On every Job, since `bencher/` may be new since startup.
        let controllers = ensure_controllers(log, cgroup_root)?;

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
            log: log.clone(),
            cgroup_path,
            created,
            controllers,
        })
    }

    /// Wrap an existing directory without owning it, with every controller
    /// enabled, so a file its test leaves out is the only absence.
    #[cfg(test)]
    #[must_use]
    pub fn detached(cgroup_path: Utf8PathBuf) -> Self {
        Self {
            log: crate::log::discard(),
            cgroup_path,
            created: false,
            controllers: Controllers::enabled(),
        }
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
            return Ok(Cpuset::Unavailable(
                "the CPU layout offers no isolation".to_owned(),
            ));
        }

        let cpuset = layout.benchmark_cpuset();
        if cpuset.is_empty() {
            return Ok(Cpuset::Unavailable(
                "the benchmark core set is empty".to_owned(),
            ));
        }

        // A stat that failed is not an undelegated controller.
        let path = self.cgroup_path.join("cpuset.cpus");
        match path.try_exists() {
            Ok(true) => {},
            Ok(false) => return Ok(Cpuset::Unavailable(self.no_cpuset())),
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

    /// Why the cgroup has no `cpuset.cpus`, naming the controller and the
    /// cgroup that withheld it.
    fn no_cpuset(&self) -> String {
        self.controllers.cpuset_absence().map_or_else(
            || {
                format!(
                    "the cpuset controller is not delegated to {}",
                    self.cgroup_path
                )
            },
            |absence| absence.to_string(),
        )
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
        // Writing a file the controller never created would blame the file.
        if let Some(absence) = self.controllers.memory_absence() {
            return Err(absence.into());
        }
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

    /// SIGKILL every process in this cgroup's subtree and wait until none is
    /// left, keeping the cgroup for [`Self::cleanup`] to remove.
    pub fn empty(&self) -> Result<(), JailError> {
        empty_cgroup(self.cgroup_path.as_std_path())?;
        Ok(())
    }

    /// SIGKILL every process in this cgroup's subtree (best-effort), so a
    /// timed out or cancelled local run leaves no grandchild behind.
    ///
    /// Best effort is sound because [`Self::cleanup`] kills the subtree again.
    pub fn kill_all(&self) {
        if let Err(e) = self.write_file(CGROUP_KILL, "1") {
            warn!(self.log, "Cgroup subtree not killed"; "cgroup" => self.cgroup_path.as_str(), "error" => %e);
        }
    }

    /// Kill whatever is left in the cgroup and remove it, leaving one that
    /// will not go to the next sweep, which finds it under the base.
    pub fn cleanup(&mut self) {
        if !self.created {
            return;
        }
        match kill_cgroup(self.cgroup_path.as_std_path()) {
            Ok(_removed) => self.created = false,
            Err(e) => {
                warn!(self.log, "Cgroup not removed, left for the next sweep";
                    "cgroup" => self.cgroup_path.as_str(),
                    "error" => %e,
                );
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

/// Enable the controllers Jobs use, one per write and in the root before
/// `bencher/`, then disable any other controller `bencher/` enables.
///
/// One per write because a write is all or nothing, and once `bencher/`
/// enables a controller the kernel refuses systemd's removal of it from the
/// root with `EBUSY`.
pub(crate) fn ensure_controllers(
    log: &Logger,
    cgroup_root: &Utf8Path,
) -> Result<Controllers, JailError> {
    ensure_controllers_with(log, cgroup_root, |path, value| fs::write(path, value))
}

/// The writer is injectable because only cgroupfs gives these writes their
/// meaning, so the unit tests stand in a fake of it.
fn ensure_controllers_with<W>(
    log: &Logger,
    cgroup_root: &Utf8Path,
    mut write: W,
) -> Result<Controllers, JailError>
where
    W: FnMut(&Utf8Path, &str) -> std::io::Result<()>,
{
    let bencher = cgroup_root.join(BENCHER_CGROUP_BASE);
    let offered = read_controllers(&cgroup_root.join("cgroup.controllers"))?;

    let mut refusals = Vec::new();
    for controller in CONTROLLERS {
        if !lists(&offered, controller) {
            continue;
        }
        let mut refusal = None;
        for cgroup in [cgroup_root, bencher.as_path()] {
            let path = cgroup.join(SUBTREE_CONTROL);
            if let Err(source) = write(&path, &format!("+{controller}"))
                && refusal.is_none()
            {
                refusal = Some((path, source));
            }
        }
        if let Some((path, source)) = refusal {
            refusals.push((controller, path, source));
        }
    }

    // Read back rather than trusting the writes, since an administrator may
    // have enabled what the writes could not.
    let subtree_control = bencher.join(SUBTREE_CONTROL);
    let enabled = read_controllers(&subtree_control)?;
    for other in enabled
        .split_whitespace()
        .filter(|controller| !CONTROLLERS.contains(controller))
    {
        if let Err(e) = write(&subtree_control, &format!("-{other}")) {
            warn!(log, "Cgroup controller not disabled, so Jobs run with it";
                "controller" => other,
                "path" => subtree_control.as_str(),
                "error" => %e,
            );
        }
    }

    let mut outcome = |controller| {
        if lists(&enabled, controller) {
            return Controller::Enabled;
        }
        if !lists(&offered, controller) {
            return Controller::NotOffered {
                parent: cgroup_root.to_owned(),
            };
        }
        // An unrefused write the read-back does not show was undone by a
        // concurrent writer, which the VM cgroup's own files will reveal.
        refusals
            .iter()
            .position(|(refused, ..)| *refused == controller)
            .map(|index| refusals.swap_remove(index))
            .map_or(Controller::Enabled, |(_, path, source)| {
                Controller::Refused { path, source }
            })
    };
    Ok(Controllers {
        cpuset: outcome(CPUSET),
        memory: outcome(MEMORY),
    })
}

const CPUSET: &str = "cpuset";

const MEMORY: &str = "memory";

/// Never `cpu` or `io`: nothing reads them, and a Job that enabled them would
/// pin them in a root where systemd adds them only while a unit asks.
const CONTROLLERS: [&str; 3] = [CPUSET, MEMORY, "pids"];

const SUBTREE_CONTROL: &str = "cgroup.subtree_control";

/// An unreadable list is not an empty one.
fn read_controllers(path: &Utf8Path) -> Result<String, JailError> {
    fs::read_to_string(path).map_err(|e| JailError::ReadCgroup {
        path: path.to_owned(),
        source: e,
    })
}

/// Matches whole tokens: `cpuset` must not satisfy `cpu`.
fn lists(controllers: &str, controller: &str) -> bool {
    controllers
        .split_whitespace()
        .any(|listed| listed == controller)
}

/// What enabling the controllers came to, for the two whose absence costs a
/// Job a feature: `pids` loses nothing the runner reads.
#[derive(Debug)]
pub(crate) struct Controllers {
    cpuset: Controller,
    memory: Controller,
}

impl Controllers {
    #[cfg(test)]
    pub(crate) fn enabled() -> Self {
        Self {
            cpuset: Controller::Enabled,
            memory: Controller::Enabled,
        }
    }

    #[cfg(test)]
    pub(crate) fn without_cpuset(parent: &Utf8Path) -> Self {
        Self {
            cpuset: Controller::NotOffered {
                parent: parent.to_owned(),
            },
            memory: Controller::Enabled,
        }
    }

    pub(crate) fn cpuset_absence(&self) -> Option<JailError> {
        self.cpuset.absence(CPUSET)
    }

    pub(crate) fn memory_absence(&self) -> Option<JailError> {
        self.memory.absence(MEMORY)
    }
}

#[derive(Debug)]
enum Controller {
    Enabled,
    NotOffered {
        parent: Utf8PathBuf,
    },
    /// The first write the kernel turned down, which names the cause: a
    /// refusal in the root makes the one in `bencher/` follow.
    Refused {
        path: Utf8PathBuf,
        source: std::io::Error,
    },
}

impl Controller {
    fn absence(&self, controller: &'static str) -> Option<JailError> {
        match self {
            Self::Enabled => None,
            Self::NotOffered { parent } => Some(JailError::ControllerNotOffered {
                controller,
                parent: parent.clone(),
            }),
            Self::Refused { path, source } => Some(JailError::ControllerRefused {
                controller,
                path: path.clone(),
                // A cgroupfs refusal is an errno, so the copy loses nothing.
                source: source.raw_os_error().map_or_else(
                    || std::io::Error::from(source.kind()),
                    std::io::Error::from_raw_os_error,
                ),
            }),
        }
    }
}

/// Whether the cpuset actually confined the VMM to the benchmark cores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Cpuset {
    /// The cgroup confines the VMM to the benchmark cores, read back and
    /// confirmed.
    Applied,
    Unavailable(String),
}

/// A cgroup under the real root for the root-only tests, removed bottom up on
/// drop so a failed assertion does not leave it pinning controllers there.
#[cfg(test)]
pub(crate) struct ScratchCgroup(Utf8PathBuf);

#[cfg(test)]
impl ScratchCgroup {
    /// Offered what the real root enables, so the controllers Jobs use are
    /// enabled there first, as the runner does at startup.
    pub(crate) fn new(name: &str) -> Self {
        let real_root = Utf8Path::new(CGROUP_ROOT);
        for controller in CONTROLLERS {
            fs::write(real_root.join(SUBTREE_CONTROL), format!("+{controller}")).unwrap();
        }
        let path = real_root.join(format!("{name}-{}", std::process::id()));
        fs::create_dir(&path).unwrap();
        Self(path)
    }

    pub(crate) fn path(&self) -> &Utf8Path {
        &self.0
    }
}

#[cfg(test)]
impl Drop for ScratchCgroup {
    fn drop(&mut self) {
        // Killed first, since a populated scratch cgroup left under the real
        // root fails every later run on the host.
        drop(kill_cgroup(self.0.as_std_path()));
    }
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

/// What a runner holding the lock does before its first Job: a sandboxed one
/// needs `cgroup.kill`, and every orphan stops at once rather than at the first
/// sandboxed Job.
pub(crate) fn prepare_at_startup(
    log: &Logger,
    lock: &RunnerLock,
    sandboxed: bool,
) -> Result<(), JailError> {
    prepare_at_startup_with(
        log,
        lock,
        sandboxed,
        require_cgroup_kill,
        reclaim_orphaned_cgroups,
    )
}

/// The check and the sweep are parameters, so tests reach every branch
/// without root.
pub(crate) fn prepare_at_startup_with<C, S>(
    log: &Logger,
    lock: &RunnerLock,
    sandboxed: bool,
    check: C,
    sweep: S,
) -> Result<(), JailError>
where
    C: FnOnce() -> Result<(), JailError>,
    S: FnOnce(&Logger) -> Result<usize, JailError>,
{
    // Without the lock a cgroup under the base may hold a root runner's Job.
    if !lock.is_held() {
        return Ok(());
    }
    if sandboxed {
        check()?;
    }
    // Only a warning, since a runner serving Jobs with no sandbox must start
    // anyway, and the next sandboxed Job's sweep tries again.
    if let Err(e) = sweep(log) {
        warn!(log, "Orphaned cgroups not reclaimed at startup"; "error" => %e);
    }
    Ok(())
}

/// Every cgroup but the root has `cgroup.kill` from Linux 5.14, and without it
/// no orphan could be killed and no Job's cgroup emptied.
pub(crate) fn require_cgroup_kill() -> Result<(), JailError> {
    require_cgroup_kill_at(Utf8Path::new(CGROUP_ROOT))
}

/// Probed in the base the kill works in, made if need be, since the root
/// cgroup has no `cgroup.kill` on any kernel.
pub(crate) fn require_cgroup_kill_at(cgroup_root: &Utf8Path) -> Result<(), JailError> {
    let base = cgroup_root.join(BENCHER_CGROUP_BASE);
    fs::create_dir_all(&base).map_err(|source| JailError::CreateCgroup {
        path: base.clone(),
        source,
    })?;
    let kill = base.join(CGROUP_KILL);
    match kill.try_exists() {
        Ok(true) => Ok(()),
        Ok(false) => Err(JailError::NoCgroupKill { path: kill }),
        Err(source) => Err(JailError::ReadCgroup { path: kill, source }),
    }
}

/// Under the runner lock every cgroup under the base is an orphan, whatever left
/// it there, so each is killed and removed; one that holds the runner itself is
/// skipped, since its kill would end the runner.
pub(crate) fn reclaim_orphaned_cgroups(log: &Logger) -> Result<usize, JailError> {
    reclaim_orphaned_cgroups_in(log, Utf8Path::new(CGROUP_ROOT))
}

/// The cgroup root is a parameter, so a test can stand a directory in for it.
pub(crate) fn reclaim_orphaned_cgroups_in(
    log: &Logger,
    cgroup_root: &Utf8Path,
) -> Result<usize, JailError> {
    let own = Utf8Path::new("/proc/self/cgroup");
    let own = fs::read_to_string(own).map_err(|source| JailError::ReadCgroup {
        path: own.to_owned(),
        source,
    })?;
    reclaim_orphaned_cgroups_at(
        log,
        &cgroup_root.join(BENCHER_CGROUP_BASE),
        &own,
        kill_cgroup,
    )
}

/// The base, the runner's own `/proc/self/cgroup`, and the kill are
/// parameters, so tests reach every path without a real orphan.
fn reclaim_orphaned_cgroups_at<K>(
    log: &Logger,
    base: &Utf8Path,
    own: &str,
    kill: K,
) -> Result<usize, JailError>
where
    K: Fn(&Path) -> Result<bool, JailError>,
{
    let read_failed = |source| JailError::ReadCgroup {
        path: base.to_owned(),
        source,
    };
    let entries = match fs::read_dir(base) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => return Err(read_failed(e)),
    };
    let runner = runner_cgroup(own);

    let mut reclaimed = 0;
    // One cgroup that will not go must not leave every other orphan running.
    let mut failure = None;
    for entry in entries {
        let entry = entry.map_err(read_failed)?;
        if !entry.file_type().map_err(read_failed)?.is_dir() {
            continue;
        }
        if runner.is_some_and(|runner| entry.file_name() == runner) {
            continue;
        }
        // The listed path itself, never one rebuilt from its name, which need
        // not be UTF-8.
        let cgroup = entry.path();
        let outcome = is_populated(&cgroup)
            .and_then(|populated| kill(&cgroup).map(|removed| (populated, removed)));
        match outcome {
            Ok((populated, true)) => {
                info!(log, "Reclaimed a stale cgroup";
                    "cgroup" => bencher_logger::capped(cgroup.display()),
                    "populated" => populated,
                );
                reclaimed += 1;
            },
            // Gone before its kill, so something else reclaimed it.
            Ok((_, false)) => {},
            Err(e) => {
                warn!(log, "Stale cgroup not reclaimed";
                    "cgroup" => bencher_logger::capped(cgroup.display()),
                    "error" => %e,
                );
                failure.get_or_insert(e);
            },
        }
    }
    failure.map_or(Ok(reclaimed), Err)
}

/// The cgroup just below the base that a `/proc/self/cgroup` listing places
/// the runner in or under.
fn runner_cgroup(own: &str) -> Option<&str> {
    own.lines()
        .find_map(|line| line.strip_prefix("0::/"))?
        .strip_prefix(BENCHER_CGROUP_BASE)?
        .strip_prefix('/')?
        .split('/')
        .next()
}

const CGROUP_KILL: &str = "cgroup.kill";

/// Far beyond the milliseconds a kill takes, even with a thousand forks racing
/// it.
const KILL_TIMEOUT: Duration = Duration::from_secs(5);

const KILL_INTERVAL: Duration = Duration::from_millis(10);

/// SIGKILL the cgroup's whole subtree with one write, which also reaches every
/// fork and is safe against migration, then wait for it to empty and remove it.
/// `Ok(false)` is a cgroup gone before this removed it.
pub(crate) fn kill_cgroup(cgroup: &Path) -> Result<bool, JailError> {
    kill_cgroup_within(cgroup, KILL_TIMEOUT)
}

fn kill_cgroup_within(cgroup: &Path, timeout: Duration) -> Result<bool, JailError> {
    if !empty_cgroup_within(cgroup, timeout)? {
        return Ok(false);
    }
    match remove_tree(cgroup) {
        Ok(()) => Ok(true),
        Err(_) if is_gone(cgroup) => Ok(false),
        Err(source) => Err(JailError::RemoveCgroup {
            path: shown(cgroup),
            source,
        }),
    }
}

/// The kill alone, leaving the cgroup in place. `Ok(false)` is a cgroup gone
/// before the kill.
fn empty_cgroup(cgroup: &Path) -> Result<bool, JailError> {
    empty_cgroup_within(cgroup, KILL_TIMEOUT)
}

fn empty_cgroup_within(cgroup: &Path, timeout: Duration) -> Result<bool, JailError> {
    if let Err(source) = fs::write(cgroup.join(CGROUP_KILL), "1") {
        return if is_gone(cgroup) {
            Ok(false)
        } else {
            Err(JailError::KillCgroup {
                path: shown(cgroup),
                source,
            })
        };
    }
    let deadline = Instant::now() + timeout;
    while is_populated(cgroup)? {
        if Instant::now() >= deadline {
            return Err(JailError::CgroupNotEmptied {
                path: shown(cgroup),
                timeout_secs: timeout.as_secs(),
            });
        }
        std::thread::sleep(KILL_INTERVAL);
    }
    Ok(true)
}

/// Children first, since `rmdir` refuses a cgroup that still has one.
fn remove_tree(cgroup: &Path) -> std::io::Result<()> {
    for entry in fs::read_dir(cgroup)? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            remove_tree(&entry.path())?;
        }
    }
    fs::remove_dir(cgroup)
}

/// Only a stat that succeeded counts as gone.
fn is_gone(cgroup: &Path) -> bool {
    cgroup.try_exists().is_ok_and(|exists| !exists)
}

/// Whether the cgroup or any cgroup below it holds a live process; a cgroup
/// gone since it was listed holds none.
fn is_populated(cgroup: &Path) -> Result<bool, JailError> {
    let path = cgroup.join("cgroup.events");
    let events = match fs::read_to_string(&path) {
        Ok(events) => events,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(source) => {
            return Err(JailError::ReadCgroup {
                path: shown(&path),
                source,
            });
        },
    };
    match events
        .lines()
        .find_map(|line| line.strip_prefix("populated "))
    {
        Some("0") => Ok(false),
        Some("1") => Ok(true),
        _ => Err(JailError::ReadCgroup {
            path: shown(&path),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("no populated line in {events:?}"),
            ),
        }),
    }
}

/// Lossy, so only for an error message: a name that is not UTF-8 is shown, never
/// used.
fn shown(path: &Path) -> Utf8PathBuf {
    Utf8PathBuf::from(path.to_string_lossy().into_owned())
}

/// Matches whole lines: pid `7` must not be satisfied by pid `70`.
pub(crate) fn procs_contains_pid(procs: &str, pid: u32) -> bool {
    procs
        .lines()
        .any(|line| line.trim().parse::<u32>() == Ok(pid))
}

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;
    use std::os::unix::ffi::OsStrExt as _;

    use camino::Utf8PathBuf;

    use super::*;
    use crate::log::discard;

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

        assert!(matches!(
            manager.apply_cpuset(&layout).unwrap(),
            Cpuset::Unavailable(_)
        ));
    }

    fn not_offered_by(parent: &Utf8Path) -> Controller {
        Controller::NotOffered {
            parent: parent.to_owned(),
        }
    }

    #[test]
    fn a_missing_cpuset_names_the_controller_and_the_root() {
        // The errno of an enable write names a file that exists and sends the
        // operator to the wrong place.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let manager = CgroupManager {
            log: discard(),
            cgroup_path: root.join("bencher").join("vm"),
            created: false,
            controllers: Controllers {
                cpuset: not_offered_by(&root),
                memory: Controller::Enabled,
            },
        };

        let Cpuset::Unavailable(reason) = manager
            .apply_cpuset(&CpuLayout::with_core_count(8))
            .unwrap()
        else {
            panic!("a cgroup with no cpuset files cannot confine anything");
        };

        assert!(
            reason.contains(&format!("{root} does not offer the cpuset controller")),
            "names the controller and the root that does not offer it: {reason}"
        );
    }

    #[test]
    fn swap_is_not_limited_through_a_memory_controller_the_root_does_not_offer() {
        // Writing `memory.swap.max` anyway would report the errno of a file the
        // controller never created.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let manager = CgroupManager {
            log: discard(),
            cgroup_path: root.clone(),
            created: false,
            controllers: Controllers {
                cpuset: Controller::Enabled,
                memory: not_offered_by(&root),
            },
        };

        let err = manager.disable_swap().unwrap_err().to_string();

        assert!(
            err.contains("memory") && err.contains(root.as_str()),
            "names the controller and the root that does not offer it: {err}"
        );
        assert!(!root.join("memory.swap.max").exists(), "nothing is written");
    }

    /// Every controller the kernel has, as a cgroup v2 root lists them.
    const EVERY_CONTROLLER: &str = "cpuset cpu io memory hugetlb pids rdma misc";

    /// A cgroup root and its `bencher/` on a tempfile tree, written to the way
    /// cgroupfs takes `cgroup.subtree_control` writes.
    struct FakeCgroupfs {
        _dir: tempfile::TempDir,
        root: Utf8PathBuf,
    }

    impl FakeCgroupfs {
        fn new(offered: &str, root_enabled: &str, bencher_enabled: &str) -> Self {
            let dir = tempfile::tempdir().unwrap();
            let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
            fs::write(root.join("cgroup.controllers"), offered).unwrap();
            fs::write(root.join(SUBTREE_CONTROL), root_enabled).unwrap();
            fs::create_dir(root.join(BENCHER_CGROUP_BASE)).unwrap();
            fs::write(
                root.join(BENCHER_CGROUP_BASE).join(SUBTREE_CONTROL),
                bencher_enabled,
            )
            .unwrap();
            Self { _dir: dir, root }
        }

        fn root_subtree_control(&self) -> Utf8PathBuf {
            self.root.join(SUBTREE_CONTROL)
        }

        fn bencher_subtree_control(&self) -> Utf8PathBuf {
            self.root.join(BENCHER_CGROUP_BASE).join(SUBTREE_CONTROL)
        }

        /// All or nothing, and `ENOENT` for a controller the parent does not
        /// offer: `bencher/` is offered only what the root enables.
        fn write(&self, path: &Utf8Path, value: &str) -> std::io::Result<()> {
            let offered = if *path == self.root_subtree_control() {
                fs::read_to_string(self.root.join("cgroup.controllers"))?
            } else {
                fs::read_to_string(self.root_subtree_control())?
            };
            let mut enabled: Vec<String> = fs::read_to_string(path)?
                .split_whitespace()
                .map(str::to_owned)
                .collect();
            for token in value.split_whitespace() {
                if let Some(controller) = token.strip_prefix('+') {
                    if !lists(&offered, controller) {
                        return Err(std::io::Error::from_raw_os_error(libc::ENOENT));
                    }
                    if !enabled.iter().any(|listed| listed == controller) {
                        enabled.push(controller.to_owned());
                    }
                } else if let Some(controller) = token.strip_prefix('-') {
                    enabled.retain(|listed| listed != controller);
                } else {
                    return Err(std::io::Error::from_raw_os_error(libc::EINVAL));
                }
            }
            fs::write(path, enabled.join(" "))
        }

        fn enabled(path: &Utf8Path) -> std::collections::BTreeSet<String> {
            fs::read_to_string(path)
                .unwrap()
                .split_whitespace()
                .map(str::to_owned)
                .collect()
        }
    }

    fn ensure_logged(cgroupfs: &FakeCgroupfs) -> (Controllers, Vec<(Utf8PathBuf, String)>) {
        let mut writes = Vec::new();
        let controllers = ensure_controllers_with(&discard(), &cgroupfs.root, |path, value| {
            writes.push((path.to_owned(), value.to_owned()));
            cgroupfs.write(path, value)
        })
        .unwrap();
        (controllers, writes)
    }

    #[test]
    fn controllers_are_enabled_one_per_write_and_root_first() {
        // From systemd's idle root: a write carrying a controller the root does
        // not enable fails whole, which kept `cpuset` from every Job, and
        // `bencher/` can enable only what the root already has.
        let cgroupfs = FakeCgroupfs::new(EVERY_CONTROLLER, "memory pids", "");

        let (controllers, writes) = ensure_logged(&cgroupfs);

        let root = cgroupfs.root_subtree_control();
        let bencher = cgroupfs.bencher_subtree_control();
        let expected: Vec<(Utf8PathBuf, String)> = ["+cpuset", "+memory", "+pids"]
            .into_iter()
            .flat_map(|write| [(root.clone(), write), (bencher.clone(), write)])
            .map(|(path, write)| (path, write.to_owned()))
            .collect();
        assert_eq!(writes, expected);
        assert!(controllers.cpuset_absence().is_none());
        assert!(controllers.memory_absence().is_none());
    }

    #[test]
    fn cpu_and_io_are_never_enabled() {
        // Nothing reads them, and enabling them in `bencher/` pins them in a
        // root where systemd means them to come and go.
        let cgroupfs = FakeCgroupfs::new(EVERY_CONTROLLER, EVERY_CONTROLLER, "");

        let (_, writes) = ensure_logged(&cgroupfs);

        for (path, value) in &writes {
            assert!(
                !value
                    .split_whitespace()
                    .any(|token| token == "+cpu" || token == "+io"),
                "{path} was written {value}"
            );
        }
    }

    #[test]
    fn bencher_is_left_with_exactly_the_controllers_jobs_use() {
        // An older runner left `cpu` enabled in `bencher/`, which holds it in
        // the root against systemd's removal for as long as `bencher/` lives.
        let cgroupfs = FakeCgroupfs::new(EVERY_CONTROLLER, "cpu memory pids", "cpu memory pids");

        let (_, writes) = ensure_logged(&cgroupfs);

        assert_eq!(
            FakeCgroupfs::enabled(&cgroupfs.bencher_subtree_control()),
            ["cpuset", "memory", "pids"].map(str::to_owned).into()
        );
        let root = cgroupfs.root_subtree_control();
        assert!(
            writes
                .iter()
                .all(|(path, value)| !(*path == root && value.starts_with('-'))),
            "the root is systemd's to prune: {writes:?}"
        );
    }

    #[test]
    fn a_controller_enabled_by_someone_else_counts_although_the_writes_were_refused() {
        // An administrator may enable `bencher/` for a runner without the
        // privilege to write the root.
        let cgroupfs =
            FakeCgroupfs::new(EVERY_CONTROLLER, "cpuset memory pids", "cpuset memory pids");

        let controllers = ensure_controllers_with(&discard(), &cgroupfs.root, |_, _| {
            Err(std::io::Error::from_raw_os_error(libc::EACCES))
        })
        .unwrap();

        assert!(controllers.cpuset_absence().is_none());
        assert!(controllers.memory_absence().is_none());
    }

    #[test]
    fn a_controller_the_root_does_not_offer_costs_only_its_own_feature() {
        let cgroupfs = FakeCgroupfs::new("cpu io memory pids", "memory pids", "");

        let (controllers, _) = ensure_logged(&cgroupfs);

        let absence = controllers.cpuset_absence().unwrap().to_string();
        assert!(
            absence.contains("cpuset") && absence.contains(cgroupfs.root.as_str()),
            "names the controller and the root: {absence}"
        );
        assert!(
            !absence.contains("No such file"),
            "a declared absence, not the errno of a write: {absence}"
        );
        assert!(
            controllers.memory_absence().is_none(),
            "the swap limit does not depend on cpuset"
        );
    }

    #[test]
    fn a_refused_controller_is_named_with_the_write_that_caused_it() {
        // The refusal in the root makes the one in `bencher/` follow with
        // `ENOENT`, which names nothing useful.
        let cgroupfs = FakeCgroupfs::new(EVERY_CONTROLLER, "memory pids", "");
        let root = cgroupfs.root_subtree_control();

        let controllers = ensure_controllers_with(&discard(), &cgroupfs.root, |path, value| {
            if *path == root && value == "+cpuset" {
                Err(std::io::Error::from_raw_os_error(libc::EACCES))
            } else {
                cgroupfs.write(path, value)
            }
        })
        .unwrap();

        let absence = controllers.cpuset_absence().unwrap().to_string();
        assert!(
            absence.contains("cpuset")
                && absence.contains(root.as_str())
                && absence.contains("Permission denied"),
            "{absence}"
        );
    }

    #[test]
    fn an_unreadable_controller_list_is_not_an_empty_one() {
        // Read as empty, it would declare every controller not offered.
        let cgroupfs = FakeCgroupfs::new(EVERY_CONTROLLER, "memory pids", "");
        fs::remove_file(cgroupfs.root.join("cgroup.controllers")).unwrap();

        ensure_controllers_with(&discard(), &cgroupfs.root, |path, value| {
            cgroupfs.write(path, value)
        })
        .unwrap_err();
    }

    #[test]
    #[expect(clippy::print_stderr, reason = "a skipped test says why")]
    fn a_root_systemd_left_idle_still_gives_the_vm_cgroup_its_controllers() {
        // Stands a scratch cgroup in for a root systemd keeps at `memory pids`
        // between logins, so a host that enables every controller in its real
        // root cannot hide the difference.
        if crate::jail::current_euid() != 0 {
            eprintln!(
                "skipped a_root_systemd_left_idle_still_gives_the_vm_cgroup_its_controllers: building a cgroup needs root"
            );
            return;
        }
        let scratch = ScratchCgroup::new("bencher-runner-idle-root");
        for controller in ["+memory", "+pids"] {
            fs::write(scratch.path().join(SUBTREE_CONTROL), controller).unwrap();
        }

        let manager = CgroupManager::new_at(&discard(), scratch.path(), &VmId::new()).unwrap();

        assert!(
            manager.path().join("cpuset.cpus.effective").exists(),
            "the VM cgroup has the cpuset controller"
        );
        assert!(
            manager.path().join("memory.swap.max").exists(),
            "the VM cgroup has the memory controller"
        );
        let removal = fs::write(scratch.path().join(SUBTREE_CONTROL), "-cpuset").unwrap_err();
        assert_eq!(
            removal.raw_os_error(),
            Some(libc::EBUSY),
            "a rewrite of the root cannot take cpuset away: {removal}"
        );
        drop(manager);
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
    fn a_cgroup_that_already_existed_is_not_ours_to_kill() {
        // Killing a cgroup this did not create would end someone else's work.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

        let ours = CgroupManager {
            log: discard(),
            cgroup_path: root.join("ours"),
            created: true,
            controllers: Controllers::enabled(),
        };
        let theirs = CgroupManager {
            log: discard(),
            cgroup_path: root.join("theirs"),
            created: false,
            controllers: Controllers::enabled(),
        };
        fs::create_dir_all(ours.path()).unwrap();
        fs::create_dir_all(theirs.path()).unwrap();
        let ours_path = ours.path().to_owned();
        let theirs_path = theirs.path().to_owned();

        drop(ours);
        drop(theirs);

        assert_eq!(
            fs::read_to_string(ours_path.join(CGROUP_KILL)).unwrap(),
            "1",
            "we kill what we created"
        );
        assert!(
            !theirs_path.join(CGROUP_KILL).exists(),
            "we leave what we did not create"
        );
    }

    #[test]
    #[expect(clippy::print_stderr, reason = "a skipped test says why")]
    fn a_cgroup_of_forking_orphans_is_emptied_and_removed() {
        // Kills a kill by pid, which misses every fork since the listing and
        // every process in a cgroup below the one listed.
        use std::io::Write as _;
        use std::os::unix::process::ExitStatusExt as _;
        use std::process::{Child, Command, Stdio};

        if crate::jail::current_euid() != 0 {
            eprintln!(
                "skipped a_cgroup_of_forking_orphans_is_emptied_and_removed: a cgroup needs root"
            );
            return;
        }
        let scratch = ScratchCgroup::new("bencher-runner-kill");
        fs::write(scratch.path().join(SUBTREE_CONTROL), "+pids").unwrap();
        let orphans = scratch.path().join("orphans");
        let nested = orphans.join("nested");
        fs::create_dir(&orphans).unwrap();
        fs::create_dir(&nested).unwrap();
        // A cap, so a broken kill cannot fork the host to death.
        fs::write(orphans.join("pids.max"), "8192").unwrap();
        // Blocks on its input until it is placed, so every fork lands inside.
        let fork_in = |cgroup: &Utf8Path| -> Child {
            let mut forker = Command::new("/bin/sh")
                .args([
                    "-c",
                    "read go; while :; do /bin/true; (setsid sleep 300 &); done",
                ])
                .stdin(Stdio::piped())
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .spawn()
                .unwrap();
            fs::write(cgroup.join("cgroup.procs"), forker.id().to_string()).unwrap();
            forker.stdin.take().unwrap().write_all(b"go\n").unwrap();
            forker
        };
        let mut forkers = [fork_in(&orphans), fork_in(&nested)];
        std::thread::sleep(Duration::from_millis(500));
        assert!(
            is_populated(orphans.as_std_path()).unwrap(),
            "the orphans are running"
        );

        assert!(
            kill_cgroup(orphans.as_std_path()).unwrap(),
            "this kill removed it"
        );

        assert!(!orphans.exists(), "an emptied cgroup is removed");
        for forker in &mut forkers {
            assert_eq!(forker.wait().unwrap().signal(), Some(libc::SIGKILL));
        }
    }

    fn cgroup_base(cgroups: &[&str]) -> (tempfile::TempDir, Utf8PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let base = Utf8PathBuf::try_from(dir.path().join(BENCHER_CGROUP_BASE)).unwrap();
        fs::create_dir(&base).unwrap();
        for cgroup in cgroups {
            fs::create_dir_all(base.join(cgroup)).unwrap();
        }
        // A control file of the base itself, which is never a cgroup to kill.
        fs::write(base.join("cgroup.procs"), "").unwrap();
        (dir, base)
    }

    #[test]
    fn every_orphan_is_killed_but_the_cgroup_holding_the_runner() {
        // Kills a sweep that skips an orphan, or kills the runner along with
        // one.
        let (_dir, base) = cgroup_base(&["vm-1", "local-2", "own"]);
        let killed = std::cell::RefCell::new(Vec::new());

        let reclaimed =
            reclaim_orphaned_cgroups_at(&discard(), &base, "0::/bencher/own/nested\n", |cgroup| {
                killed
                    .borrow_mut()
                    .push(cgroup.file_name().unwrap().to_owned());
                Ok(true)
            })
            .unwrap();

        let mut killed = killed.into_inner();
        killed.sort();
        assert_eq!(killed, ["local-2", "vm-1"]);
        assert_eq!(reclaimed, 2);
    }

    #[test]
    fn an_orphan_that_will_not_go_fails_the_sweep_after_the_rest() {
        // Kills a sweep that stops at the first failure, leaving later orphans
        // running, or that reports success over a survivor. Two of three fail,
        // so a failure comes before the last whatever the listing's order.
        let (_dir, base) = cgroup_base(&["a", "b", "c"]);
        let killed = std::cell::RefCell::new(0);

        let swept = reclaim_orphaned_cgroups_at(&discard(), &base, "0::/\n", |cgroup| {
            *killed.borrow_mut() += 1;
            if cgroup.file_name() == Some(OsStr::new("b")) {
                Ok(true)
            } else {
                Err(JailError::CgroupNotEmptied {
                    path: shown(cgroup),
                    timeout_secs: 5,
                })
            }
        });

        assert!(
            matches!(swept, Err(JailError::CgroupNotEmptied { .. })),
            "{swept:?}"
        );
        assert_eq!(killed.into_inner(), 3, "every orphan is tried");
    }

    #[test]
    fn a_base_that_cannot_be_listed_is_not_an_empty_one() {
        let (_dir, base) = cgroup_base(&[]);
        let never = |_cgroup: &Path| -> Result<bool, JailError> { panic!("nothing to kill") };

        assert_eq!(
            reclaim_orphaned_cgroups_at(&discard(), &base.join("absent"), "", never).unwrap(),
            0
        );
        reclaim_orphaned_cgroups_at(&discard(), &base.join("cgroup.procs"), "", never).unwrap_err();
    }

    #[test]
    fn a_cgroup_is_empty_only_when_its_events_say_so() {
        // Kills a reading that takes a missing or garbled line for an empty
        // cgroup, which would remove one still running.
        let dir = tempfile::tempdir().unwrap();
        let cgroup = dir.path();

        for (events, populated) in [("populated 1\nfrozen 0\n", true), ("populated 0\n", false)] {
            fs::write(cgroup.join("cgroup.events"), events).unwrap();
            assert_eq!(is_populated(cgroup).unwrap(), populated, "{events}");
        }
        fs::write(cgroup.join("cgroup.events"), "frozen 0\n").unwrap();
        is_populated(cgroup).unwrap_err();
        assert!(
            !is_populated(&cgroup.join("gone")).unwrap(),
            "a cgroup gone since it was listed holds nothing"
        );
    }

    #[test]
    fn an_orphan_whose_name_is_not_utf8_is_killed_not_skipped() {
        // Kills a sweep that rebuilds the path from a lossy name, which names
        // nothing, so the kill finds it gone and the orphan runs on.
        let (_dir, base) = cgroup_base(&[]);
        let odd = base.as_std_path().join(OsStr::from_bytes(b"odd-\xff"));
        fs::create_dir(&odd).unwrap();
        let killed = std::cell::RefCell::new(Vec::new());

        let reclaimed = reclaim_orphaned_cgroups_at(&discard(), &base, "0::/\n", |cgroup| {
            killed.borrow_mut().push(cgroup.to_owned());
            Ok(true)
        })
        .unwrap();

        assert_eq!(killed.into_inner(), [odd], "the kill gets the listed path");
        assert_eq!(reclaimed, 1);
    }

    #[test]
    fn only_a_cgroup_this_sweep_removed_counts_as_reclaimed() {
        // Kills a sweep that counts, and logs, a cgroup gone before its kill.
        let (_dir, base) = cgroup_base(&["gone", "killed"]);

        let reclaimed = reclaim_orphaned_cgroups_at(&discard(), &base, "0::/\n", |cgroup| {
            Ok(cgroup.file_name() == Some(OsStr::new("killed")))
        })
        .unwrap();

        assert_eq!(reclaimed, 1);
    }

    #[test]
    fn a_cgroup_gone_before_its_kill_is_not_an_error() {
        // Kills a kill that fails the sweep over a cgroup that is already gone.
        let dir = tempfile::tempdir().unwrap();

        assert!(!kill_cgroup(&dir.path().join("gone")).unwrap());
    }

    #[test]
    fn a_cgroup_that_will_not_empty_is_given_up_on_at_the_bound() {
        // Kills a wait for `populated 0` that ignores its bound, which would
        // hold a sweep, and every Job after it, for ever.
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("cgroup.events"), "populated 1\n").unwrap();
        let started = Instant::now();

        let err = kill_cgroup_within(dir.path(), Duration::from_millis(50)).unwrap_err();

        assert!(matches!(err, JailError::CgroupNotEmptied { .. }), "{err}");
        assert!(
            started.elapsed() < Duration::from_secs(5),
            "{:?}",
            started.elapsed()
        );
    }

    #[test]
    fn a_kernel_without_cgroup_kill_is_refused_by_name() {
        // Kills a startup that lets a sandboxed runner serve one Job on a kernel
        // that cannot kill the next one's orphans.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

        let err = require_cgroup_kill_at(&root).unwrap_err();

        assert!(matches!(err, JailError::NoCgroupKill { .. }), "{err}");
        assert!(err.to_string().contains("Linux 5.14"), "{err}");
        fs::write(root.join(BENCHER_CGROUP_BASE).join(CGROUP_KILL), "").unwrap();
        require_cgroup_kill_at(&root).unwrap();
    }

    #[test]
    #[expect(clippy::print_stderr, reason = "a skipped test says why")]
    fn an_orphan_whose_name_is_not_utf8_is_emptied_and_removed() {
        // Kills a sweep that loses a cgroup whose name is not UTF-8, which
        // only root can make and which holds a live process.
        use std::os::unix::process::ExitStatusExt as _;
        use std::process::{Command, Stdio};

        if crate::jail::current_euid() != 0 {
            eprintln!(
                "skipped an_orphan_whose_name_is_not_utf8_is_emptied_and_removed: a cgroup needs root"
            );
            return;
        }
        let scratch = ScratchCgroup::new("bencher-runner-odd");
        let odd = scratch
            .path()
            .as_std_path()
            .join(OsStr::from_bytes(b"odd-\xff"));
        fs::create_dir(&odd).unwrap();
        let mut orphan = Command::new("sleep")
            .arg("300")
            .stdin(Stdio::null())
            .spawn()
            .unwrap();
        fs::write(odd.join("cgroup.procs"), orphan.id().to_string()).unwrap();

        let reclaimed =
            reclaim_orphaned_cgroups_at(&discard(), scratch.path(), "0::/\n", kill_cgroup);

        let ended = (0..500).find_map(|_| {
            let status = orphan.try_wait().unwrap();
            if status.is_none() {
                std::thread::sleep(Duration::from_millis(10));
            }
            status
        });
        if ended.is_none() {
            drop(orphan.kill());
            drop(orphan.wait());
        }
        assert_eq!(reclaimed.unwrap(), 1);
        assert!(!odd.exists(), "the cgroup is removed");
        assert_eq!(
            ended.and_then(|status| status.signal()),
            Some(libc::SIGKILL)
        );
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
