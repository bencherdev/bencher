//! Reaping the VMM a runner left behind.
//!
//! The sweep reclaims chroots because `Drop` does not run when a runner is
//! `SIGKILL`ed, crashes, or `exec`s itself during a self-update. The same exits
//! strand the jailed VMM: it is not signalled when its parent dies, so it is
//! reparented and keeps running, holding the benchmark cores through its
//! cgroup. Reclaiming only the disk leaves the more damaging half behind.
//!
//! The consequence is not leakage but wrong numbers that look right, and
//! nothing downstream catches it. The per-VM cgroups set only `cpuset.cpus`
//! and `cpuset.mems`, never `cpuset.cpus.exclusive`, so a leftover cgroup
//! claims nothing and does not narrow the next job's effective set: the next
//! job's cpuset applies cleanly and verifies cleanly while the stray VMM runs
//! untrusted guest code on the very same cores. The harm is contention, which
//! is invisible in the result.
//!
//! That is why a jail that cannot be cleared fails the job rather than merely
//! warning: it is the one remaining path where the runner would knowingly
//! emit a number it has reason to distrust.
//!
//! Killing a process the runner does not own is a destructive capability, so
//! the target is identified as narrowly as possible: a process whose root
//! directory *is* the chroot being swept, compared by device and inode, or a
//! member of the cgroup that chroot is named after, which is a UUID this runner
//! minted and nothing else on the host is in. A process merely owned by the jail
//! uid is not a target, because on a shared host that uid may legitimately own
//! something else.
//!
//! The cgroup is a second handle rather than a redundant one, because for a
//! moment it is the only handle there is: the jailer joins the cgroup in the
//! `pre_exec` of the runner's fork and `chroot`s much later, inside itself, just
//! before it execs Firecracker. See [`find_cgrouped_vmm`], which also says what
//! neither handle covers.

#![expect(clippy::print_stderr, reason = "reaping prints diagnostics")]

use std::fs;
use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd};
use std::os::unix::fs::MetadataExt as _;
use std::time::{Duration, Instant};

use camino::{Utf8Path, Utf8PathBuf};

/// How many processes to reap from one jail before giving up.
///
/// A jail holds one VMM, so this is a bound on a loop that should run once,
/// not an expectation.
const MAX_JAILED_PROCESSES: usize = 64;

/// How long to wait for a killed VMM to disappear.
const REAP_TIMEOUT: Duration = Duration::from_secs(5);

/// How often to check whether it has.
const REAP_INTERVAL: Duration = Duration::from_millis(20);

/// Whether a jail still has a VMM running in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reaped {
    /// Nothing is running in the jail: either nothing was, or it has exited.
    Clear,
    /// A VMM is still running in the jail and could not be reaped.
    ///
    /// The caller must not remove the chroot: doing so would pull the rootfs
    /// out from under a live process, and would destroy the only handle for
    /// identifying that process on a later sweep.
    StillRunning {
        /// The VMM that is still running.
        pid: u32,
    },
    /// The jail could not be examined, so what is in it is unknown.
    ///
    /// Distinct from [`Self::Clear`], which is the whole reason this variant
    /// exists: a jail root that cannot be stat'ed, or a `/proc` that cannot be
    /// listed, says nothing about the jail, and reporting nothing as "empty" is
    /// what would have the caller delete a tree with a live VMM in it. The
    /// caller treats it exactly like [`Self::StillRunning`], with no pid to
    /// name.
    Unexaminable,
}

/// Kill the VMM the jail at `jail_root` still holds, if one is still running.
///
/// Best effort about *which* process it touches: a VMM that cannot be
/// identified is left alone, because the alternative to leaving an
/// unidentified process alone is killing the wrong one. Never best effort
/// about what it reports, because the caller decides whether to delete a
/// directory based on the answer.
pub fn reap_jailed_vmm(jail_root: &Utf8Path) -> Reaped {
    reap_jailed_vmm_with(jail_root, find_jailed_vmm, reap_one)
}

/// The reap, with the scan and the kill injectable.
///
/// Both reach into `/proc` and signal processes on the machine running the
/// tests, so the exhaustion path is exercised through parameters rather than by
/// manufacturing sixty-four jailed processes.
fn reap_jailed_vmm_with<F, R>(jail_root: &Utf8Path, find: F, reap: R) -> Reaped
where
    F: Fn(&Utf8Path) -> std::io::Result<Option<u32>>,
    R: Fn(u32, &Utf8Path) -> Reaped,
{
    // Rescan after each reap rather than assuming one process per jail. That
    // assumption holds today, since neither `--daemonize` nor `--new-pid-ns`
    // is passed and the jailer execs in place as a single process, but the
    // caller deletes a directory tree based on this answer. An invariant that
    // load-bearing is worth enforcing rather than trusting, and a survivor
    // would otherwise have the tree removed out from under it.
    for _ in 0..MAX_JAILED_PROCESSES {
        match find(jail_root) {
            Ok(Some(pid)) => match reap(pid, jail_root) {
                // Killed, or already gone: look again, because a jail is not
                // clear until a scan finds nothing in it.
                Reaped::Clear => {},
                Reaped::StillRunning { pid } => return Reaped::StillRunning { pid },
                // `reap_one` cannot return this today, and it is matched
                // exhaustively so that a third outcome arriving here is a
                // compile error rather than a silent fall-through into another
                // pass of the loop. This module's whole argument is that a state
                // nobody could examine must not read as a cleared one, and an
                // `if let` is the construct that lets exactly that happen.
                Reaped::Unexaminable => return Reaped::Unexaminable,
            },
            Ok(None) => return Reaped::Clear,
            // A scan that could not run has not found the jail empty, it has
            // found out nothing. The caller deletes a tree on this answer.
            Err(e) => {
                eprintln!(
                    "Warning: cannot examine {jail_root} to see whether a VMM is still in it: {e}. It is left in place."
                );
                return Reaped::Unexaminable;
            },
        }
    }

    // Exhausting the bound is anomalous by construction: Firecracker does not
    // fork, the jailer execs in place, and a healthy jail therefore holds
    // exactly one process. Reaching here means either the scan and the kill
    // disagree, which is a bug and the likelier of the two, or something is
    // spawning into this jail, which is precisely when the sweep must refuse.
    // Neither says the jail is clear, so it is not reported clear.
    //
    // Nothing else would catch it. The cgroup removal that follows in the sweep
    // refuses while anything is still in the cgroup, but a run with no CPU
    // layout, or one whose host does not delegate cpuset, has no cgroup at all.
    // Failing the job is recoverable, because nothing latches and the next job
    // sweeps again; measuring through a jail that is still occupied is not, and
    // that is the one outcome this module exists to prevent.
    match find(jail_root) {
        Ok(Some(pid)) => {
            eprintln!(
                "Warning: gave up scanning {jail_root} after {MAX_JAILED_PROCESSES} passes; pid {pid} still matches it while every reap reported the jail clear."
            );
            Reaped::StillRunning { pid }
        },
        Ok(None) => Reaped::Clear,
        Err(e) => {
            eprintln!(
                "Warning: cannot examine {jail_root} to see whether a VMM is still in it: {e}. It is left in place."
            );
            Reaped::Unexaminable
        },
    }
}

/// Kill one process known to belong to the jail at `jail_root`.
fn reap_one(pid: u32, jail_root: &Utf8Path) -> Reaped {
    // Pin the process before signalling it. A pid found by scanning `/proc` can
    // exit and have its number recycled before the signal lands, and this runs
    // as root, so a `kill` on the number would go to whatever inherited it.
    //
    // A pidfd does not hold the number back: a pid is freed the moment the
    // process is reaped, open descriptors notwithstanding. What it holds is the
    // identity. `pidfd_send_signal` can only ever reach the process the
    // descriptor was opened on, and reaches nothing at all once that process is
    // gone, so a number recycled between the check below and the kill cannot
    // carry the signal to a stranger. That is what turns the check into a
    // guarantee rather than a narrow window.
    let pidfd = match pidfd_open(pid) {
        Ok(Some(pidfd)) => pidfd,
        // Already gone, which is the common case and not a failure.
        Ok(None) => return Reaped::Clear,
        Err(e) => {
            // Silence here is what wedges a runner: the orphan keeps the
            // benchmark CPUs, the cgroup cannot be removed, and nothing says
            // why. Kernels before 5.3 have no pidfd_open at all.
            eprintln!(
                "Warning: cannot pin orphaned VMM (pid {pid}) in {jail_root} to reap it: {e}. It is still running and still holds the benchmark CPUs."
            );
            return Reaped::StillRunning { pid };
        },
    };

    // Re-check now that the pid cannot change underneath us. If it is no
    // longer the jail's VMM, the jail is clear whatever else is true.
    if !is_jailed_vmm(pid, jail_root) {
        return Reaped::Clear;
    }

    if let Err(e) = pidfd_kill(&pidfd) {
        eprintln!("Warning: failed to kill orphaned VMM (pid {pid}) in {jail_root}: {e}");
        return Reaped::StillRunning { pid };
    }

    if wait_for_exit(&pidfd) {
        eprintln!("Reaped orphaned VMM (pid {pid}) left behind in {jail_root}");
        Reaped::Clear
    } else {
        eprintln!(
            "Warning: orphaned VMM (pid {pid}) in {jail_root} did not exit within {} seconds",
            REAP_TIMEOUT.as_secs()
        );
        Reaped::StillRunning { pid }
    }
}

/// Find the pid of a process the jail at `jail_root` still holds.
///
/// Both handles, narrowest first: a process chrooted into the jail, and failing
/// that a process in the jail's cgroup.
fn find_jailed_vmm(jail_root: &Utf8Path) -> std::io::Result<Option<u32>> {
    match find_chrooted_vmm(jail_root)? {
        Some(pid) => Ok(Some(pid)),
        None => find_cgrouped_vmm(jail_root),
    }
}

/// Find the pid of the VMM whose root directory is `jail_root`.
///
/// The jailer `chroot`s before exec, so the confined process's root *is* the
/// chroot. Comparing device and inode rather than the path is what makes this
/// exact: the jailer pivots into a private mount namespace, so the path reads
/// back as `/`, while the identity is preserved.
///
/// `Ok(None)` is "nothing is confined here", which a jail root that is not there
/// at all also means: nothing can be chrooted into a directory that does not
/// exist. Every other failure is an error rather than an absence, because the
/// caller deletes a directory tree on the strength of this answer and a scan
/// that could not run has established nothing.
fn find_chrooted_vmm(jail_root: &Utf8Path) -> std::io::Result<Option<u32>> {
    let jail = match fs::metadata(jail_root) {
        Ok(jail) => jail,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    for entry in fs::read_dir("/proc")? {
        let entry = entry?;
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        if matches_jail(pid, &jail) {
            return Ok(Some(pid));
        }
    }
    Ok(None)
}

/// Find the pid of a process in the cgroup of the jail at `jail_root`.
///
/// The chroot is not the jail's only handle on its VMM, and for a moment it is
/// not a handle at all. Cgroup placement happens in the `pre_exec` of the
/// runner's fork, before the jailer runs; the `chroot` happens inside the jailer
/// much later, just before it execs Firecracker. A runner `SIGKILL`ed in between
/// orphans a jailer that is already a member of this jail's cgroup and is still
/// rooted at `/`, which the scan above cannot see. The sweep would then call the
/// jail clear and delete the chroot out from under it, and a jailer that went on
/// to finish its `exec` would leave a Firecracker rooted at a deleted directory
/// that no later scan could ever match: a stray on the benchmark cores with
/// nothing left to find it by.
///
/// Membership identifies the target as narrowly as the root inode does, because
/// the cgroup is named by the same id as the chroot and that id is a UUID this
/// runner minted.
///
/// What neither handle covers: a run with no CPU layout, and a host that does
/// not delegate cpuset, have no cgroup at all, so a jailer orphaned before its
/// `chroot` on one of those is invisible to the sweep. Saying so is better than
/// leaving the sweep looking complete.
fn find_cgrouped_vmm(jail_root: &Utf8Path) -> std::io::Result<Option<u32>> {
    match jail_cgroup_procs(jail_root) {
        Some(procs) => first_cgroup_member(&procs),
        None => Ok(None),
    }
}

/// The first process listed in a `cgroup.procs`, if there is one.
///
/// A listing that is not there is a cgroup that is not there, and nothing can be
/// in a cgroup that does not exist. Every other failure is an error rather than
/// an absence, for the same reason as in the scan above: the caller deletes a
/// directory tree on this answer.
///
/// The runner's own pid is skipped. It is never in a jail's cgroup, and what one
/// comparison against a pid that cannot be there buys is that a future path
/// which does put it there cannot have the sweep `SIGKILL` the runner itself.
fn first_cgroup_member(procs: &Utf8Path) -> std::io::Result<Option<u32>> {
    let listing = match fs::read_to_string(procs) {
        Ok(listing) => listing,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    Ok(listing
        .lines()
        .filter_map(|line| line.trim().parse::<u32>().ok())
        .find(|&pid| pid != std::process::id()))
}

/// The `cgroup.procs` of the cgroup belonging to the jail at `jail_root`.
///
/// The chroot and the cgroup are named by the same VM id by construction, and
/// the jail root sits one directory under that id, so the id is the name of
/// `jail_root`'s parent. The same pairing the sweep uses in the other direction
/// to remove the cgroup of a jail it found on disk.
///
/// `None` is a path with no parent to read an id from, which is not a jail this
/// runner built.
fn jail_cgroup_procs(jail_root: &Utf8Path) -> Option<Utf8PathBuf> {
    let vm_id = jail_root.parent()?.file_name()?;
    Some(super::cgroup::vm_cgroup(vm_id).join("cgroup.procs"))
}

/// Whether a process is one the jail at `jail_root` still holds.
///
/// Either handle: rooted at the chroot, or a member of the jail's cgroup.
///
/// A jail root that cannot be stat'ed, or a listing that cannot be read, reads
/// as "not this jail's VMM", which keeps the kill from landing on a process this
/// function cannot vouch for. The caller's next scan turns the same failure into
/// [`Reaped::Unexaminable`], so nothing downstream mistakes it for an empty
/// jail.
fn is_jailed_vmm(pid: u32, jail_root: &Utf8Path) -> bool {
    fs::metadata(jail_root).is_ok_and(|jail| matches_jail(pid, &jail))
        || jail_cgroup_procs(jail_root).is_some_and(|procs| is_cgroup_member(&procs, pid))
}

/// Whether a `cgroup.procs` lists `pid`.
fn is_cgroup_member(procs: &Utf8Path, pid: u32) -> bool {
    fs::read_to_string(procs).is_ok_and(|listing| super::cgroup::procs_contains_pid(&listing, pid))
}

/// Whether a process's root directory is the same inode as `jail`.
///
/// A `/proc/<pid>/root` that cannot be read means the process is gone or is not
/// one this runner may inspect, and either way it is not the jail's VMM. That is
/// the one failure here that is genuinely nothing: processes come and go under a
/// scan of `/proc` constantly.
fn matches_jail(pid: u32, jail: &fs::Metadata) -> bool {
    // Following this magic symlink crosses into the process's own mount
    // namespace, which a privileged reader is allowed to do.
    fs::metadata(format!("/proc/{pid}/root"))
        .is_ok_and(|root| root.dev() == jail.dev() && root.ino() == jail.ino())
}

/// Open a descriptor pinned to a process.
///
/// `Ok(None)` means the process is already gone, which is the common case and
/// not a failure. Everything else is distinguished and reported by the caller,
/// because a reap that quietly does nothing leaves an orphan holding the
/// benchmark CPUs: `ENOSYS` on a kernel before 5.3, `EPERM` under a
/// restrictive policy, and a pid too large to convert all look identical from
/// the outside otherwise.
fn pidfd_open(pid: u32) -> std::io::Result<Option<OwnedFd>> {
    let pid = libc::pid_t::try_from(pid).map_err(|_err| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "pid out of range")
    })?;

    #[expect(
        unsafe_code,
        reason = "pidfd_open has no std wrapper; it takes plain integers"
    )]
    // SAFETY: `pidfd_open` takes a pid and a flag word and touches no memory.
    // It returns a new descriptor or -1, and the descriptor is handed straight
    // to `OwnedFd` so it is closed exactly once. Every argument is widened to
    // `c_long` explicitly: `syscall` is variadic, so the width each value is
    // passed at is the width written here rather than anything the signature
    // enforces.
    let raw = unsafe {
        libc::syscall(
            libc::SYS_pidfd_open,
            libc::c_long::from(pid),
            libc::c_long::from(0i32),
        )
    };

    if raw < 0 {
        let error = std::io::Error::last_os_error();
        return if error.raw_os_error() == Some(libc::ESRCH) {
            Ok(None)
        } else {
            Err(error)
        };
    }

    let raw = libc::c_int::try_from(raw)
        .map_err(|_err| std::io::Error::other("pidfd out of descriptor range"))?;
    #[expect(
        unsafe_code,
        reason = "taking ownership of a descriptor this call just created"
    )]
    // SAFETY: `raw` is a fresh descriptor returned by the syscall above and is
    // not owned by anything else.
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    Ok(Some(fd))
}

/// SIGKILL the process a descriptor is pinned to.
///
/// SIGKILL rather than a graceful shutdown: the job that owned this VMM is
/// already gone, so there is nothing left to shut down cleanly for.
fn pidfd_kill(pidfd: &OwnedFd) -> std::io::Result<()> {
    #[expect(
        unsafe_code,
        reason = "pidfd_send_signal has no std wrapper; the fd is owned and valid"
    )]
    // SAFETY: `pidfd` is an open, owned descriptor for the duration of the
    // call. A null `siginfo` pointer is the documented way to ask the kernel
    // to synthesize one, and the final argument is a reserved flag word. The
    // integers are widened to `c_long` explicitly, since a variadic call passes
    // each value at the width it is written at.
    let ret = unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            libc::c_long::from(pidfd.as_raw_fd()),
            libc::c_long::from(libc::SIGKILL),
            std::ptr::null::<libc::siginfo_t>(),
            libc::c_long::from(0i32),
        )
    };
    if ret == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Wait for a killed process to stop running.
///
/// Waits on the descriptor rather than on the number, because the number is the
/// part that goes stale. The VMM can exit, be reaped by init, and have its pid
/// handed to something unrelated well inside this budget, and a wait watching
/// `/proc/<pid>` would then run the budget out against a stranger and report the
/// VMM still running: a job failed on a clean host, with an error confidently
/// naming a pid that is already gone. The descriptor is the identity, so it
/// cannot be answered by whatever inherited the number.
///
/// A zombie counts as exited, which the descriptor gives for free: it becomes
/// readable when its process terminates, whether or not anything ever reaps it.
/// That case is not hypothetical. The orphan reparents to whatever is PID 1, and
/// if the runner is itself PID 1 (a container with no init) nothing ever reaps
/// it, so it stays a zombie forever.
fn wait_for_exit(pidfd: &OwnedFd) -> bool {
    wait_for_exit_until(Instant::now() + REAP_TIMEOUT, || is_running(pidfd))
}

/// The wait, with the deadline and the liveness check supplied.
///
/// Parameters for the same reason the scan and the kill are: the case worth
/// testing is a process that exits in the last interval of the budget, and
/// reproducing that against a real process means racing a sleep.
fn wait_for_exit_until<R>(deadline: Instant, running: R) -> bool
where
    R: Fn() -> bool,
{
    while Instant::now() < deadline {
        if !running() {
            return true;
        }
        std::thread::sleep(REAP_INTERVAL);
    }

    // Once more before the verdict. The loop sleeps between checks, so a process
    // that exits during the last of them was never looked at again, and a bare
    // `false` here becomes `JailError::JailStillRunning`: a job failed on a host
    // that is clean, with an error confidently naming a pid that is already
    // gone. The same shape as the readiness wait in the VMM process, where it
    // cost a misleading message rather than a spurious failure.
    !running()
}

/// Whether the process a descriptor is pinned to has yet to terminate.
///
/// A pidfd becomes readable exactly when its process terminates, so a `poll`
/// that reports nothing readable is a process still running. The timeout is
/// zero because the budget belongs to [`wait_for_exit_until`]; this answers the
/// question as it stands right now.
///
/// A `poll` that fails, or reports something other than readable, has answered
/// nothing, and is reported as still running. That direction is deliberate: the
/// caller turns it into `JailStillRunning` and fails a job on a host that may
/// well be clean, while the other direction reads a question that failed as an
/// exit and has the sweep delete a tree with a live VMM under it.
fn is_running(pidfd: &OwnedFd) -> bool {
    let mut poll_fd = libc::pollfd {
        fd: pidfd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };

    #[expect(
        unsafe_code,
        reason = "poll has no std wrapper; the fd is owned and valid"
    )]
    // SAFETY: `poll` reads and writes exactly the `nfds` entries the pointer
    // names, and one entry is passed for the one that is declared. `pidfd` is an
    // open, owned descriptor for the duration of the call.
    let ready = unsafe { libc::poll(&raw mut poll_fd, 1, 0) };

    ready <= 0 || (poll_fd.revents & libc::POLLIN) == 0
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;

    use super::*;

    #[test]
    fn no_process_is_rooted_at_an_ordinary_directory() {
        // The identification has to be narrow enough that a directory no
        // process is chrooted into matches nothing at all.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

        assert_eq!(find_jailed_vmm(&root).unwrap(), None);
    }

    #[test]
    fn a_missing_jail_matches_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

        assert_eq!(find_jailed_vmm(&root.join("absent")).unwrap(), None);
        assert!(!is_jailed_vmm(std::process::id(), &root.join("absent")));
    }

    #[test]
    fn the_runners_own_root_is_matched_by_identity_not_by_path() {
        // The runner is rooted at `/`, so it is found when `/` is the jail and
        // not otherwise. This is the mechanism the reap depends on: a jailed
        // VMM's root path reads back as `/` too, so only the inode separates
        // them.
        //
        // It also pins the subtler half. `/proc/<pid>/root` is a magic
        // symlink, and the comparison has to follow it: `metadata` does,
        // `symlink_metadata` would return the procfs link's own inode and
        // match nothing ever. This assertion fails if that changes.
        let root = Utf8Path::new("/");

        assert!(is_jailed_vmm(std::process::id(), root));

        let dir = tempfile::tempdir().unwrap();
        let other = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        assert!(!is_jailed_vmm(std::process::id(), &other));
    }

    #[test]
    fn a_jails_cgroup_is_the_one_its_chroot_is_named_after() {
        // The pairing the sweep already relies on to remove the cgroup of a jail
        // it found on disk, read the other way round. The id is the directory
        // over the jail root, not the jail root itself.
        let procs = jail_cgroup_procs(Utf8Path::new("/srv/runner/jail/vm-1/root")).unwrap();

        assert_eq!(
            procs,
            Utf8Path::new("/sys/fs/cgroup/bencher/vm-1/cgroup.procs")
        );
        assert_eq!(jail_cgroup_procs(Utf8Path::new("/")), None);
    }

    #[test]
    fn a_jailer_orphaned_before_its_chroot_is_found_by_its_cgroup() {
        // Cgroup placement happens in the fork's `pre_exec` and the `chroot`
        // only inside the jailer, so a runner killed in between leaves a jailer
        // that is in the cgroup and still rooted at `/`. Nothing else finds it,
        // and calling the jail clear would delete the chroot out from under a
        // process about to `exec` into it.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let procs = root.join("cgroup.procs");
        fs::write(&procs, "4242\n").unwrap();

        assert_eq!(first_cgroup_member(&procs).unwrap(), Some(4242));
        assert!(is_cgroup_member(&procs, 4242));
        assert!(!is_cgroup_member(&procs, 42));
    }

    #[test]
    fn the_runner_is_never_a_process_the_sweep_reaps() {
        // The runner does not put itself in a jail's cgroup. This costs one
        // comparison and makes sure a path that someday does cannot have the
        // sweep SIGKILL the runner.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let procs = root.join("cgroup.procs");
        fs::write(&procs, format!("{}\n", std::process::id())).unwrap();

        assert_eq!(first_cgroup_member(&procs).unwrap(), None);
    }

    #[test]
    fn a_cgroup_listing_that_cannot_be_read_is_not_an_empty_cgroup() {
        // A jail with no cgroup is ordinary: a run with no CPU layout never
        // creates one. A listing that is there and could not be read is not
        // that, and the caller deletes a directory tree on the difference.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let absent = root.join("cgroup.procs");
        let unreadable = root.join("unreadable");
        fs::create_dir_all(unreadable.join("cgroup.procs")).unwrap();

        assert_eq!(first_cgroup_member(&absent).unwrap(), None);
        first_cgroup_member(&unreadable.join("cgroup.procs")).unwrap_err();
        assert!(!is_cgroup_member(&absent, 4242));
    }

    #[test]
    fn a_pidfd_pins_a_live_process() {
        let pidfd = pidfd_open(std::process::id()).expect("pidfd_open is available");
        assert!(pidfd.is_some(), "this process is alive");
    }

    #[test]
    fn a_pidfd_distinguishes_gone_from_broken() {
        // Pid 0 is never a process, and the syscall rejects it rather than
        // signalling the caller's process group the way plain `kill` would.
        // The distinction matters: an absent process is nothing to report,
        // while a refusal leaves an orphan running and has to be said out loud.
        match pidfd_open(0) {
            Ok(None) => {},
            Ok(Some(_)) => panic!("pid 0 must never yield a pidfd"),
            Err(e) => assert_ne!(
                e.raw_os_error(),
                Some(libc::ESRCH),
                "ESRCH must be reported as gone, not as an error"
            ),
        }
    }

    #[test]
    fn a_process_that_exits_in_the_last_interval_counts_as_exited() {
        // A deadline already spent, so the loop never runs and the check after
        // it is the whole verdict. Without that check the caller fails the job
        // on a host that is clean, naming a pid that no longer exists.
        let checks = std::cell::Cell::new(0);

        let exited = wait_for_exit_until(Instant::now(), || {
            checks.set(checks.get() + 1);
            false
        });

        assert!(exited, "the process is gone, whatever the budget did");
        assert_eq!(checks.get(), 1, "the check after the loop is the verdict");
    }

    #[test]
    fn a_process_that_outlives_the_budget_is_still_running() {
        assert!(!wait_for_exit_until(Instant::now(), || true));
    }

    #[test]
    fn a_pinned_live_process_reads_as_running() {
        let pidfd = pidfd_open(std::process::id())
            .expect("pidfd_open is available")
            .expect("this process is alive");

        assert!(is_running(&pidfd));
    }

    #[test]
    fn a_zombie_nobody_reaps_counts_as_exited() {
        // A child that has exited and has not been waited on still holds its
        // number and still has a `/proc/<pid>`, and nothing ever clears either
        // when the runner is PID 1 in a container with no init. The wait has to
        // call that exited, or every sweep on such a host spends the whole
        // budget and then warns about a process that is already dead.
        let mut child = std::process::Command::new("/bin/sh")
            .args(["-c", "exit 0"])
            .spawn()
            .unwrap();
        let pidfd = pidfd_open(child.id()).unwrap().unwrap();

        // Deliberately before the `wait`: reaping it first would leave nothing
        // to distinguish a zombie from a process that is fully gone.
        assert!(wait_for_exit(&pidfd));

        child.wait().unwrap();
    }

    #[test]
    fn a_reaped_process_is_waited_on_by_identity_not_by_number() {
        // The failure this prevents: the VMM exits, init reaps it, and its
        // number is handed to something else inside the budget. A wait watching
        // `/proc/<pid>` would spend the whole budget on a stranger and then fail
        // the job naming a pid that is not the VMM. The descriptor answers for
        // the process it was opened on, and that one is gone.
        let mut child = std::process::Command::new("/bin/sh")
            .args(["-c", "exit 0"])
            .spawn()
            .unwrap();
        let pidfd = pidfd_open(child.id()).unwrap().unwrap();
        child.wait().unwrap();

        assert!(wait_for_exit(&pidfd));
    }

    #[test]
    fn reaping_an_unjailed_directory_kills_nothing_and_reports_clear() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

        assert_eq!(reap_jailed_vmm(&root), Reaped::Clear);
    }

    #[test]
    fn a_survivor_is_reported_without_exhausting_the_scan() {
        // A process the reap could not kill is the answer immediately: the
        // caller must not delete the tree under it, and there is nothing to be
        // gained by rescanning.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let scans = std::cell::Cell::new(0);

        let reaped = reap_jailed_vmm_with(
            &root,
            |_jail_root| {
                scans.set(scans.get() + 1);
                Ok(Some(99))
            },
            |pid, _jail_root| Reaped::StillRunning { pid },
        );

        assert_eq!(reaped, Reaped::StillRunning { pid: 99 });
        assert_eq!(scans.get(), 1, "a survivor is the answer on the first pass");
    }

    #[test]
    fn a_jail_that_keeps_producing_processes_is_never_called_clear() {
        // A healthy jail holds one process, so exhausting the bound is either
        // the scan disagreeing with the kill or something spawning into the
        // jail. Neither is evidence the jail is clear, and reporting it clear
        // would have the sweep delete the tree and the next job measure through
        // whatever is left. The cgroup removal cannot be leaned on here: a run
        // with no CPU layout has no cgroup to refuse.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let reaps = std::cell::Cell::new(0);

        let reaped = reap_jailed_vmm_with(
            &root,
            |_jail_root| Ok(Some(7)),
            |_pid, _jail_root| {
                reaps.set(reaps.get() + 1);
                Reaped::Clear
            },
        );

        assert_eq!(reaped, Reaped::StillRunning { pid: 7 });
        assert_eq!(
            reaps.get(),
            MAX_JAILED_PROCESSES,
            "the loop is bounded rather than endless"
        );
    }

    #[test]
    fn a_jail_that_empties_on_the_last_pass_is_clear() {
        // The bound is a guard, not a verdict: a jail that no longer matches
        // anything is clear however many passes it took to get there.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let scans = std::cell::Cell::new(0);

        let reaped = reap_jailed_vmm_with(
            &root,
            |_jail_root| {
                scans.set(scans.get() + 1);
                Ok((scans.get() <= MAX_JAILED_PROCESSES).then_some(7))
            },
            |_pid, _jail_root| Reaped::Clear,
        );

        assert_eq!(reaped, Reaped::Clear);
        assert_eq!(
            scans.get(),
            MAX_JAILED_PROCESSES + 1,
            "the scan after the loop is what decides"
        );
    }

    #[test]
    fn a_jail_that_cannot_be_examined_is_not_reported_clear() {
        // The caller deletes a directory tree on this answer, so a scan that
        // could not run must not read as an empty jail. A jail root that is
        // simply absent is a different thing and stays clear: nothing can be
        // chrooted into a directory that is not there.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

        let reaped = reap_jailed_vmm_with(
            &root,
            |_jail_root| Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
            |_pid, _jail_root| Reaped::Clear,
        );

        assert_eq!(reaped, Reaped::Unexaminable);
        assert_eq!(reap_jailed_vmm(&root.join("absent")), Reaped::Clear);
    }

    #[test]
    fn a_scan_that_fails_after_the_bound_is_not_reported_clear() {
        // The same rule on the way out of the loop as on the way in.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let scans = std::cell::Cell::new(0);

        let reaped = reap_jailed_vmm_with(
            &root,
            |_jail_root| {
                scans.set(scans.get() + 1);
                if scans.get() > MAX_JAILED_PROCESSES {
                    Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied))
                } else {
                    Ok(Some(7))
                }
            },
            |_pid, _jail_root| Reaped::Clear,
        );

        assert_eq!(reaped, Reaped::Unexaminable);
    }

    #[test]
    fn a_still_running_vmm_carries_its_pid() {
        // The caller keys the decision not to delete a directory off this, so
        // the variant has to name the process it is refusing to abandon.
        let still = Reaped::StillRunning { pid: 4242 };
        assert_ne!(still, Reaped::Clear);
        match still {
            Reaped::StillRunning { pid } => assert_eq!(pid, 4242),
            Reaped::Clear | Reaped::Unexaminable => panic!("expected StillRunning"),
        }
    }
}
