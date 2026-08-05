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
//!
//! The wait is therefore unbounded, and says so on a schedule. A holder is
//! entitled to the lock for a whole benchmark job, so any timeout would fail
//! honest waits, which leaves the operator with nothing to read: one line and
//! then silence for as long as the holder lasts is indistinguishable from a
//! hang. Repeating the line is what a bound would really have bought, without
//! the cost.

#![expect(clippy::print_stdout, reason = "prints why the runner is waiting")]

use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd as _;
use std::sync::{Condvar, Mutex, PoisonError};
use std::time::Duration;

use camino::Utf8Path;

use crate::error::JailError;

/// Lock file name inside the state directory.
///
/// It lives beside the chroot base rather than inside it, so the sweep (which
/// only removes directories under `<state_dir>/jail/firecracker`) can never
/// reach it.
///
/// Spelled here, where the lock is created, and read by the state directory
/// guard through this constant rather than a literal of its own. The guard
/// tolerates this name in a root that holds nothing else, so that the runner's
/// own lock cannot disown the runner's own directory, but it never treats the
/// name as proof of ownership: that is what once let a populated system
/// directory pass the guard, and the tree is what proves ownership now. See
/// [`crate::jail::state`].
pub(super) const LOCK_FILE: &str = ".lock";

/// How often an announced wait repeats itself.
///
/// Long enough that a job handing the lock to the next runner prints nothing
/// beyond the first line, short enough that an operator watching a runner that
/// has said nothing else knows within half a minute whether it is waiting or
/// wedged. The sweep announces a long reclamation on the same cadence, for the
/// same reason: it runs under this lock, so its silence reads the same way.
pub(super) const ANNOUNCE_EVERY: Duration = Duration::from_secs(30);

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

        // Nothing here gives up: the wait is signal proof and has no bound, so
        // the announcement is the only thing that distinguishes it from a hang.
        while_waiting(
            ANNOUNCE_EVERY,
            || println!("  Still waiting for another bencher runner to release {path}..."),
            || flock_exclusive(&file),
        )
        .map_err(|e| JailError::JailLock {
            path: path.clone(),
            source: e,
        })?;

        Ok(Self { _file: file })
    }
}

/// Run `wait`, calling `announce` every `interval` until it returns.
///
/// The repetition is a companion thread rather than a poll, because a poll
/// cannot see inside the wait: the lock blocks in the kernel, and the sweep's
/// reclamation of one jail is a single long call. Polling for the lock would
/// also hand it over up to an interval late, and the point of holding the jail
/// serially is that the next job starts as soon as the last one is done with
/// it. The thread is scoped, so it is joined before this returns and cannot
/// outlive the wait it describes.
pub(super) fn while_waiting<T, A: Fn() + Sync, W: FnOnce() -> T>(
    interval: Duration,
    announce: A,
    wait: W,
) -> T {
    /// Flips the predicate and wakes the announcer, on unwind as well as on
    /// return.
    ///
    /// The scope joins the announcer on the way out of either, and its loop
    /// runs until the predicate flips, so flipping it only after `wait`
    /// returns would turn a panic in the wait into a join that never
    /// finishes: the process hangs in place of propagating the panic.
    struct Done<'scope> {
        /// The announcer's predicate.
        done: &'scope Mutex<bool>,
        /// What the announcer sleeps on.
        woken: &'scope Condvar,
    }
    impl Drop for Done<'_> {
        fn drop(&mut self) {
            *self.done.lock().unwrap_or_else(PoisonError::into_inner) = true;
            self.woken.notify_all();
        }
    }

    #[expect(
        clippy::mutex_atomic,
        reason = "the condvar's predicate has to be read under the condvar's mutex"
    )]
    let done = Mutex::new(false);
    let woken = Condvar::new();

    std::thread::scope(|scope| {
        scope.spawn(|| {
            let mut finished = done.lock().unwrap_or_else(PoisonError::into_inner);
            while !*finished {
                let (guard, timed_out) = woken
                    .wait_timeout(finished, interval)
                    .unwrap_or_else(PoisonError::into_inner);
                finished = guard;
                // Only a full interval with the wait still running is worth a
                // line. A spurious wakeup has nothing new to say, and neither
                // does the wakeup that means the lock was just taken.
                if !*finished && timed_out.timed_out() {
                    announce();
                }
            }
        });

        let _flip = Done {
            done: &done,
            woken: &woken,
        };
        wait()
    })
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
    use std::sync::atomic::{AtomicUsize, Ordering};

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
        std::thread::sleep(Duration::from_millis(200));
        assert!(
            !waiter.is_finished(),
            "a second runner must wait rather than proceed to sweep"
        );

        drop(held);
        waiter.join().unwrap().unwrap();
    }

    #[test]
    fn a_wait_that_outlives_the_interval_is_announced_again() {
        // One line and then silence for as long as the holder's job lasts is
        // what makes an honest wait read as a wedged runner.
        let announced = AtomicUsize::new(0);

        while_waiting(
            Duration::from_millis(20),
            || {
                announced.fetch_add(1, Ordering::Relaxed);
            },
            || std::thread::sleep(Duration::from_millis(250)),
        );

        let announced = announced.load(Ordering::Relaxed);
        assert!(
            announced >= 2,
            "a wait of several intervals must keep saying so, said it {announced} times"
        );
    }

    #[test]
    fn a_wait_shorter_than_the_interval_says_nothing_more() {
        // The usual case is a lock handed straight over. A runner that narrated
        // that too would teach an operator to skip the line that matters.
        let announced = AtomicUsize::new(0);

        while_waiting(
            Duration::from_secs(30),
            || {
                announced.fetch_add(1, Ordering::Relaxed);
            },
            || {},
        );

        assert_eq!(
            announced.load(Ordering::Relaxed),
            0,
            "a wait that ended within the interval has nothing to add"
        );
    }

    #[test]
    fn a_wait_that_panics_still_propagates_the_panic() {
        // The scope joins the announcer before the panic leaves it, and the
        // announcer runs until the predicate flips. A predicate flipped only
        // after `wait` returns never flips on the unwind path, so the join
        // blocks forever and the process hangs instead of panicking. Nothing
        // waited on can panic today; the function is generic over the wait,
        // so the guarantee belongs to it rather than to today's caller.
        let unwound = std::panic::catch_unwind(|| {
            while_waiting(Duration::from_secs(30), || {}, || panic!("wait failed"));
        });

        unwound.unwrap_err();
    }

    #[test]
    fn a_missing_state_directory_is_an_error() {
        let (_dir, state) = state_in_tmpdir();

        JailLock::acquire(&state.join("absent")).unwrap_err();
    }
}
