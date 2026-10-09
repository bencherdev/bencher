//! Confinement for Firecracker microVMs, which run code submitted by anyone and
//! so must not inherit the runner's root.
//!
//! # What a failing step does
//!
//! - **Fails the job.** The host cannot be trusted to measure, and since nothing
//!   latches, the next job tries the whole thing again.
//! - **Leaves it to the next sweep, or declares the absence.** What teardown
//!   could not remove is retried by the next job's sweep, and isolation or a
//!   metric the host cannot provide is reported as absent, never as a number.
//! - **Ignored.** Only where the failure is itself the answer or a later step is
//!   guaranteed to catch it, and each row says which.
//!
//! | Step | On failure |
//! |---|---|
//! | [`HostPreparation::ensure`]: the root check | fails the job |
//! | [`HostPreparation::ensure`]: creating the state directory | fails the job |
//! | [`HostPreparation::ensure`]: reading `/etc/passwd`, `/etc/group` | ignored: the check is advisory and cannot see a directory service anyway |
//! | `StateDir::refuse_unusable_mount`: the mount options cannot be read, or include `nodev` or `noexec` | fails the job |
//! | `vm_execute`: taking the jail lock | fails the job |
//! | `JailLock::acquire`: a cancel while waiting | fails the job without the lock |
//! | `vm_execute` and `run_firecracker`: a cancel at a stage boundary before `InstanceStart` | fails the job before the next stage |
//! | `StateDir::sweep`: a sweep that returns an error | fails the job |
//! | `refuse_occupied_cgroups`: the cgroup base is absent | nothing to check |
//! | `refuse_occupied_cgroups`: the base, an entry, or a `cgroup.procs` cannot be read or parsed | fails the job |
//! | `refuse_occupied_cgroups`: a cgroup gone since it was listed | ignored: gone holds nothing |
//! | `refuse_occupied_cgroups`, before the jail and again once the VMM is placed: another cgroup holds a process | fails the job, before the guest runs |
//! | `sweep_jails`: the jail parent is absent | nothing to sweep |
//! | `sweep_jails`: the jail parent cannot be read | fails the job |
//! | `sweep_jails`: an entry cannot be read | fails the job |
//! | `sweep_jails`: an entry's kind cannot be read | fails the job: it may be a jail |
//! | `sweep_jails`: a name that is not UTF-8 | ignored: every name here is a UUID this runner minted, so it is not ours |
//! | `sweep_jails`: a name this runner could not have minted | ignored: same reason, and the id is joined into a chroot path and a cgroup path |
//! | `sweep_jails`: the reap reports a live VMM | fails the job |
//! | `sweep_jails`: the reap could not examine the jail | fails the job |
//! | `sweep_jails`: removing the cgroup | fails the job, and the chroot is kept because its name is the cgroup's only handle |
//! | `sweep_jails`: removing the chroot | left to the next job's sweep |
//! | `reap_jailed_vmm`: the jail root is absent | clear: nothing can be chrooted into a directory that is not there |
//! | `reap_jailed_vmm`: the jail root cannot be stat'ed | reported unexaminable, which fails the job |
//! | `reap_jailed_vmm`: `/proc` cannot be listed | reported unexaminable |
//! | `reap_jailed_vmm`: a `/proc/<pid>/root` cannot be read | ignored: that process is gone or is not this jail's |
//! | `reap_jailed_vmm`: a jail's `cgroup.procs` cannot be read while scanning | reported unexaminable, which fails the job |
//! | `reap_jailed_vmm`: a jail's `cgroup.procs` cannot be read while re-checking a pid | ignored: that process is gone or is not this jail's |
//! | `reap_jailed_vmm`: pinning or signalling the VMM | reported still running, which fails the job |
//! | `reap_jailed_vmm`: a VMM that will not exit | reported still running |
//! | `reap_jailed_vmm`: the rescan bound runs out | reported still running |
//! | `reap_jailed_vmm`: a `poll` of the pidfd that fails or answers nothing | reported still running, which fails the job |
//! | `reap_jailed_vmm`: a `poll` of the pidfd a signal interrupts | retried: the signal reports an arrival in this process and says nothing about the one being watched |
//! | `JailDir::create`: the state tree fails its re-check at job time | fails the job: the chroot would otherwise be built through a component swapped since preparation |
//! | `JailDir::create`: a step that fails once the tree exists | fails the job, and the guard that already took the tree reclaims it |
//! | `JailDir` teardown: the chroot is already gone | ignored: that is the goal state |
//! | `JailDir` teardown: removing the chroot | left to the next job's sweep |
//! | `JailDir` teardown: this job's own cgroup survived | keeps the chroot, since its name is the cgroup's only handle |
//! | `CgroupManager` teardown: the cgroup cannot be stat'ed | keeps the chroot for the next job's sweep: it is treated as still there |
//! | `CgroupManager` teardown: `rmdir` of the cgroup | keeps the chroot for the next job's sweep |
//! | `CgroupManager` creation: the cgroup cannot be stat'ed | fails the job: this decides whether `Drop` may remove it |
//! | `cgroup_for_run`: the VM cgroup cannot be created, for any reason | fails the job: a VMM outside its cgroup runs unconfined, unmetered, and unseen by `refuse_occupied_cgroups` |
//! | `cgroup_for_run`: the CPU layout offers no isolation | runs with no VM cgroup, which startup announced as CPU isolation disabled |
//! | `run_firecracker`: the VMM cannot be placed in its cgroup before exec, or is not in it after | fails the job |
//! | `remove_stale_cgroup`: the cgroup cannot be stat'ed | fails the job: the caller deletes the chroot on an `Ok` here |
//! | `StateDir::create`: the chroot tree cannot be stat'ed | fails the job: the 0700 chmod follows |
//! | `StateDir::create`: taking a directory of the tree for root (the chown to 0:0) | fails the job; `EPERM` alone is ignored: it refuses exactly a process that never builds a jail, since root is checked by name before any of this runs |
//! | `StateDir::new`: the root is a symlink proven the operator's own choice (parent root-only-writable, single hop to an absolute canonical target, that target's whole ancestry root-only-writable) | followed: no unprivileged user influenced or can race it, and the populated check still runs on the target |
//! | `StateDir::new`: the root is a symlink failing any of those showings | left as given, so `real_dir` refuses it as a symlink at create time |
//! | `StateDir::create`: an interior component of the tree is a symlink | fails the job: the 0700 chmod and the sweep would land on a directory somebody else chose |
//!
//! And the same three columns for the reads that decide what a run measured:
//!
//! | Step | On failure |
//! |---|---|
//! | `apply_cpuset`: no isolation in the layout, or an empty core set | declares the absence |
//! | `apply_cpuset`: `cpuset.cpus` is absent | declares the absence, naming the controller and the cgroup that withheld it, or the VM cgroup itself when no absence was recorded |
//! | `apply_cpuset`: `cpuset.cpus` cannot be stat'ed | fails the job: an error is not an absence |
//! | `apply_cpuset`: the kernel rejects a cpuset write | fails the job: half-applied confinement |
//! | `apply_cpuset`: the parent's node set is absent | node 0, which is what an undelegated controller means |
//! | `apply_cpuset`: the parent's node set cannot be read | fails the job: this value is written, so a guess confines the guest's memory |
//! | `apply_cpuset`: the effective set cannot be read back | fails the job: the read back *is* the mechanism, and the write already proved the controller delegated |
//! | `apply_cpuset`: the kernel narrowed the set | fails the job |
//! | `apply_cpuset`: either side of the verification cannot be parsed as a cpu list | fails the job: dropping what would not parse would compare partial sets nobody read |
//! | `ensure_controllers`, on a job: the root's `cgroup.controllers` or `bencher/`'s `cgroup.subtree_control` cannot be read | fails the job: an unreadable list is not an empty one |
//! | `ensure_controllers`: the root does not offer `cpuset` or `memory` | declares that controller's absence, naming it and the root: no cpuset, or no swap limit |
//! | `ensure_controllers`: the kernel refuses a write for `cpuset` or `memory`, and `bencher/` does not enable it | declares that controller's absence, naming it and the first cgroup that refused it |
//! | `ensure_controllers`: `pids` is not offered or is refused | ignored: nothing the runner reads depends on it |
//! | `ensure_controllers`: another controller cannot be disabled in `bencher/` | warns: Jobs run with it enabled |
//! | `disable_swap`: `memory` is absent, or `memory.swap.max` cannot be written | declares the absence: no swap limit |
//! | `BencherPartition::apply`: any read or write in the partition path, `ensure_controllers` included | declares the absence: it warns, and the level achieved is reported, down to `member` |
//! | `BencherPartition::apply`: the kernel refuses both partition modes | writes `member` back, where it stays: tuning is never reverted |
//! | `CpuLayout::detect`: the online CPU list cannot be read or parsed | declares the absence: the counted layout is announced as a guess |
//! | `CpuLayout::detect`: the core count cannot be read | one core, which reports as a layout with no isolation |
//! | `metrics`: a cgroup that is not there | no metrics, reported as absent |
//! | `metrics`: a field that cannot be read or parsed | absent, never zero |
//! | `tuning::preflight`: any check that cannot be performed | ignored: advisory only, and nothing reads it to decide whether the host can measure. A quiet preflight is not evidence of a quiet host |
//! | `pin_vcpu_threads`: a thread list that cannot be read | declares the absence: it reports how many of the vCPUs it pinned |
//! | `FirecrackerClient`: a status line that cannot be parsed | fails the request: no status is invented for a response Firecracker did not send |
//! | `find_binary`: a candidate path that cannot be stat'ed | ignored: the search is a list of guesses, and finding nothing is reported by name |
//!
//! `CgroupManager::kill_all` is in neither table: the jailed path never calls
//! it, and where the non-sandboxed path does, a failure is warned and any
//! survivor is caught by the cgroup `rmdir` above.

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

#[cfg(all(test, target_os = "linux"))]
pub(crate) use cgroup::ScratchCgroup;
#[cfg(target_os = "linux")]
pub(crate) use cgroup::{
    BENCHER_CGROUP_BASE, Controllers, effective_mems, ensure_controllers, refuse_occupied_cgroups,
};
#[cfg(target_os = "linux")]
pub use cgroup::{CgroupManager, Cpuset};
#[cfg(target_os = "linux")]
pub use chroot::JailDir;
#[cfg(target_os = "linux")]
pub use lock::JailLock;
#[cfg(target_os = "linux")]
pub use paths::{ChrootPath, HostPath, JailFile, JailPaths, PinnedSocket, SocketPath};
#[cfg(target_os = "linux")]
pub use state::StateDir;

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

pub const DEFAULT_STATE_DIR: &str = "/var/lib/bencher-runner";

/// This uid sits in the unallocated gap between `systemd-homed` ids (60001-60513) and
/// the `DynamicUser` range (61184-65519).
///
/// Two runners on one host should each pass their own `--jail-uid`, since VMMs sharing
/// a uid can signal each other.
pub const DEFAULT_JAIL_UID: u32 = 61016;

pub const DEFAULT_JAIL_GID: u32 = 61016;

/// The fields are private so `0` never reaches them: a jail user of root is no
/// jail at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JailUser {
    uid: u32,
    gid: u32,
}

impl JailUser {
    pub fn new(uid: u32, gid: u32) -> Result<Self, crate::error::JailError> {
        if uid == 0 {
            return Err(crate::error::JailError::PrivilegedJailUser { field: "uid" });
        }
        if gid == 0 {
            return Err(crate::error::JailError::PrivilegedJailUser { field: "gid" });
        }
        Ok(Self { uid, gid })
    }

    #[must_use]
    pub fn uid(self) -> u32 {
        self.uid
    }

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

/// The jailer resolves `--chroot-base-dir` against its own working directory,
/// so a relative state directory builds chroots the sweep never reaches and the
/// lock never protects.
pub fn check_absolute_state_dir(path: &camino::Utf8Path) -> Result<(), crate::error::JailError> {
    if path.is_absolute() {
        Ok(())
    } else {
        Err(crate::error::JailError::RelativeStateDir {
            path: path.to_owned(),
        })
    }
}

/// One string serves as the jailer's `--id`, the chroot directory name, and the
/// cgroup name, so the three cannot drift.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VmId(String);

impl VmId {
    #[must_use]
    pub fn new() -> Self {
        Self(uuid::Uuid::new_v4().to_string())
    }

    /// The `local-` prefix tells an operator reading the cgroup base which kind
    /// of run left a cgroup there.
    #[cfg(target_os = "linux")]
    #[must_use]
    pub(crate) fn for_local_run() -> Self {
        Self(format!("local-{}", uuid::Uuid::new_v4()))
    }

    /// `None` for a name this runner could not have minted, since callers join
    /// the id into both a chroot path and a `/sys/fs/cgroup` path.
    #[cfg(target_os = "linux")]
    #[must_use]
    pub(crate) fn from_chroot_name(name: String) -> Option<Self> {
        let minted = !name.is_empty()
            && !name.starts_with('.')
            && !name.contains('/')
            && !name.contains("..");
        minted.then_some(Self(name))
    }

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

/// Per runner rather than global, so nothing else can observe or reset it and
/// each test gets its own.
#[derive(Debug, Default)]
pub struct HostPreparation {
    #[cfg_attr(
        not(target_os = "linux"),
        expect(dead_code, reason = "host preparation is Linux-only")
    )]
    warned_jail_user: bool,
}

/// Set when this job's cgroup outlives teardown, so the chroot is kept: its
/// name is the next sweep's only handle on that cgroup.
#[derive(Debug, Clone, Default)]
pub struct CgroupSurvived(Arc<AtomicBool>);

impl CgroupSurvived {
    pub fn set(&self) {
        self.0.store(true, Ordering::SeqCst);
    }

    #[cfg(target_os = "linux")]
    fn is_set(&self) -> bool {
        self.0.load(Ordering::SeqCst)
    }
}

impl HostPreparation {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Runs before every jail, never at startup, so a runner serving only
    /// non-sandboxed Specs can start without root and a state directory removed
    /// between jobs is rebuilt.
    #[cfg(target_os = "linux")]
    pub fn ensure(
        &mut self,
        log: &slog::Logger,
        state_dir: &camino::Utf8Path,
        jail_user: JailUser,
    ) -> Result<(), crate::error::JailError> {
        self.ensure_as(log, current_euid(), state_dir, jail_user)
    }

    /// The uid is a parameter so tests reach past the root check without
    /// running as root.
    #[cfg(target_os = "linux")]
    fn ensure_as(
        &mut self,
        log: &slog::Logger,
        euid: u32,
        state_dir: &camino::Utf8Path,
        jail_user: JailUser,
    ) -> Result<(), crate::error::JailError> {
        // First, so a non-root runner is told about root rather than hitting a
        // bare EPERM from a later step.
        check_root(euid)?;
        StateDir::new(state_dir.to_owned())?.create()?;
        if !self.warned_jail_user {
            warn_on_named_account(log, jail_user);
            self.warned_jail_user = true;
        }
        Ok(())
    }

    /// A no-op, since the VM executor this jail protects is Linux-only.
    #[cfg(not(target_os = "linux"))]
    pub fn ensure(
        &mut self,
        _log: &slog::Logger,
        _state_dir: &camino::Utf8Path,
        _jail_user: JailUser,
    ) -> Result<(), crate::error::JailError> {
        Ok(())
    }
}

#[cfg(target_os = "linux")]
fn check_root(euid: u32) -> Result<(), crate::error::JailError> {
    if euid == 0 {
        Ok(())
    } else {
        Err(crate::error::JailError::NotRoot { euid })
    }
}

#[cfg(target_os = "linux")]
#[expect(
    unsafe_code,
    reason = "geteuid has no std wrapper and cannot fail or touch memory"
)]
pub(crate) fn current_euid() -> u32 {
    // SAFETY: `geteuid` takes no arguments and always succeeds.
    unsafe { libc::geteuid() }
}

/// A warning rather than a refusal, because only the operator can tell a
/// deliberately created account from an id the host allocated elsewhere.
#[cfg(target_os = "linux")]
fn warn_on_named_account(log: &slog::Logger, jail_user: JailUser) {
    let (uid, gid) = (jail_user.uid(), jail_user.gid());
    // That account can signal the jailed VMM.
    if let Some(name) = passwd_name(uid) {
        slog::warn!(log, "Jail uid belongs to an existing account, pass --jail-uid to pick an unallocated id";
            "uid" => uid,
            "account" => bencher_logger::capped(name),
        );
    }
    if let Some(name) = group_name(gid) {
        slog::warn!(log, "Jail gid belongs to an existing group, pass --jail-gid to pick an unallocated id";
            "gid" => gid,
            "group" => bencher_logger::capped(name),
        );
    }
}

/// Not `getpwuid`, since NSS would tie the self-contained binary to the host's
/// resolver, at the cost of missing ids from LDAP or SSSD.
#[cfg(target_os = "linux")]
fn passwd_name(uid: u32) -> Option<String> {
    lookup_name("/etc/passwd", uid)
}

#[cfg(target_os = "linux")]
fn group_name(gid: u32) -> Option<String> {
    lookup_name("/etc/group", gid)
}

/// Both `/etc/passwd` and `/etc/group` put the name first and the numeric id
/// third.
#[cfg(target_os = "linux")]
fn lookup_name(path: &str, id: u32) -> Option<String> {
    let database = std::fs::read_to_string(path).ok()?;
    lookup_name_in(&database, id)
}

#[cfg(target_os = "linux")]
fn lookup_name_in(database: &str, id: u32) -> Option<String> {
    database.lines().find_map(|line| {
        let mut fields = line.split(':');
        let name = fields.next()?;
        let _password = fields.next()?;
        (fields.next()?.parse::<u32>().ok()? == id).then(|| name.to_owned())
    })
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;
    use crate::log::discard;

    const ROOT_EUID: u32 = 0;

    #[test]
    fn a_runner_that_is_not_root_is_refused_by_name() {
        // A non-root runner must be told about root and both subcommands'
        // escape hatches on every job, not left with a bare EPERM.
        let dir = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let state_dir = root.join("state");

        let mut host = HostPreparation::new();
        for attempt in 1..=3 {
            let err = host
                .ensure_as(&discard(), 1000, &state_dir, JailUser::default())
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
    fn a_state_directory_removed_between_jobs_is_rebuilt() {
        // Nothing is prepared before the first job, and a tree removed between
        // jobs must come back rather than fail every later job until a restart.
        let dir = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let state_dir = root.join("state");
        assert!(!state_dir.exists(), "startup has not prepared anything");

        let mut host = HostPreparation::new();
        host.ensure_as(&discard(), ROOT_EUID, &state_dir, JailUser::default())
            .unwrap();
        assert!(state_dir.join("jail").is_dir(), "the first job prepares");

        std::fs::remove_dir_all(&state_dir).unwrap();
        host.ensure_as(&discard(), ROOT_EUID, &state_dir, JailUser::default())
            .unwrap();
        assert!(
            state_dir.join("jail").join("firecracker").is_dir(),
            "a removed tree is rebuilt"
        );
    }

    #[test]
    fn a_failure_is_not_latched_and_self_heals() {
        // A failed preparation must not latch: every job fails while the state
        // directory holds foreign data, and the next succeeds once it is gone.
        let dir = tempfile::tempdir().unwrap();
        let root = camino::Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let state_dir = root.join("state");
        std::fs::create_dir_all(state_dir.join("someone-elses-data")).unwrap();

        let mut host = HostPreparation::new();
        for attempt in 1..=3 {
            assert!(
                host.ensure_as(&discard(), ROOT_EUID, &state_dir, JailUser::default())
                    .is_err(),
                "attempt {attempt} must fail"
            );
        }

        std::fs::remove_dir(state_dir.join("someone-elses-data")).unwrap();
        host.ensure_as(&discard(), ROOT_EUID, &state_dir, JailUser::default())
            .unwrap();
        assert!(state_dir.join("jail").is_dir());
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
    fn a_name_that_could_not_have_been_minted_is_not_an_identity() {
        // A recovered name that walks out of the chroot or cgroup base must be
        // refused, while every minted id still round-trips.
        for name in ["", ".", "..", "../../etc", "jail/../..", "a/b", ".hidden"] {
            assert!(
                VmId::from_chroot_name(name.to_owned()).is_none(),
                "'{name}' is not a name this runner could have minted"
            );
        }

        let jail = VmId::new();
        assert_eq!(
            VmId::from_chroot_name(jail.as_str().to_owned()),
            Some(jail),
            "a minted id has to survive the round trip the sweep makes"
        );
        let local = VmId::for_local_run();
        assert_eq!(
            VmId::from_chroot_name(local.as_str().to_owned()),
            Some(local)
        );
    }

    #[test]
    fn a_local_run_mints_an_id_rather_than_recovering_one() {
        // Two local runs' cgroups are siblings under one base, so their ids
        // must be unique and say they are local.
        let one = VmId::for_local_run();
        let two = VmId::for_local_run();

        assert!(
            one.as_str().starts_with("local-"),
            "a local run says so in the name: {one}"
        );
        assert_ne!(one, two, "two local runs must not name one cgroup");
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
