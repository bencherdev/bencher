//! A wait announced while it lasts, and the `flock` calls the network
//! namespace lock makes.

use std::fs::File;
use std::os::fd::AsRawFd as _;
use std::sync::{Condvar, Mutex, PoisonError};
use std::time::Duration;

/// Long enough that a short wait prints nothing more, short enough that an
/// operator can tell a wait from a wedge within half a minute.
pub(super) const ANNOUNCE_EVERY: Duration = Duration::from_secs(30);

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

    use super::*;

    /// A bound only a broken wait reaches, so a failure is a failure and not a
    /// hang.
    const NEVER: Duration = Duration::from_secs(30);

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
        // A short wait must not repeat its line, or operators learn to skip
        // the one that matters.
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
}
