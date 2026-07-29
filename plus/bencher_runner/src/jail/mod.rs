//! Confinement for Firecracker microVMs.
//!
//! Managed runners execute arbitrary code submitted by anyone, so the VMM must
//! not inherit the runner's root. This module owns everything that confines it:
//! the persistent state directory the chroots are built under, the empty
//! network namespace the VMM joins, and the cgroup that both places it on the
//! benchmark cores and bounds its resources.

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

use serde::{Deserialize, Serialize};

/// Default location of the runner's persistent state directory.
pub const DEFAULT_STATE_DIR: &str = "/var/lib/bencher-runner";

/// Default unprivileged uid the jailed Firecracker VMM runs as.
///
/// One dedicated id, not one per job: jobs run serially and each gets a fresh
/// chroot that is swept, so a per-job allocator adds a scheme without closing
/// a live vector.
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
}

impl HostPreparation {
    /// A runner process that has not prepared the host yet.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
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
    #[cfg(target_os = "linux")]
    pub fn ensure(
        &mut self,
        state_dir: &camino::Utf8Path,
        jail_user: JailUser,
    ) -> Result<(), crate::error::JailError> {
        if self.prepared {
            return Ok(());
        }
        prepare_host(state_dir, jail_user)?;
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
    state_dir: &camino::Utf8Path,
    jail_user: JailUser,
) -> Result<(), crate::error::JailError> {
    let state = StateDir::new(state_dir.to_owned());
    state.create()?;

    warn_on_named_account(jail_user);

    let _lock = JailLock::acquire(state.path())?;
    let swept = state::sweep_jails(&state.jail_parent())?;
    if swept > 0 {
        // Each one held a copy of the VMM binary and a full guest rootfs
        // image, so an operator should hear about it.
        println!("  Reclaimed {swept} stale jail(s) from {state_dir}");
    }
    Ok(())
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

/// Resource limits for the Firecracker microVM process.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceLimits {
    /// Maximum CPU time in microseconds per second.
    /// E.g., 100000 = 100ms per 100ms period = 1 full CPU.
    #[serde(default)]
    pub cpu_quota_us: Option<u64>,

    /// CPU period in microseconds (default: 100000 = 100ms).
    #[serde(default = "default_cpu_period")]
    pub cpu_period_us: u64,

    /// Maximum memory in bytes.
    #[serde(default)]
    pub memory_bytes: Option<u64>,

    /// Maximum number of open file descriptors.
    #[serde(default = "default_max_fds")]
    pub max_fds: u64,

    /// Maximum number of processes/threads.
    #[serde(default = "default_max_procs")]
    pub max_procs: u64,

    /// Maximum I/O read bandwidth in bytes per second.
    /// Applied via cgroup v2 io.max.
    #[serde(default)]
    pub io_read_bps: Option<u64>,

    /// Maximum I/O write bandwidth in bytes per second.
    /// Applied via cgroup v2 io.max.
    #[serde(default)]
    pub io_write_bps: Option<u64>,
}

const fn default_cpu_period() -> u64 {
    100_000 // 100ms
}

const fn default_max_fds() -> u64 {
    1024
}

const fn default_max_procs() -> u64 {
    64
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            cpu_quota_us: None,
            cpu_period_us: default_cpu_period(),
            memory_bytes: None,
            max_fds: default_max_fds(),
            max_procs: default_max_procs(),
            io_read_bps: None,
            io_write_bps: None,
        }
    }
}

impl ResourceLimits {
    /// Set CPU limit as a fraction of CPUs (e.g., 0.5 = half a CPU, 2.0 = 2 CPUs).
    #[must_use]
    #[expect(
        clippy::cast_possible_truncation,
        clippy::cast_sign_loss,
        clippy::cast_precision_loss,
        reason = "CPU fraction to microsecond quota conversion"
    )]
    pub fn with_cpu_limit(mut self, cpus: f64) -> Self {
        let quota = (cpus * self.cpu_period_us as f64) as u64;
        self.cpu_quota_us = Some(quota);
        self
    }

    /// Set memory limit in bytes.
    #[must_use]
    pub fn with_memory_limit(mut self, bytes: u64) -> Self {
        self.memory_bytes = Some(bytes);
        self
    }

    /// Set I/O bandwidth limits in bytes per second.
    #[must_use]
    pub fn with_io_limits(mut self, read_bps: u64, write_bps: u64) -> Self {
        self.io_read_bps = Some(read_bps);
        self.io_write_bps = Some(write_bps);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(target_os = "linux")]
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

    #[cfg(target_os = "linux")]
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
        host.ensure(&state_dir, JailUser::default()).unwrap();
        assert!(state_dir.join("jail").is_dir(), "the first job prepares");

        // A second job must not redo it: proven by removing the tree and
        // seeing that it is not rebuilt. Owning the token is what makes this
        // independent of every other test in the process.
        std::fs::remove_dir_all(&state_dir).unwrap();
        host.ensure(&state_dir, JailUser::default()).unwrap();
        assert!(!state_dir.exists(), "preparation happens at most once");

        // A different runner prepares its own host.
        let mut other = HostPreparation::new();
        other.ensure(&state_dir, JailUser::default()).unwrap();
        assert!(
            state_dir.join("jail").is_dir(),
            "a fresh token prepares again"
        );
    }

    #[cfg(target_os = "linux")]
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
                host.ensure(&state_dir, JailUser::default()).is_err(),
                "attempt {attempt} must fail"
            );
        }

        // Remove the cause and the very next job succeeds, with no restart.
        std::fs::remove_dir(state_dir.join("someone-elses-data")).unwrap();
        host.ensure(&state_dir, JailUser::default()).unwrap();
        assert!(state_dir.join("jail").is_dir());
    }

    #[cfg(target_os = "linux")]
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

    #[cfg(target_os = "linux")]
    #[test]
    fn the_default_jail_user_is_unprivileged() {
        let default = JailUser::default();
        assert_eq!(default.uid(), DEFAULT_JAIL_UID);
        assert_eq!(default.gid(), DEFAULT_JAIL_GID);
        JailUser::new(default.uid(), default.gid()).unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_named_account_is_found_by_id() {
        let passwd = "root:x:0:0:root:/root:/bin/bash\nbuild:x:61016:61016:CI build user:/home/build:/bin/sh\n";

        assert_eq!(lookup_name_in(passwd, 61016).as_deref(), Some("build"));
        assert_eq!(lookup_name_in(passwd, 0).as_deref(), Some("root"));
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn an_unallocated_id_has_no_name() {
        let passwd =
            "root:x:0:0:root:/root:/bin/bash\ndaemon:x:1:1:daemon:/usr/sbin:/usr/sbin/nologin\n";

        assert_eq!(lookup_name_in(passwd, 61016), None);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn malformed_records_are_skipped() {
        let passwd = "\nnot-a-record\nshort:x\nbuild:x:61016:61016::/home/build:/bin/sh\n";

        assert_eq!(lookup_name_in(passwd, 61016).as_deref(), Some("build"));
        assert_eq!(lookup_name_in("", 61016), None);
    }

    #[test]
    fn resource_limits_defaults() {
        let limits = ResourceLimits::default();
        assert_eq!(limits.cpu_quota_us, None);
        assert_eq!(limits.cpu_period_us, 100_000);
        assert_eq!(limits.memory_bytes, None);
        assert_eq!(limits.max_fds, 1024);
        assert_eq!(limits.max_procs, 64);
        assert_eq!(limits.io_read_bps, None);
        assert_eq!(limits.io_write_bps, None);
    }

    #[test]
    fn with_cpu_limit_one_cpu() {
        let limits = ResourceLimits::default().with_cpu_limit(1.0);
        assert_eq!(limits.cpu_quota_us, Some(100_000));
    }

    #[test]
    fn with_cpu_limit_half_cpu() {
        let limits = ResourceLimits::default().with_cpu_limit(0.5);
        assert_eq!(limits.cpu_quota_us, Some(50_000));
    }

    #[test]
    fn with_cpu_limit_two_cpus() {
        let limits = ResourceLimits::default().with_cpu_limit(2.0);
        assert_eq!(limits.cpu_quota_us, Some(200_000));
    }

    #[test]
    fn with_memory_limit() {
        let limits = ResourceLimits::default().with_memory_limit(1024 * 1024 * 512);
        assert_eq!(limits.memory_bytes, Some(0x2000_0000));
    }

    #[test]
    fn with_io_limits() {
        let limits = ResourceLimits::default().with_io_limits(1_000_000, 500_000);
        assert_eq!(limits.io_read_bps, Some(1_000_000));
        assert_eq!(limits.io_write_bps, Some(500_000));
    }

    #[test]
    fn builder_chain() {
        let limits = ResourceLimits::default()
            .with_cpu_limit(2.0)
            .with_memory_limit(1024)
            .with_io_limits(100, 200);
        assert_eq!(limits.cpu_quota_us, Some(200_000));
        assert_eq!(limits.memory_bytes, Some(1024));
        assert_eq!(limits.io_read_bps, Some(100));
        assert_eq!(limits.io_write_bps, Some(200));
    }

    #[test]
    fn serde_round_trip() {
        let limits = ResourceLimits::default()
            .with_cpu_limit(1.5)
            .with_memory_limit(2048);
        let json = serde_json::to_string(&limits).unwrap();
        let parsed: ResourceLimits = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.cpu_quota_us, limits.cpu_quota_us);
        assert_eq!(parsed.memory_bytes, Some(2048));
        assert_eq!(parsed.cpu_period_us, 100_000);
    }

    #[test]
    fn serde_deserialize_minimal() {
        let json = "{}";
        let limits: ResourceLimits = serde_json::from_str(json).unwrap();
        assert_eq!(limits.cpu_quota_us, None);
        assert_eq!(limits.cpu_period_us, 100_000);
        assert_eq!(limits.max_procs, 64);
    }
}
