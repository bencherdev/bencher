//! Reaping the VMM a runner left behind.
//!
//! The sweep reclaims chroots because `Drop` does not run when a runner is
//! `SIGKILL`ed, crashes, or `exec`s itself during a self-update. The same exits
//! strand the jailed VMM: it is not signalled when its parent dies, so it is
//! reparented and keeps running, holding the benchmark cores through its
//! cgroup. Reclaiming only the disk leaves the more damaging half behind.
//!
//! The consequence is not leakage but wrong numbers that look right. An
//! orphaned cgroup still owns the exclusive benchmark CPUs, so the next job's
//! cpuset write is rejected and, before this, the job reported success anyway.
//!
//! Killing a process the runner does not own is a destructive capability, so
//! the target is identified as narrowly as possible: only a process whose root
//! directory *is* the chroot being swept, compared by device and inode. A
//! process merely owned by the jail uid is not a target, because on a shared
//! host that uid may legitimately own something else.

#![expect(clippy::print_stderr, reason = "reaping prints diagnostics")]

use std::fs;
use std::os::fd::{AsRawFd as _, FromRawFd as _, OwnedFd};
use std::os::unix::fs::MetadataExt as _;
use std::time::{Duration, Instant};

use camino::Utf8Path;

/// How long to wait for a killed VMM to disappear.
const REAP_TIMEOUT: Duration = Duration::from_secs(5);

/// How often to check whether it has.
const REAP_INTERVAL: Duration = Duration::from_millis(20);

/// Kill the VMM confined to `jail_root`, if one is still running.
///
/// Returns the pid that was reaped. Best effort: a VMM that cannot be
/// identified or killed is reported and left alone, because the alternative
/// to leaving an unidentified process alone is killing the wrong one.
pub fn reap_jailed_vmm(jail_root: &Utf8Path) -> Option<u32> {
    let pid = find_jailed_vmm(jail_root)?;

    // Pin the pid before signalling it. A pid found by scanning `/proc` can
    // exit and have its number recycled before the signal lands, and this runs
    // as root, so the signal would go to whatever inherited the number. A
    // pidfd refers to one process for as long as it is open and keeps the
    // number from being reused, which turns the check below into a guarantee
    // rather than a narrow window.
    let pidfd = pidfd_open(pid)?;

    // Re-check now that the pid cannot change underneath us.
    if !is_jailed_vmm(pid, jail_root) {
        return None;
    }

    if let Err(e) = pidfd_kill(&pidfd) {
        eprintln!("Warning: failed to kill orphaned VMM (pid {pid}) in {jail_root}: {e}");
        return None;
    }

    if wait_for_exit(pid) {
        eprintln!("Warning: reaped orphaned VMM (pid {pid}) left behind in {jail_root}");
        Some(pid)
    } else {
        eprintln!(
            "Warning: orphaned VMM (pid {pid}) in {jail_root} did not exit within {} seconds",
            REAP_TIMEOUT.as_secs()
        );
        None
    }
}

/// Find the pid of the VMM whose root directory is `jail_root`.
///
/// The jailer `chroot`s before exec, so the confined process's root *is* the
/// chroot. Comparing device and inode rather than the path is what makes this
/// exact: the jailer pivots into a private mount namespace, so the path reads
/// back as `/`, while the identity is preserved.
fn find_jailed_vmm(jail_root: &Utf8Path) -> Option<u32> {
    let jail = fs::metadata(jail_root).ok()?;
    for entry in fs::read_dir("/proc").ok()?.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        if matches_jail(pid, &jail) {
            return Some(pid);
        }
    }
    None
}

/// Whether a process's root directory is `jail_root`.
fn is_jailed_vmm(pid: u32, jail_root: &Utf8Path) -> bool {
    fs::metadata(jail_root).is_ok_and(|jail| matches_jail(pid, &jail))
}

/// Whether a process's root directory is the same inode as `jail`.
fn matches_jail(pid: u32, jail: &fs::Metadata) -> bool {
    // Following this magic symlink crosses into the process's own mount
    // namespace, which a privileged reader is allowed to do.
    fs::metadata(format!("/proc/{pid}/root"))
        .is_ok_and(|root| root.dev() == jail.dev() && root.ino() == jail.ino())
}

/// Open a descriptor pinned to a process.
///
/// `None` when the process is already gone, which is the common case and not
/// an error: something else reaped it first.
fn pidfd_open(pid: u32) -> Option<OwnedFd> {
    #[expect(
        unsafe_code,
        reason = "pidfd_open has no std wrapper; it takes plain integers"
    )]
    // SAFETY: `pidfd_open` takes a pid and a flag word and touches no memory.
    // It returns a new descriptor or -1, and the descriptor is handed straight
    // to `OwnedFd` so it is closed exactly once.
    let raw = unsafe { libc::syscall(libc::SYS_pidfd_open, libc::pid_t::try_from(pid).ok()?, 0) };
    let raw = libc::c_int::try_from(raw).ok()?;
    if raw < 0 {
        return None;
    }
    #[expect(
        unsafe_code,
        reason = "taking ownership of a descriptor this call just created"
    )]
    // SAFETY: `raw` is a fresh descriptor returned by the syscall above and is
    // not owned by anything else.
    let fd = unsafe { OwnedFd::from_raw_fd(raw) };
    Some(fd)
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
    // to synthesize one, and the final argument is a reserved flag word.
    let ret = unsafe {
        libc::syscall(
            libc::SYS_pidfd_send_signal,
            pidfd.as_raw_fd(),
            libc::SIGKILL,
            std::ptr::null::<libc::siginfo_t>(),
            0,
        )
    };
    if ret == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Wait for a killed process to disappear.
fn wait_for_exit(pid: u32) -> bool {
    let deadline = Instant::now() + REAP_TIMEOUT;
    while Instant::now() < deadline {
        if !Utf8Path::new(&format!("/proc/{pid}")).exists() {
            return true;
        }
        std::thread::sleep(REAP_INTERVAL);
    }
    false
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

        assert_eq!(find_jailed_vmm(&root), None);
    }

    #[test]
    fn a_missing_jail_matches_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

        assert_eq!(find_jailed_vmm(&root.join("absent")), None);
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
    fn a_pidfd_pins_a_live_process_and_refuses_a_dead_one() {
        pidfd_open(std::process::id()).expect("this process is alive");

        // Pid 0 is never a process: the syscall rejects it rather than
        // signalling the caller's process group, which is what the plain
        // `kill` interface would have done.
        assert!(pidfd_open(0).is_none());
    }

    #[test]
    fn reaping_an_unjailed_directory_kills_nothing() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

        assert_eq!(reap_jailed_vmm(&root), None);
    }
}
