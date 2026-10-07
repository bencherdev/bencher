//! Reaping a VMM stranded by a runner that exited without unwinding, which
//! would otherwise share the next job's cores and silently skew its results.

use std::fs;
use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd};
use std::os::unix::fs::MetadataExt as _;
use std::time::{Duration, Instant};

use camino::{Utf8Path, Utf8PathBuf};
use slog::{Logger, info, warn};

/// Bound on reaps per jail; a healthy jail holds one VMM, so the loop should
/// run once.
const MAX_JAILED_PROCESSES: usize = 64;

const REAP_TIMEOUT: Duration = Duration::from_secs(5);

const REAP_INTERVAL: Duration = Duration::from_millis(20);

/// Whether a jail still has a VMM running in it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reaped {
    Clear,
    /// A VMM survived the reap, so the caller must not remove the chroot, the
    /// only handle a later sweep has on that process.
    StillRunning {
        /// The survivor, which the caller names in `JailStillRunning`.
        pid: u32,
    },
    /// The jail could not be examined, which the caller must treat like
    /// [`Self::StillRunning`] and never like [`Self::Clear`].
    Unexaminable,
}

/// Kill the VMM still in the jail at `jail_root`, leaving alone any process it
/// cannot identify rather than risk killing the wrong one.
pub fn reap_jailed_vmm(log: &Logger, jail_root: &Utf8Path) -> Reaped {
    reap_jailed_vmm_with(log, jail_root, find_jailed_vmm, |pid, jail_root| {
        reap_one(log, pid, jail_root)
    })
}

/// `reap_jailed_vmm` with the scan and the kill injectable, so tests reach the
/// exhaustion path without real jailed processes.
fn reap_jailed_vmm_with<F, R>(log: &Logger, jail_root: &Utf8Path, find: F, reap: R) -> Reaped
where
    F: Fn(&Utf8Path) -> std::io::Result<Option<u32>>,
    R: Fn(u32, &Utf8Path) -> Reaped,
{
    // Rescan after each reap rather than trusting one process per jail, because
    // the caller deletes the tree on this answer.
    for _ in 0..MAX_JAILED_PROCESSES {
        match find(jail_root) {
            Ok(Some(pid)) => match reap(pid, jail_root) {
                // A jail is not clear until a scan finds nothing in it.
                Reaped::Clear => {},
                Reaped::StillRunning { pid } => return Reaped::StillRunning { pid },
                Reaped::Unexaminable => return Reaped::Unexaminable,
            },
            Ok(None) => return Reaped::Clear,
            Err(e) => {
                warn!(log, "Jail unexaminable, left in place"; "jail_root" => jail_root.as_str(), "error" => %e);
                return Reaped::Unexaminable;
            },
        }
    }

    // An exhausted bound means the scan and kill disagree or something keeps
    // spawning into the jail, so only a final empty scan reports it clear.
    match find(jail_root) {
        Ok(Some(pid)) => {
            // Every reap reported the jail clear, yet the pid still matches it.
            warn!(log, "Jail scan gave up";
                "jail_root" => jail_root.as_str(),
                "passes" => MAX_JAILED_PROCESSES,
                "pid" => pid,
            );
            Reaped::StillRunning { pid }
        },
        Ok(None) => Reaped::Clear,
        Err(e) => {
            warn!(log, "Jail unexaminable, left in place"; "jail_root" => jail_root.as_str(), "error" => %e);
            Reaped::Unexaminable
        },
    }
}

fn reap_one(log: &Logger, pid: u32, jail_root: &Utf8Path) -> Reaped {
    // Pin the process with a pidfd before signalling it, so a pid recycled
    // after the check below cannot carry the `SIGKILL` to a stranger.
    let pidfd = match pidfd_open(pid) {
        Ok(Some(pidfd)) => pidfd,
        Ok(None) => return Reaped::Clear,
        Err(e) => {
            // It is still running and still holds the benchmark CPUs.
            warn!(log, "Orphaned VMM not pinned to reap";
                "pid" => pid,
                "jail_root" => jail_root.as_str(),
                "error" => %e,
            );
            return Reaped::StillRunning { pid };
        },
    };

    // Re-check against the pinned process, since the scanned pid may already
    // name a different one.
    if !is_jailed_vmm(pid, jail_root) {
        return Reaped::Clear;
    }

    if let Err(e) = pidfd_kill(&pidfd) {
        warn!(log, "Orphaned VMM not killed";
            "pid" => pid,
            "jail_root" => jail_root.as_str(),
            "error" => %e,
        );
        return Reaped::StillRunning { pid };
    }

    if wait_for_exit(&pidfd) {
        info!(log, "Reaped orphaned VMM"; "pid" => pid, "jail_root" => jail_root.as_str());
        Reaped::Clear
    } else {
        warn!(log, "Orphaned VMM did not exit";
            "pid" => pid,
            "jail_root" => jail_root.as_str(),
            "timeout_secs" => REAP_TIMEOUT.as_secs(),
        );
        Reaped::StillRunning { pid }
    }
}

/// Find a process the jail holds by its root or its cgroup, never by the jail
/// uid, which may own unrelated processes on a shared host.
fn find_jailed_vmm(jail_root: &Utf8Path) -> std::io::Result<Option<u32>> {
    match find_chrooted_vmm(jail_root)? {
        Some(pid) => Ok(Some(pid)),
        None => find_cgrouped_vmm(jail_root),
    }
}

/// Find the process whose root is `jail_root`, compared by device and inode
/// because inside the jailer's mount namespace the path reads back as `/`.
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

/// Find a process in the jail's cgroup, the only handle on a jailer orphaned
/// between joining it in `pre_exec` and its `chroot`; a run with no cgroup has
/// no handle on that jailer at all.
fn find_cgrouped_vmm(jail_root: &Utf8Path) -> std::io::Result<Option<u32>> {
    match jail_cgroup_procs(jail_root) {
        Some(procs) => first_cgroup_member(&procs),
        None => Ok(None),
    }
}

/// The first pid in a `cgroup.procs` other than the runner's own, which is
/// skipped so the sweep can never `SIGKILL` the runner.
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

/// The jail's `cgroup.procs`, whose cgroup is named by the VM id that also
/// names `jail_root`'s parent.
fn jail_cgroup_procs(jail_root: &Utf8Path) -> Option<Utf8PathBuf> {
    let vm_id = jail_root.parent()?.file_name()?;
    Some(super::cgroup::vm_cgroup(vm_id).join("cgroup.procs"))
}

/// Whether `pid` is rooted at the jail or in its cgroup, reading any failure as
/// no so the kill never lands on a process it cannot vouch for.
fn is_jailed_vmm(pid: u32, jail_root: &Utf8Path) -> bool {
    fs::metadata(jail_root).is_ok_and(|jail| matches_jail(pid, &jail))
        || jail_cgroup_procs(jail_root).is_some_and(|procs| is_cgroup_member(&procs, pid))
}

fn is_cgroup_member(procs: &Utf8Path, pid: u32) -> bool {
    fs::read_to_string(procs).is_ok_and(|listing| super::cgroup::procs_contains_pid(&listing, pid))
}

/// Whether a process's root is `jail`, an unreadable `/proc/<pid>/root` meaning
/// no because processes come and go under a scan.
fn matches_jail(pid: u32, jail: &fs::Metadata) -> bool {
    // Following this magic symlink crosses into the process's own mount
    // namespace, which a privileged reader is allowed to do.
    fs::metadata(format!("/proc/{pid}/root"))
        .is_ok_and(|root| root.dev() == jail.dev() && root.ino() == jail.ino())
}

/// Open a descriptor pinned to a process, `Ok(None)` meaning it is already gone.
fn pidfd_open(pid: u32) -> std::io::Result<Option<OwnedFd>> {
    let pid = libc::pid_t::try_from(pid).map_err(|_err| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, "pid out of range")
    })?;

    #[expect(
        unsafe_code,
        reason = "pidfd_open has no std wrapper; it takes plain integers"
    )]
    // SAFETY: `pidfd_open` touches no memory, and each argument is widened to
    // `c_long` because `syscall` is variadic.
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

/// SIGKILL the pinned process, with no graceful shutdown because the job that
/// owned it is already gone.
fn pidfd_kill(pidfd: &OwnedFd) -> std::io::Result<()> {
    #[expect(
        unsafe_code,
        reason = "pidfd_send_signal has no std wrapper; the fd is owned and valid"
    )]
    // SAFETY: `pidfd` is open for the call, a null `siginfo` asks the kernel to
    // synthesize one, and each integer is widened to `c_long` for `syscall`.
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

/// Wait on the descriptor rather than the pid, so a recycled number cannot
/// stall the wait and a zombie nobody reaps still counts as exited.
fn wait_for_exit(pidfd: &OwnedFd) -> bool {
    wait_for_exit_until(Instant::now() + REAP_TIMEOUT, || is_running(pidfd))
}

/// `wait_for_exit` with the deadline and liveness check injectable, so a test
/// can hit the last interval without racing a sleep.
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

    // Once more, or a process that exits during the final sleep fails a clean
    // job as `JailError::JailStillRunning`.
    !running()
}

/// Whether the pinned process is still running, reading a failed `poll` as
/// running so the sweep never deletes a tree over a live VMM.
fn is_running(pidfd: &OwnedFd) -> bool {
    let mut poll_fd = libc::pollfd {
        fd: pidfd.as_raw_fd(),
        events: libc::POLLIN,
        revents: 0,
    };

    let ready = loop {
        #[expect(
            unsafe_code,
            reason = "poll has no std wrapper; the fd is owned and valid"
        )]
        // SAFETY: `poll` touches only the one entry passed, and `pidfd` is open
        // for the call.
        let ready = unsafe { libc::poll(&raw mut poll_fd, 1, 0) };
        if ready < 0 && std::io::Error::last_os_error().kind() == std::io::ErrorKind::Interrupted {
            continue;
        }
        break ready;
    };

    ready <= 0 || (poll_fd.revents & libc::POLLIN) == 0
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;

    use super::*;
    use crate::log::discard;

    #[test]
    fn no_process_is_rooted_at_an_ordinary_directory() {
        // Fails if the identification is loose enough to match a directory no
        // process is chrooted into.
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
        // Fails if the match compares paths, or reads `/proc/<pid>/root` with
        // `symlink_metadata` and so gets the link's own inode.
        let root = Utf8Path::new("/");

        assert!(is_jailed_vmm(std::process::id(), root));

        let dir = tempfile::tempdir().unwrap();
        let other = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        assert!(!is_jailed_vmm(std::process::id(), &other));
    }

    #[test]
    fn a_jails_cgroup_is_the_one_its_chroot_is_named_after() {
        // Fails if the cgroup id is taken from the jail root instead of its
        // parent directory.
        let procs = jail_cgroup_procs(Utf8Path::new("/srv/runner/jail/vm-1/root")).unwrap();

        assert_eq!(
            procs,
            Utf8Path::new("/sys/fs/cgroup/bencher/vm-1/cgroup.procs")
        );
        assert_eq!(jail_cgroup_procs(Utf8Path::new("/")), None);
    }

    #[test]
    fn a_jailer_orphaned_before_its_chroot_is_found_by_its_cgroup() {
        // Fails if the cgroup listing is misread, leaving a jailer orphaned
        // before its `chroot` (still rooted at `/`) unfound.
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
        // Fails if the runner's own pid in a jail's cgroup would have the sweep
        // SIGKILL the runner.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let procs = root.join("cgroup.procs");
        fs::write(&procs, format!("{}\n", std::process::id())).unwrap();

        assert_eq!(first_cgroup_member(&procs).unwrap(), None);
    }

    #[test]
    fn a_cgroup_listing_that_cannot_be_read_is_not_an_empty_cgroup() {
        // Fails if an unreadable listing reads as an empty cgroup, which the
        // caller would take as license to delete the tree.
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
        // Fails if a gone process (`ESRCH`) is reported as an error instead of
        // as `Ok(None)`.
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
        // Fails without the check after the loop, which a spent deadline makes
        // the whole verdict.
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
        // Fails if a zombie counts as running, which is why the child is only
        // reaped after the assertion.
        let mut child = std::process::Command::new("/bin/sh")
            .args(["-c", "exit 0"])
            .spawn()
            .unwrap();
        let pidfd = pidfd_open(child.id()).unwrap().unwrap();

        assert!(wait_for_exit(&pidfd));

        child.wait().unwrap();
    }

    #[test]
    fn a_reaped_process_is_waited_on_by_identity_not_by_number() {
        // Fails if the wait watches the pid rather than the descriptor, and so
        // stalls on whatever reuses the number.
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

        assert_eq!(reap_jailed_vmm(&discard(), &root), Reaped::Clear);
    }

    #[test]
    fn a_survivor_is_reported_without_exhausting_the_scan() {
        // Fails if a survivor triggers a rescan instead of being returned at
        // once.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let scans = std::cell::Cell::new(0);

        let reaped = reap_jailed_vmm_with(
            &discard(),
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
        // Fails if exhausting the bound reports the jail clear.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let reaps = std::cell::Cell::new(0);

        let reaped = reap_jailed_vmm_with(
            &discard(),
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
        // Fails if exhausting the bound is a verdict rather than one more scan.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let scans = std::cell::Cell::new(0);

        let reaped = reap_jailed_vmm_with(
            &discard(),
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
        // Fails if a scan error reads as an empty jail, or an absent jail root
        // as an unexaminable one.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

        let reaped = reap_jailed_vmm_with(
            &discard(),
            &root,
            |_jail_root| Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
            |_pid, _jail_root| Reaped::Clear,
        );

        assert_eq!(reaped, Reaped::Unexaminable);
        assert_eq!(
            reap_jailed_vmm(&discard(), &root.join("absent")),
            Reaped::Clear
        );
    }

    #[test]
    fn a_scan_that_fails_after_the_bound_is_not_reported_clear() {
        // Fails if a scan error after the bound reads as an empty jail.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let scans = std::cell::Cell::new(0);

        let reaped = reap_jailed_vmm_with(
            &discard(),
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
}
