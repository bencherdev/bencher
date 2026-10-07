//! Advisory lock serializing the jail lifecycle across runner processes, so a
//! sweep never reclaims a chroot another runner's VMM is still using.

use std::fs::{File, OpenOptions};
use std::os::fd::AsRawFd as _;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Condvar, Mutex, PoisonError};
use std::time::{Duration, Instant};

use camino::Utf8Path;
use slog::{Logger, info};

use crate::error::JailError;

/// Beside the chroot base rather than inside it, so the sweep can never remove
/// it.
pub(super) const LOCK_FILE: &str = ".lock";

/// Long enough that a lock handed straight over prints one line, short enough
/// that an operator can tell a wait from a wedge within half a minute.
pub(super) const ANNOUNCE_EVERY: Duration = Duration::from_secs(30);

/// Short enough that a released lock and a cancel are both noticed at once.
const RETRY_EVERY: Duration = Duration::from_millis(20);

/// Not reentrant: `flock` is per open file description, so a second `acquire`
/// in a process that already holds the lock blocks on itself forever.
#[derive(Debug)]
pub struct JailLock {
    _file: File,
}

impl JailLock {
    /// The state directory must already exist, since the lock guards its
    /// contents and cannot also guard its creation. A set `cancel` ends the wait
    /// without the lock.
    pub fn acquire(
        log: &Logger,
        state_dir: &Utf8Path,
        cancel: Option<&AtomicBool>,
    ) -> Result<Self, JailError> {
        Self::acquire_with(log, state_dir, cancel, |path| {
            info!(log, "Waiting for the jail lock"; "path" => path.as_str());
        })
    }

    fn acquire_with<C: FnOnce(&Utf8Path)>(
        log: &Logger,
        state_dir: &Utf8Path,
        cancel: Option<&AtomicBool>,
        contended: C,
    ) -> Result<Self, JailError> {
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

        if try_lock(&file, &path)? {
            return Ok(Self { _file: file });
        }
        contended(&path);

        // Polled rather than blocked on, so a cancel is seen whichever thread
        // the signal landed on. Unbounded, because a holder is entitled to the
        // lock for a whole job.
        let mut announced = Instant::now();
        loop {
            if cancel.is_some_and(|cancel| cancel.load(Ordering::SeqCst)) {
                return Err(JailError::JailLockCancelled { path });
            }
            std::thread::sleep(RETRY_EVERY);
            if try_lock(&file, &path)? {
                return Ok(Self { _file: file });
            }
            if announced.elapsed() >= ANNOUNCE_EVERY {
                info!(log, "Still waiting for the jail lock"; "path" => path.as_str());
                announced = Instant::now();
            }
        }
    }
}

fn try_lock(file: &File, path: &Utf8Path) -> Result<bool, JailError> {
    match flock_nonblocking(file) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => Ok(false),
        Err(e) => Err(JailError::JailLock {
            path: path.to_owned(),
            source: e,
        }),
    }
}

/// Announces from a companion thread rather than polling, because the wait
/// blocks in a single call a poll cannot see inside.
pub(super) fn while_waiting<T, A: Fn() + Sync, W: FnOnce() -> T>(
    interval: Duration,
    announce: A,
    wait: W,
) -> T {
    /// A drop guard, so the predicate also flips on unwind and a panicking
    /// `wait` propagates rather than hanging the scope's join.
    struct Done<'scope> {
        done: &'scope Mutex<bool>,
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
                // line, not a spurious wakeup or the one that ends the wait.
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

pub(super) fn flock_exclusive(file: &File) -> std::io::Result<()> {
    flock(file, libc::LOCK_EX)
}

pub(super) fn flock_nonblocking(file: &File) -> std::io::Result<()> {
    flock(file, libc::LOCK_EX | libc::LOCK_NB)
}

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
    use std::sync::mpsc;

    use camino::Utf8PathBuf;

    use super::*;
    use crate::log::discard;

    fn state_in_tmpdir() -> (tempfile::TempDir, Utf8PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        (dir, root)
    }

    #[test]
    fn the_lock_can_be_taken() {
        let (_dir, state) = state_in_tmpdir();

        let lock = JailLock::acquire(&discard(), &state, None).unwrap();

        assert!(state.join(LOCK_FILE).exists());
        drop(lock);
    }

    #[test]
    fn a_released_lock_can_be_retaken() {
        let (_dir, state) = state_in_tmpdir();
        drop(JailLock::acquire(&discard(), &state, None).unwrap());

        JailLock::acquire(&discard(), &state, None).unwrap();
    }

    /// A bound only a broken wait reaches, so a failure is a failure and not a
    /// hang.
    const NEVER: Duration = Duration::from_secs(30);

    #[test]
    fn a_held_lock_makes_a_second_runner_wait() {
        // A contended runner must announce its wait and return holding the
        // lock, or a sweep could run while another runner has a job in flight.
        let (_dir, state) = state_in_tmpdir();
        let held = JailLock::acquire(&discard(), &state, None).unwrap();
        let (contended, waiting) = mpsc::sync_channel(1);

        let waiter = {
            let state = state.clone();
            std::thread::spawn(move || {
                JailLock::acquire_with(&discard(), &state, None, move |_path| {
                    contended.send(()).unwrap();
                })
            })
        };
        let announced = waiting.recv_timeout(NEVER);
        drop(held);
        let taken = waiter.join().unwrap().unwrap();

        announced.expect("a contended lock is announced before the wait");
        let probe = OpenOptions::new()
            .write(true)
            .open(state.join(LOCK_FILE))
            .unwrap();
        assert!(
            flock_nonblocking(&probe).is_err(),
            "the waiter must hold the lock it returned with"
        );
        drop(taken);
        // A child another test forks keeps the lock until its exec closes the
        // copy, so the release is awaited rather than probed once.
        let dropped = Instant::now();
        while let Err(e) = flock_nonblocking(&probe) {
            assert_eq!(e.kind(), std::io::ErrorKind::WouldBlock, "{e}");
            assert!(dropped.elapsed() < NEVER, "a dropped lock must be released");
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    #[test]
    fn a_wait_that_outlives_the_interval_is_announced_again() {
        // A wait announced once and then silent for a whole job reads as a
        // wedged runner.
        let (announce, announced) = mpsc::sync_channel(2);

        while_waiting(
            Duration::from_millis(1),
            // Dropped once two are queued: a blocked announcement would hold
            // the lock the wait needs to end.
            || announce.try_send(()).unwrap_or_default(),
            || {
                for _ in 0..2 {
                    announced
                        .recv_timeout(NEVER)
                        .expect("a wait that outlives the interval is announced again");
                }
            },
        );
    }

    #[test]
    fn a_wait_shorter_than_the_interval_says_nothing_more() {
        // A lock handed straight over must not repeat its line, or operators
        // learn to skip the one that matters.
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
        // A predicate flipped only after `wait` returns never flips on unwind,
        // so a panicking wait would hang the scope's join instead of panicking.
        let unwound = std::panic::catch_unwind(|| {
            while_waiting(Duration::from_secs(30), || {}, || panic!("wait failed"));
        });

        unwound.unwrap_err();
    }

    #[test]
    fn a_missing_state_directory_is_an_error() {
        let (_dir, state) = state_in_tmpdir();

        JailLock::acquire(&discard(), &state.join("absent"), None).unwrap_err();
    }

    #[test]
    fn a_cancelled_wait_gives_up_without_the_lock() {
        // A signal to a runner queued behind a long job must end the wait, not
        // sit it out.
        let (_dir, state) = state_in_tmpdir();
        let held = JailLock::acquire(&discard(), &state, None).unwrap();
        let cancel = AtomicBool::new(false);
        let (finished, outcome) = mpsc::sync_channel(1);

        std::thread::scope(|scope| {
            scope.spawn(|| {
                let waited = JailLock::acquire_with(&discard(), &state, Some(&cancel), |_path| {
                    cancel.store(true, Ordering::SeqCst);
                });
                finished.send(waited).unwrap();
            });
            let waited = outcome.recv_timeout(NEVER);
            drop(held);
            let waited = waited.expect("a cancelled wait ends while the lock is still held");
            assert!(
                matches!(waited, Err(JailError::JailLockCancelled { .. })),
                "{waited:?}"
            );
        });
    }
}
