//! Advisory lock serializing the jail lifecycle across runner processes.
//!
//! The sweep reclaims every chroot it finds, on the reasoning that jobs are
//! serial and so anything left is stale. That reasoning is only sound while it
//! is enforced: a one-shot `runner run` started on a host where the `runner up`
//! daemon has a job in flight would otherwise `remove_dir_all` the live chroot
//! out from under a running VMM. The same lock closes the race on the network
//! namespace handle, where two processes clearing and rebinding it at once can
//! stack mounts.
//!
//! Unlike the host tuning lock, which degrades to skipping tuning when it is
//! contended, this one waits. A runner that proceeded without it would destroy
//! another runner's work, so declining to hold it is not an option.

#![expect(clippy::print_stdout, reason = "prints why the runner is waiting")]

use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd as _;

use camino::Utf8Path;

use crate::error::JailError;

/// Lock file name inside the state directory.
///
/// It lives beside the chroot base rather than inside it, so the sweep (which
/// only removes directories under `<state_dir>/jail/firecracker`) can never
/// reach it.
///
/// Private again, and only spelled here. The state directory used to count this
/// name among the marks of a directory it owns, until proving ownership by a
/// name turned out to be the way a populated system directory could pass that
/// guard. Ownership is proven by the chroot tree now, so nothing outside this
/// module needs the name.
const LOCK_FILE: &str = ".lock";

/// Holds the jail lock for as long as it is alive.
///
/// The kernel releases a `flock` when the holder exits or dies, so a crashed
/// runner cannot wedge future runs.
///
/// `flock` is per open file description, not per process, so a second
/// `acquire` on the same path from a process that already holds it opens a new
/// description and blocks on itself forever. Nothing nests today: host
/// preparation takes and releases this lock before a job takes it, and the
/// network namespace uses a different lock file. Any new caller has to keep it
/// that way.
#[derive(Debug)]
pub struct JailLock {
    /// The locked file, held only for its `flock`.
    _file: File,
}

impl JailLock {
    /// Take the jail lock, waiting for whichever runner holds it.
    ///
    /// The state directory must already exist: the lock guards the contents,
    /// so creating the directory is not something it can protect.
    pub fn acquire(state_dir: &Utf8Path) -> Result<Self, JailError> {
        let path = state_dir.join(LOCK_FILE);
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(&path)
            .map_err(|e| JailError::OpenJailLock {
                path: path.clone(),
                source: e,
            })?;

        // Try once without blocking, so waiting can be announced rather than
        // looking like a hang.
        if flock_nonblocking(&file).is_ok() {
            return Ok(Self { _file: file });
        }
        println!("  Waiting for another bencher runner to release {path}...");

        flock_exclusive(&file).map_err(|e| JailError::JailLock {
            path: path.clone(),
            source: e,
        })?;

        Ok(Self { _file: file })
    }
}

/// Take an exclusive `flock`, waiting for whichever holder has it.
pub(super) fn flock_exclusive(file: &File) -> std::io::Result<()> {
    flock(file, libc::LOCK_EX)
}

/// Try for an exclusive `flock` without waiting.
///
/// Shared with the network namespace lock so both locks announce a wait the same
/// way: an unexplained pause is the worst thing either of them can do.
pub(super) fn flock_nonblocking(file: &File) -> std::io::Result<()> {
    flock(file, libc::LOCK_EX | libc::LOCK_NB)
}

/// Apply `flock` to a file, retrying if a signal interrupts the wait.
fn flock(file: &File, operation: libc::c_int) -> std::io::Result<()> {
    loop {
        #[expect(
            unsafe_code,
            reason = "flock has no std wrapper; the fd is owned and valid"
        )]
        // SAFETY: `file` is an open, owned descriptor for the duration of the
        // call; flock does not touch memory.
        let ret = unsafe { libc::flock(file.as_raw_fd(), operation) };
        if ret == 0 {
            return Ok(());
        }
        let err = std::io::Error::last_os_error();
        if err.kind() != std::io::ErrorKind::Interrupted {
            return Err(err);
        }
    }
}

#[cfg(test)]
mod tests {
    use camino::Utf8PathBuf;

    use super::*;

    fn state_in_tmpdir() -> (tempfile::TempDir, Utf8PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        (dir, root)
    }

    #[test]
    fn the_lock_can_be_taken() {
        let (_dir, state) = state_in_tmpdir();

        let lock = JailLock::acquire(&state).unwrap();

        assert!(state.join(LOCK_FILE).exists());
        drop(lock);
    }

    #[test]
    fn a_released_lock_can_be_retaken() {
        let (_dir, state) = state_in_tmpdir();
        drop(JailLock::acquire(&state).unwrap());

        JailLock::acquire(&state).unwrap();
    }

    #[test]
    fn a_held_lock_makes_a_second_runner_wait() {
        // `flock` is per open file description, so a second `acquire` in this
        // process contends exactly as another process would. The waiter is
        // released only once the holder drops, which is what keeps a sweep
        // from running while another runner has a job in flight.
        let (_dir, state) = state_in_tmpdir();
        let held = JailLock::acquire(&state).unwrap();

        let waiter = {
            let state = state.clone();
            std::thread::spawn(move || JailLock::acquire(&state))
        };

        // The waiter must still be blocked while the lock is held.
        std::thread::sleep(std::time::Duration::from_millis(200));
        assert!(
            !waiter.is_finished(),
            "a second runner must wait rather than proceed to sweep"
        );

        drop(held);
        waiter.join().unwrap().unwrap();
    }

    #[test]
    fn a_missing_state_directory_is_an_error() {
        let (_dir, state) = state_in_tmpdir();

        JailLock::acquire(&state.join("absent")).unwrap_err();
    }
}
