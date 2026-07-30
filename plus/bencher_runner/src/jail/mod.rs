//! Confinement for Firecracker microVMs.
//!
//! Managed runners execute arbitrary code submitted by anyone, so the VMM must
//! not inherit the runner's root. This module owns everything that confines it:
//! the persistent state directory the chroots are built under, the empty
//! network namespace the VMM joins, and the cgroup that both places it on the
//! benchmark cores and bounds its resources.
//!
//! # What a failing step does
//!
//! Teardown is where this module is easiest to get wrong, because every step of
//! it runs after the thing it is cleaning up already happened, so a failure has
//! no obvious caller to tell. Measurement is easiest to get wrong for the
//! mirror-image reason: a read that fails still leaves a value in hand, and a
//! plausible one. Several separate defects here were the same mistake in those
//! two dresses, a fallible step whose failure either never reached the mechanism
//! built to retry it or was reported as a state that had been observed. So every
//! fallible step does exactly one of three things, and adding a step means
//! choosing which, rather than reaching for `eprintln!` or a fallback value:
//!
//! - **Fails the job.** The host cannot be trusted to measure. Used wherever
//!   something may still be running on the benchmark cores, wherever the runner
//!   could not establish that nothing is, and wherever a confinement it applied
//!   could not be read back. Recoverable by construction: nothing latches, so
//!   the next job tries the whole thing again.
//! - **Arms the retry, or declares the absence.** For teardown that is
//!   [`ReclaimFailed`], read by [`HostPreparation::ensure`], used where the cost
//!   is disk rather than a contended benchmark and where the code has no caller
//!   to report to at all, which is every step reached from a `Drop`. For
//!   measurement it is `Cpuset::Unavailable` or an absent metric: isolation the
//!   host cannot offer is reported as isolation this run did not have, loudly,
//!   and never as a number.
//! - **Ignored.** Only where the failure is itself the answer, or where a later
//!   step is guaranteed to catch it. Each one below says which.
//!
//! What no step does is answer a question it could not ask. An unreadable file is
//! not an empty one, an unstattable path is not an absent one, and a field that
//! was never read is not zero.
//!
//! | Step | On failure |
//! |---|---|
//! | [`HostPreparation::ensure`]: the root check | fails the job |
//! | [`HostPreparation::ensure`]: creating the state directory | fails the job |
//! | [`HostPreparation::ensure`]: reading `/etc/passwd`, `/etc/group` | ignored: the check is advisory and cannot see a directory service anyway |
//! | [`HostPreparation::ensure`]: taking the jail lock | fails the job |
//! | [`HostPreparation::ensure`]: a sweep that returns an error | fails the job |
//! | [`HostPreparation::ensure`]: a sweep that leaves a chroot behind | arms the retry |
//! | `sweep_jails`: the jail parent is absent | nothing to sweep |
//! | `sweep_jails`: the jail parent cannot be read | fails the job |
//! | `sweep_jails`: an entry cannot be read | fails the job |
//! | `sweep_jails`: an entry's kind cannot be read | fails the job: it may be a jail |
//! | `sweep_jails`: a name that is not UTF-8 | ignored: every name here is a UUID this runner minted, so it is not ours |
//! | `sweep_jails`: the reap reports a live VMM | fails the job |
//! | `sweep_jails`: the reap could not examine the jail | fails the job |
//! | `sweep_jails`: removing the cgroup | fails the job, and the chroot is kept because its name is the cgroup's only handle |
//! | `sweep_jails`: removing the chroot | arms the retry |
//! | `reap_jailed_vmm`: the jail root is absent | clear: nothing can be chrooted into a directory that is not there |
//! | `reap_jailed_vmm`: the jail root cannot be stat'ed | reported unexaminable, which fails the job |
//! | `reap_jailed_vmm`: `/proc` cannot be listed | reported unexaminable |
//! | `reap_jailed_vmm`: a `/proc/<pid>/root` cannot be read | ignored: that process is gone or is not this jail's |
//! | `reap_jailed_vmm`: pinning or signalling the VMM | reported still running, which fails the job |
//! | `reap_jailed_vmm`: a VMM that will not exit | reported still running |
//! | `reap_jailed_vmm`: the rescan bound runs out | reported still running |
//! | `reap_jailed_vmm`: a `/proc/<pid>/status` that cannot be read | ignored: the process is gone, which is what was being asked |
//! | `JailDir` teardown: the chroot is already gone | ignored: that is the goal state |
//! | `JailDir` teardown: removing the chroot | arms the retry |
//! | `JailDir` teardown: the retry is already armed | keeps the chroot, since its name is the cgroup's only handle |
//! | `CgroupManager` teardown: `rmdir` of the cgroup | arms the retry, this job's and the runner's both |
//! | `CgroupManager` teardown: killing the cgroup's survivors | ignored: whatever survives is what makes the `rmdir` above fail, which arms the retry |
//!
//! And the same three columns for the reads that decide what a run measured:
//!
//! | Step | On failure |
//! |---|---|
//! | `apply_cpuset`: no isolation in the layout, or an empty core set | declares the absence |
//! | `apply_cpuset`: `cpuset.cpus` is absent | declares the absence: the controller is not delegated |
//! | `apply_cpuset`: `cpuset.cpus` cannot be stat'ed | fails the job: an error is not an absence |
//! | `apply_cpuset`: the kernel rejects a cpuset write | fails the job: half-applied confinement |
//! | `apply_cpuset`: the parent's node set is absent | node 0, which is what an undelegated controller means |
//! | `apply_cpuset`: the parent's node set cannot be read | fails the job: this value is written, so a guess confines the guest's memory |
//! | `apply_cpuset`: the effective set cannot be read back | fails the job: the read back *is* the mechanism, and the write already proved the controller delegated |
//! | `apply_cpuset`: the kernel narrowed the set | fails the job |
//! | `enable_controllers`: `cgroup.subtree_control` cannot be read | fails the job: an unreadable list is not an empty one |
//! | `BencherPartition::apply`: any read or write in the partition path | declares the absence: the level achieved is reported, down to `member` |
//! | `CpuLayout::detect`: the online CPU list cannot be read or parsed | declares the absence: the counted layout is announced as a guess |
//! | `CpuLayout::detect`: the core count cannot be read | one core, which reports as a layout with no isolation |
//! | `metrics`: a cgroup that is not there | no metrics, reported as absent |
//! | `metrics`: a field that cannot be read or parsed | absent, never zero |
//! | `tuning::preflight`: any check that cannot be performed | ignored: advisory only, and nothing reads it to decide whether the host can measure. A quiet preflight is not evidence of a quiet host |

#[cfg(target_os = "linux")]
mod cgroup;
#[cfg(target_os = "linux")]
pub mod chroot;
#[cfg(target_os = "linux")]
pub mod lock;
#[cfg(target_os = "linux")]
pub mod netns;
#[cfg(target_os = "linux")]
pub mod paths;
#[cfg(target_os = "linux")]
pub mod reap;
#[cfg(target_os = "linux")]
pub mod state;

#[cfg(target_os = "linux")]
pub(crate) use cgroup::{BENCHER_CGROUP_BASE, effective_mems};
#[cfg(target_os = "linux")]
pub use cgroup::{CgroupManager, Cpuset};
#[cfg(target_os = "linux")]
pub use chroot::JailDir;
#[cfg(target_os = "linux")]
pub use lock::JailLock;
#[cfg(target_os = "linux")]
pub use paths::{ChrootPath, HostPath, JailFile, JailPaths, SocketPath};
#[cfg(target_os = "linux")]
pub use state::StateDir;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// Default location of the runner's persistent state directory.
pub const DEFAULT_STATE_DIR: &str = "/var/lib/bencher-runner";

/// Default unprivileged uid the jailed Firecracker VMM runs as.
///
/// One dedicated id, not one per job: jobs run serially and each gets a fresh
/// chroot that is swept, so a per-job allocator adds a scheme without closing
/// a live vector.
///
/// Serially per runner, which is what the jail lock enforces. Two runners with
/// different `--state-dir` values hold different locks and can therefore have
/// jobs in flight at the same time, as the same uid, and that is a supported
/// configuration: the network namespace module is built around it. Two VMMs
/// sharing a uid can signal each other and, where `ptrace_scope` permits, trace
/// each other, which is the hazard [`JailUser`] describes. A per-runner id would
/// close it, and is not attempted here: the ids would have to be allocated
/// without coordination between runners that by construction do not know about
/// each other, which trades a known hazard for an unknown one. An operator
/// running two runners on one host should give them different `--jail-uid`
/// values, which is what that flag is for.
///
/// The number is Bencher's historic default self-hosted API server port,
/// retired in favor of the IANA-registered 6610, so it reads as a project
/// convention rather than an arbitrary pick. It also lands in the unallocated
/// gap between the ids `systemd-homed` claims (60001-60513) and the
/// `DynamicUser` range (61184-65519), clear of both the regular user range and
/// `nobody` (65534). No passwd entry is needed: the jailer sets the numeric id
/// directly.
///
/// This is a default rather than a fixed constant because self-hosted runners
/// land on hardware whose id allocation Bencher does not control. See
/// `--jail-uid`.
pub const DEFAULT_JAIL_UID: u32 = 61016;

/// Default unprivileged gid the jailed Firecracker VMM runs as.
///
/// See [`DEFAULT_JAIL_UID`].
pub const DEFAULT_JAIL_GID: u32 = 61016;

/// The unprivileged uid and gid the jailed Firecracker VMM drops to.
///
/// A host process owning this uid can signal the VMM and, depending on the
/// `ptrace` scope, trace it, so it must not be an id the host allocates to
/// anything else.
///
/// The fields are private because `0` must never reach them. The whole
/// sandbox is built by dropping privilege, so a jail user of root is not a
/// weaker jail, it is no jail at all: untrusted code would run against a root
/// VMM, which is the one thing the confinement exists to prevent. An operator
/// hitting a permission error is exactly the person most likely to try it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JailUser {
    uid: u32,
    gid: u32,
}

impl JailUser {
    /// Build a jail user, rejecting root.
    pub fn new(uid: u32, gid: u32) -> Result<Self, crate::error::JailError> {
        if uid == 0 {
            return Err(crate::error::JailError::PrivilegedJailUser { field: "uid" });
        }
        if gid == 0 {
            return Err(crate::error::JailError::PrivilegedJailUser { field: "gid" });
        }
        Ok(Self { uid, gid })
    }

    /// The uid the VMM drops to.
    #[must_use]
    pub fn uid(self) -> u32 {
        self.uid
    }

    /// The gid the VMM drops to.
    #[must_use]
    pub fn gid(self) -> u32 {
        self.gid
    }
}

impl Default for JailUser {
    fn default() -> Self {
        Self {
            uid: DEFAULT_JAIL_UID,
            gid: DEFAULT_JAIL_GID,
        }
    }
}

/// Require an absolute state directory.
///
/// The invariant belongs to the library, not to one command line. The path
/// reaches the jailer as `--chroot-base-dir`, which the jailer resolves against
/// its own working directory rather than the runner's, so a relative value
/// builds the chroot somewhere the runner does not look: the sweep never reaches
/// it and the lock does not protect it. Every caller that hands this crate a
/// state directory is exposed to that, and the CLI is only one of them.
///
/// Shared with the argument parser rather than restated there, so the rule has
/// one implementation and the operator still hears about it before the runner
/// starts.
pub fn check_absolute_state_dir(path: &camino::Utf8Path) -> Result<(), crate::error::JailError> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(crate::error::JailError::RelativeStateDir {
            path: path.to_owned(),
        })
    }
}

/// The identity of one microVM.
///
/// The same string is the jailer's `--id`, the name of the chroot directory,
/// and the name of the cgroup, by construction. Naming it once keeps the three
/// from drifting, and keeps a bare directory name read off the filesystem from
/// being mistaken for an identity that was minted.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmId(String);

impl VmId {
    /// Mint a fresh identity for a job.
    #[must_use]
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// Recover the identity of a jail from its chroot directory name.
    ///
    /// The sweep works backwards from the filesystem, and the directory name
    /// is the identity that created it.
    #[must_use]
    pub fn from_chroot_name(name: String) -> Self {
        Self(name)
    }

    /// The identity as a string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for VmId {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Display for VmId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// Tracks whether this runner process has prepared the host.
///
/// Owned and threaded through the callers rather than kept in a global,
/// so that the latch belongs to one runner and cannot be observed, or reset,
/// by anything else. Tests get their own.
#[derive(Debug, Default)]
pub struct HostPreparation {
    /// Only the jail reads this, and the jail is Linux-only.
    #[cfg_attr(
        not(target_os = "linux"),
        expect(dead_code, reason = "host preparation is Linux-only")
    )]
    prepared: bool,
    /// Set when a job's teardown could not reclaim its chroot.
    reclaim_failed: ReclaimFailed,
}

/// Shared signal that a jail could not be reclaimed.
///
/// `Drop` has nowhere to report a failure, and a chroot that outlives its job
/// holds a copy of the VMM binary and a full guest rootfs. Because the sweep
/// otherwise runs once per process, a long-lived daemon would carry that leak
/// until a restart. Setting this makes the next job sweep again, which is the
/// mechanism that already exists for exactly this.
///
/// Owned by the runner's [`HostPreparation`] and cloned into each jail, never
/// global.
#[derive(Debug, Clone, Default)]
pub struct ReclaimFailed(Arc<AtomicBool>);

impl ReclaimFailed {
    /// Record that a jail could not be reclaimed.
    pub fn set(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Consume the signal.
    ///
    /// Deliberately not one swap together with [`Self::is_set`]. The signal asks
    /// for a sweep, so it is spent only once a sweep has actually finished:
    /// spending it up front would let a sweep that failed clear the one thing
    /// that makes a later job try again. Jobs are serial, so nothing can raise
    /// it between the sweep finishing and this call.
    ///
    /// Only the jail reads it, and the jail is Linux-only.
    #[cfg(target_os = "linux")]
    fn clear(&self) {
        self.0.store(false, Ordering::SeqCst);
    }

    /// Whether the signal is set, without consuming it.
    ///
    /// Read by the chroot teardown, which must not remove a directory whose
    /// cgroup is still there: the directory name is the only handle a later
    /// sweep has for finding that cgroup again.
    #[cfg(target_os = "linux")]
    #[must_use]
    pub(crate) fn is_set(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// Whether this job's own cgroup outlived its teardown.
///
/// Job-scoped, where [`ReclaimFailed`] is runner-scoped, and that distinction is
/// the point. The chroot has to be kept when *this* job's cgroup could not be
/// removed, because the directory name is the only handle a later sweep has for
/// finding that cgroup. A runner-wide signal cannot say that: it is also raised
/// by an unrelated stale jail the sweep could not reclaim, and reading it made
/// the current job hold a chroot whose own cgroup came down cleanly. Two
/// questions on one flag is also what made the earlier ordering bugs easy to
/// write.
#[derive(Debug, Clone, Default)]
pub struct CgroupSurvived(Arc<AtomicBool>);

impl CgroupSurvived {
    /// Record that this job's cgroup could not be removed.
    pub fn set(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    /// Whether this job's cgroup is still there.
    ///
    /// Only the chroot teardown reads it, and the jail is Linux-only.
    #[cfg(target_os = "linux")]
    fn is_set(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

/// The signals one job's teardown raises.
///
/// Carried together because every teardown step needs both: a cgroup that will
/// not go away has to hold this job's chroot *and* earn the next job a sweep.
/// Keeping them in one value with two names is what stops the two being confused
/// for each other again.
#[derive(Debug, Clone)]
pub struct JailSignals {
    /// Runner-wide: a later job owes another sweep.
    ///
    /// Only the jail raises and reads these, and the jail is Linux-only.
    #[cfg_attr(
        not(target_os = "linux"),
        expect(dead_code, reason = "the jail is Linux-only")
    )]
    reclaim_failed: ReclaimFailed,
    /// This job only: its cgroup outlived its teardown.
    #[cfg_attr(
        not(target_os = "linux"),
        expect(dead_code, reason = "the jail is Linux-only")
    )]
    cgroup_survived: CgroupSurvived,
}

impl JailSignals {
    /// The signals for one job, sharing the runner's reclaim signal.
    #[must_use]
    pub fn for_job(reclaim_failed: ReclaimFailed) -> Self {
        Self {
            reclaim_failed,
            cgroup_survived: CgroupSurvived::default(),
        }
    }

    /// Signals nothing reads, for a cgroup no sweep can find again.
    ///
    /// A non-sandboxed run has no chroot to hold and no directory the sweep
    /// walks, so there is no handle for a later sweep to work from and nothing a
    /// raised signal could change. Named rather than defaulted so the call site
    /// says which of the two it is.
    #[cfg(target_os = "linux")]
    #[must_use]
    pub fn unwatched() -> Self {
        Self::for_job(ReclaimFailed::default())
    }

    /// Record that this job's cgroup could not be removed.
    ///
    /// Both signals, because both are true: the chroot that names this cgroup
    /// has to stay, and the next job has to sweep for it.
    #[cfg(target_os = "linux")]
    pub fn cgroup_survived(&self) {
        self.cgroup_survived.set();
        self.reclaim_failed.set();
    }

    /// Record that this job's chroot could not be removed.
    #[cfg(target_os = "linux")]
    pub fn chroot_survived(&self) {
        self.reclaim_failed.set();
    }

    /// Whether this job's cgroup is still there, so its chroot must stay.
    #[cfg(target_os = "linux")]
    #[must_use]
    pub(crate) fn must_keep_chroot(&self) -> bool {
        self.cgroup_survived.is_set()
    }
}

impl HostPreparation {
    /// A runner process that has not prepared the host yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// A handle each jail uses to report that it could not be reclaimed.
    #[must_use]
    pub fn reclaim_signal(&self) -> ReclaimFailed {
        self.reclaim_failed.clone()
    }

    /// Prepare the host for jailed execution, at most once.
    ///
    /// Called on demand, immediately before the first job builds a jail, never
    /// at startup. The daemon learns which Specs it serves from the server, so
    /// at startup it cannot know whether it will ever need a jail, and a
    /// Runner that serves only non-sandboxed Specs is a supported
    /// configuration that must come up on a host where the runner is not root.
    /// Preparing eagerly would make `runner up` require root just to start.
    ///
    /// Failure is fatal. Untrusted code never runs with silently degraded
    /// confinement, so a host that cannot be prepared does not execute a job.
    /// A failure is not remembered, so the next job retries rather than
    /// needing a restart. That matters most for a sweep that could not reclaim
    /// a stale cgroup: the orphan may simply not have exited yet, and a
    /// latched failure would leave every later job failing with no retry.
    ///
    /// Requires root, and says so by name rather than letting the operator
    /// infer it from a permission error several layers down.
    #[cfg(target_os = "linux")]
    pub fn ensure(
        &mut self,
        state_dir: &camino::Utf8Path,
        jail_user: JailUser,
    ) -> Result<(), crate::error::JailError> {
        self.ensure_as(current_euid(), state_dir, jail_user)
    }

    /// Prepare the host, with the effective uid supplied.
    ///
    /// The uid is a parameter for the same reason the sweep's reap is one: the
    /// check refuses every uid but root, so a test that had to be root to reach
    /// anything past it would be exercising the harness rather than this.
    #[cfg(target_os = "linux")]
    fn ensure_as(
        &mut self,
        euid: u32,
        state_dir: &camino::Utf8Path,
        jail_user: JailUser,
    ) -> Result<(), crate::error::JailError> {
        // A jail that could not be reclaimed earns another sweep, whatever
        // this process has already done.
        if self.prepared && !self.reclaim_failed.is_set() {
            return Ok(());
        }
        let swept = prepare_host(euid, state_dir, jail_user)?;
        // Spent only on a sweep that finished, and armed by one that did not.
        // Spending it any earlier disarms the mechanism precisely when it is
        // needed: the signal would be gone, this process would still count as
        // prepared, and every later job would return early while the jail nobody
        // could reclaim sat there. That is true of a sweep that failed outright,
        // which never reaches here, and equally of one that returned `Ok` having
        // left a chroot on disk.
        if swept.is_complete() {
            self.reclaim_failed.clear();
        } else {
            self.reclaim_failed.set();
        }
        self.prepared = true;
        Ok(())
    }

    /// Prepare the host for jailed execution, at most once.
    ///
    /// The jail is Linux-only, as is the VM executor it protects.
    #[cfg(not(target_os = "linux"))]
    pub fn ensure(
        &mut self,
        _state_dir: &camino::Utf8Path,
        _jail_user: JailUser,
    ) -> Result<(), crate::error::JailError> {
        Ok(())
    }
}

/// Create the state directory and reclaim what a previous runner left behind.
///
/// The sweep is taken under the jail lock: it removes every chroot it finds on
/// the reasoning that jobs are serial, so it must not run while another runner
/// has one in flight. Running before any jail exists in this process is what
/// the sweep's purpose actually requires.
///
/// The network namespace is deliberately not built here. It is a process-
/// global object on a tmpfs, so it is rebuilt per job rather than once per
/// daemon lifetime.
#[cfg(target_os = "linux")]
#[expect(clippy::print_stdout, reason = "host preparation reports what it did")]
fn prepare_host(
    euid: u32,
    state_dir: &camino::Utf8Path,
    jail_user: JailUser,
) -> Result<state::Swept, crate::error::JailError> {
    // Checked first, and by name. Without it the most likely upgrade failure
    // surfaces as a permission error on a directory, or a bare EPERM out of
    // `unshare`, neither of which mentions root or the flag that avoids it.
    check_root(euid)?;

    let state = StateDir::new(state_dir.to_owned())?;
    state.create()?;

    warn_on_named_account(jail_user);

    let _lock = JailLock::acquire(state.path())?;
    let swept = state::sweep_jails(&state.jail_parent())?;
    if swept.reclaimed() > 0 {
        // Each one held a copy of the VMM binary and a full guest rootfs
        // image, so an operator should hear about it.
        println!(
            "  Reclaimed {} stale jail(s) from {state_dir}",
            swept.reclaimed()
        );
    }
    Ok(swept)
}

/// Refuse to build a jail without the privileges building one needs.
///
/// The failure an operator actually hits on upgrade is this one, so it says
/// what the release notes say rather than leaving them to infer it from a
/// permission error several layers down.
#[cfg(target_os = "linux")]
fn check_root(euid: u32) -> Result<(), crate::error::JailError> {
    if euid == 0 {
        Ok(())
    } else {
        Err(crate::error::JailError::NotRoot { euid })
    }
}

/// The effective uid of this process.
#[cfg(target_os = "linux")]
#[expect(
    unsafe_code,
    reason = "geteuid has no std wrapper and cannot fail or touch memory"
)]
fn current_euid() -> u32 {
    // SAFETY: `geteuid` takes no arguments, returns a plain integer, and is
    // documented as always succeeding.
    unsafe { libc::geteuid() }
}

/// Warn when the jail uid or gid belongs to a named account.
///
/// The jailer needs no passwd entry, so a name resolving here is the cheap
/// signal that the host allocates ids in this range: whatever owns that
/// account can signal the VMM and may be able to trace it. A warning rather
/// than a refusal, because an operator who deliberately created the account is
/// a legitimate setup and only they can tell the two apart.
#[cfg(target_os = "linux")]
#[expect(clippy::print_stderr, reason = "host preparation prints diagnostics")]
fn warn_on_named_account(jail_user: JailUser) {
    let (uid, gid) = (jail_user.uid(), jail_user.gid());
    if let Some(name) = passwd_name(uid) {
        eprintln!(
            "Warning: jail uid {uid} belongs to the existing account '{name}'. That account can signal the jailed VMM; pass --jail-uid to pick an unallocated id."
        );
    }
    if let Some(name) = group_name(gid) {
        eprintln!(
            "Warning: jail gid {gid} belongs to the existing group '{name}'. Pass --jail-gid to pick an unallocated id."
        );
    }
}

/// The account name for a uid, read from `/etc/passwd`.
///
/// Best effort, and blind to anything the local files do not know about: a
/// host backed by LDAP, Active Directory, or SSSD allocates ids that never
/// appear here, and those are the hosts most likely to allocate in this range
/// at all. Deliberately not a `getpwuid` call even so, because the runner
/// ships as a self-contained binary and NSS would make it depend on the host's
/// resolver configuration. This catches the cheap case; it is not a guarantee
/// that the id is unallocated.
#[cfg(target_os = "linux")]
fn passwd_name(uid: u32) -> Option<String> {
    lookup_name("/etc/passwd", uid)
}

/// The group name for a gid, read from `/etc/group`.
#[cfg(target_os = "linux")]
fn group_name(gid: u32) -> Option<String> {
    lookup_name("/etc/group", gid)
}

/// Find the name whose record carries `id` in a colon-separated database.
///
/// Both `/etc/passwd` and `/etc/group` put the name first and the numeric id
/// third.
#[cfg(target_os = "linux")]
fn lookup_name(path: &str, id: u32) -> Option<String> {
    let database = std::fs::read_to_string(path).ok()?;
    lookup_name_in(&database, id)
}

/// Find the name whose record carries `id`, given the database contents.
#[cfg(target_os = "linux")]
fn lookup_name_in(database: &str, id: u32) -> Option<String> {
    database.lines().find_map(|line| {
        let mut fields = line.split(':');
        let name = fields.next()?;
        let _password = fields.next()?;
        (fields.next()?.parse::<u32>().ok()? == id).then(|| name.to_owned())
    })
}

// Everything the jail prepares is Linux-only, and so is every test of it.
#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    /// The only uid that can build a jail.
    ///
    /// Supplied rather than inherited from the test process, which is usually
    /// not root and would otherwise never reach the preparation being tested.
    const ROOT_EUID: u32 = 0;

    #[test]
    fn a_runner_that_is_not_root_is_refused_by_name() {
        // The upgrade failure an operator actually hits. Without this it
        // surfaces as a permission error on a directory, or a bare EPERM out of
        // `unshare`, neither of which mentions root or the flag that avoids it.
        let dir = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let state_dir = root.join("state");

        let mut host = HostPreparation::new();
        // Nothing latches, so every job says it again rather than only the
        // first one.
        for attempt in 1..=3 {
            let err = host
                .ensure_as(1000, &state_dir, JailUser::default())
                .unwrap_err();
            let message = err.to_string();
            assert!(
                message.contains("1000"),
                "attempt {attempt} must name the uid: {message}"
            );
            assert!(
                message.contains("root"),
                "attempt {attempt} must name root: {message}"
            );
            // Both escapes, because both subcommands reach this error and each
            // has only one of them: `--danger-allow-no-sandbox` exists on
            // `runner up`, and a one-shot `runner run` gives up the sandbox by
            // omitting `--sandbox`. Naming only the daemon's flag sends a
            // `runner run` operator to an argument it does not accept.
            assert!(
                message.contains("--danger-allow-no-sandbox"),
                "attempt {attempt} must name the daemon's escape hatch: {message}"
            );
            assert!(
                message.contains("without --sandbox"),
                "attempt {attempt} must name the one-shot escape hatch: {message}"
            );
        }

        assert!(
            !state_dir.exists(),
            "a refused runner must not have touched the state directory"
        );
    }

    #[test]
    fn the_default_jail_user_is_outside_the_allocated_ranges() {
        // systemd-homed takes 60001-60513 and DynamicUser takes 61184-65519.
        // An id inside either would collide with something the host allocates.
        for id in [DEFAULT_JAIL_UID, DEFAULT_JAIL_GID] {
            assert_eq!(id, 61016, "the jail id is a project convention");
            assert!(id > 60513, "{id} must clear the systemd-homed range");
            assert!(id < 61184, "{id} must clear the DynamicUser range");
        }
    }

    #[test]
    fn preparation_is_lazy_and_happens_at_most_once() {
        // A daemon that prepared at startup would need root just to come up,
        // which breaks a Runner serving only non-sandboxed Specs. Nothing may
        // touch the state directory until a job actually builds a jail.
        let dir = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let state_dir = root.join("state");
        assert!(!state_dir.exists(), "startup has not prepared anything");

        let mut host = HostPreparation::new();
        host.ensure_as(ROOT_EUID, &state_dir, JailUser::default())
            .unwrap();
        assert!(state_dir.join("jail").is_dir(), "the first job prepares");

        // A second job must not redo it: proven by removing the tree and
        // seeing that it is not rebuilt. Owning the token is what makes this
        // independent of every other test in the process.
        std::fs::remove_dir_all(&state_dir).unwrap();
        host.ensure_as(ROOT_EUID, &state_dir, JailUser::default())
            .unwrap();
        assert!(!state_dir.exists(), "preparation happens at most once");

        // A different runner prepares its own host.
        let mut other = HostPreparation::new();
        other
            .ensure_as(ROOT_EUID, &state_dir, JailUser::default())
            .unwrap();
        assert!(
            state_dir.join("jail").is_dir(),
            "a fresh token prepares again"
        );
    }

    #[test]
    fn a_failure_is_not_latched_and_self_heals() {
        // Fatal to the job, not to the runner. A host that cannot be prepared
        // has to fail every job that needs a jail, and recover on its own the
        // moment the cause goes away, rather than wedging until a restart.
        let dir = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let state_dir = root.join("state");
        // A populated directory the runner did not create is refused.
        std::fs::create_dir_all(state_dir.join("someone-elses-data")).unwrap();

        let mut host = HostPreparation::new();
        for attempt in 1..=3 {
            assert!(
                host.ensure_as(ROOT_EUID, &state_dir, JailUser::default())
                    .is_err(),
                "attempt {attempt} must fail"
            );
        }

        // Remove the cause and the very next job succeeds, with no restart.
        std::fs::remove_dir(state_dir.join("someone-elses-data")).unwrap();
        host.ensure_as(ROOT_EUID, &state_dir, JailUser::default())
            .unwrap();
        assert!(state_dir.join("jail").is_dir());
    }

    #[test]
    fn a_jail_that_could_not_be_reclaimed_earns_another_sweep() {
        // The sweep otherwise runs once per process, so a teardown that failed
        // in a long-lived daemon would leak a chroot holding a VMM copy and a
        // full guest rootfs until a restart. Drop has nowhere to report, so it
        // raises this instead.
        let dir = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let state_dir = root.join("state");

        let mut host = HostPreparation::new();
        host.ensure_as(ROOT_EUID, &state_dir, JailUser::default())
            .unwrap();

        // Already prepared: a second job does not redo the work.
        std::fs::remove_dir_all(&state_dir).unwrap();
        host.ensure_as(ROOT_EUID, &state_dir, JailUser::default())
            .unwrap();
        assert!(!state_dir.exists(), "preparation happens at most once");

        // A jail that could not be reclaimed changes that.
        host.reclaim_signal().set();
        host.ensure_as(ROOT_EUID, &state_dir, JailUser::default())
            .unwrap();
        assert!(
            state_dir.join("jail").is_dir(),
            "a failed teardown must earn another sweep"
        );

        // And the signal is consumed, not sticky.
        std::fs::remove_dir_all(&state_dir).unwrap();
        host.ensure_as(ROOT_EUID, &state_dir, JailUser::default())
            .unwrap();
        assert!(!state_dir.exists(), "the signal is consumed once");
    }

    #[test]
    fn a_sweep_that_failed_does_not_disarm_the_next_one() {
        // The signal is spent on a sweep that finished, never on one that was
        // merely attempted. Consuming it up front costs nothing on the first
        // failure and everything on the second: this process still counts as
        // prepared, so with the signal gone every later job returns early and
        // the jail that could not be reclaimed is never swept again for the
        // lifetime of the daemon. If the reason it could not be reclaimed is a
        // VMM still in its cgroup, that orphan holds the benchmark cores while
        // every later job measures through it and reports clean.
        let dir = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let state_dir = root.join("state");

        let mut host = HostPreparation::new();
        host.ensure_as(ROOT_EUID, &state_dir, JailUser::default())
            .unwrap();

        // A teardown that could not reclaim its jail asks for another sweep.
        host.reclaim_signal().set();

        // Preparation now fails, twice, with the signal still outstanding.
        std::fs::remove_dir_all(&state_dir).unwrap();
        std::fs::create_dir_all(state_dir.join("someone-elses-data")).unwrap();
        for attempt in 1..=2 {
            host.ensure_as(ROOT_EUID, &state_dir, JailUser::default())
                .unwrap_err();
            assert!(
                host.reclaim_signal().is_set(),
                "attempt {attempt} failed, so the sweep it asked for is still owed"
            );
        }

        // Remove the cause: the next job must still sweep rather than return
        // early on the strength of a signal an earlier failure ate.
        std::fs::remove_dir_all(&state_dir).unwrap();
        host.ensure_as(ROOT_EUID, &state_dir, JailUser::default())
            .unwrap();
        assert!(
            state_dir.join("jail").is_dir(),
            "the owed sweep must survive every failed attempt at it"
        );

        // And it is spent now that one has finished.
        assert!(!host.reclaim_signal().is_set());
        std::fs::remove_dir_all(&state_dir).unwrap();
        host.ensure_as(ROOT_EUID, &state_dir, JailUser::default())
            .unwrap();
        assert!(!state_dir.exists(), "a finished sweep spends the signal");
    }

    #[test]
    fn the_jail_user_rejects_root() {
        // Untrusted code against a root VMM is the one thing the confinement
        // exists to prevent, so this must not be reachable by a typo.
        JailUser::new(0, DEFAULT_JAIL_GID).unwrap_err();
        JailUser::new(DEFAULT_JAIL_UID, 0).unwrap_err();
        JailUser::new(0, 0).unwrap_err();

        let user = JailUser::new(1234, 5678).unwrap();
        assert_eq!(user.uid(), 1234);
        assert_eq!(user.gid(), 5678);
    }

    #[test]
    fn the_default_jail_user_is_unprivileged() {
        let default = JailUser::default();
        assert_eq!(default.uid(), DEFAULT_JAIL_UID);
        assert_eq!(default.gid(), DEFAULT_JAIL_GID);
        JailUser::new(default.uid(), default.gid()).unwrap();
    }

    #[test]
    fn a_named_account_is_found_by_id() {
        let passwd = "root:x:0:0:root:/root:/bin/bash\nbuild:x:61016:61016:CI build user:/home/build:/bin/sh\n";

        assert_eq!(lookup_name_in(passwd, 61016).as_deref(), Some("build"));
        assert_eq!(lookup_name_in(passwd, 0).as_deref(), Some("root"));
    }

    #[test]
    fn an_unallocated_id_has_no_name() {
        let passwd =
            "root:x:0:0:root:/root:/bin/bash\ndaemon:x:1:1:daemon:/usr/sbin:/usr/sbin/nologin\n";

        assert_eq!(lookup_name_in(passwd, 61016), None);
    }

    #[test]
    fn malformed_records_are_skipped() {
        let passwd = "\nnot-a-record\nshort:x\nbuild:x:61016:61016::/home/build:/bin/sh\n";

        assert_eq!(lookup_name_in(passwd, 61016).as_deref(), Some("build"));
        assert_eq!(lookup_name_in("", 61016), None);
    }
}
