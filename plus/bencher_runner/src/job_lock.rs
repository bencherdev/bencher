//! The per-Job lock and the maintenance markers, through which host
//! maintenance and Jobs take turns.
//!
//! Maintenance touches a marker named `maintenance` or `maintenance.<name>`,
//! then waits for `job.lock` (`flock -F`), and removes its marker when done.
//! The runner holds the lock from before `Ready` through any Job, and pauses
//! while a marker is present or the lock is held elsewhere.

use std::fs::{File, TryLockError};
use std::time::Duration;

use bencher_json::runner::PauseReason;
use camino::Utf8Path;
use slog::{Logger, info, warn};

use crate::error::LockError;
use crate::maintenance::{JOB_LOCK, RUN_DIR, marker_present};

/// How often `runner run` tries the lock while maintenance holds it.
const WAIT_POLL: Duration = Duration::from_millis(250);

/// Dropping it releases the lock; the descriptor is close-on-exec, so no
/// child keeps it.
#[must_use]
pub(crate) struct JobLock {
    #[expect(dead_code, reason = "held for its drop, which releases the lock")]
    file: Option<File>,
}

impl JobLock {
    /// A runner that is not root can open neither the lock nor the markers'
    /// directory, so it takes its turn without them.
    pub(crate) fn take_at(log: &Logger, dir: &Utf8Path, euid: u32) -> Result<Self, PauseReason> {
        let marker = marker_present(dir.as_std_path());
        let path = dir.join(JOB_LOCK);
        let file = match crate::runner_lock::open(&path) {
            Ok(file) => file,
            Err(e) => {
                if euid != 0 {
                    return if marker {
                        Err(maintenance(true, false))
                    } else {
                        Ok(Self { file: None })
                    };
                }
                warn!(log, "Job lock not opened, so the runner pauses"; "path" => path.as_str(), "error" => %e);
                return Err(maintenance(marker, true));
            },
        };
        match file.try_lock() {
            Ok(()) => {
                if marker {
                    Err(maintenance(true, false))
                } else {
                    Ok(Self { file: Some(file) })
                }
            },
            Err(TryLockError::WouldBlock) => Err(maintenance(marker, true)),
            Err(TryLockError::Error(e)) => {
                warn!(log, "Job lock not taken, so the runner pauses"; "path" => path.as_str(), "error" => %e);
                Err(maintenance(marker, true))
            },
        }
    }

    /// `runner run` waits out maintenance that holds the lock, until
    /// `stopped`, which gives `None`.
    pub(crate) fn wait(
        log: &Logger,
        stopped: impl Fn() -> bool,
    ) -> Result<Option<Self>, LockError> {
        Self::wait_at(
            log,
            Utf8Path::new(RUN_DIR),
            crate::jail::current_euid(),
            stopped,
        )
    }

    fn wait_at(
        log: &Logger,
        dir: &Utf8Path,
        euid: u32,
        stopped: impl Fn() -> bool,
    ) -> Result<Option<Self>, LockError> {
        let path = dir.join(JOB_LOCK);
        let file = match crate::runner_lock::open(&path) {
            Ok(file) => file,
            Err(source) => {
                if euid == 0 {
                    return Err(LockError::OpenJob { path, source });
                }
                warn!(log, "Job lock not taken, so host maintenance may run beside this run";
                    "path" => path.as_str(),
                    "error" => %source,
                );
                return Ok(Some(Self { file: None }));
            },
        };
        let mut waiting = false;
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(Some(Self { file: Some(file) })),
                Err(TryLockError::WouldBlock) => {
                    if !waiting {
                        info!(log, "Waiting for host maintenance to finish"; "path" => path.as_str());
                        waiting = true;
                    }
                },
                Err(TryLockError::Error(source)) => {
                    return Err(LockError::LockJob { path, source });
                },
            }
            if stopped() {
                return Ok(None);
            }
            std::thread::sleep(WAIT_POLL);
        }
    }
}

fn maintenance(marker: bool, lock: bool) -> PauseReason {
    PauseReason::Maintenance { marker, lock }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::PermissionsExt as _;
    use std::process::{Command, Stdio};
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Instant;

    use camino::Utf8PathBuf;

    use super::*;
    use crate::log::discard;
    use crate::maintenance::MARKER;

    const ROOT_EUID: u32 = 0;

    /// A bound only a broken lock reaches, so a failure is a failure and not a
    /// hang.
    const NEVER: Duration = Duration::from_secs(30);

    fn run_dir() -> (tempfile::TempDir, Utf8PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = Utf8PathBuf::try_from(dir.path().join("bencher")).unwrap();
        (dir, path)
    }

    /// Another process's hold on the lock: a second open file description,
    /// which `flock` treats as another holder even in this process. A child
    /// another test forks keeps a copy of a released lock until its exec, so
    /// the hold is awaited.
    fn hold(dir: &Utf8Path) -> File {
        std::fs::create_dir_all(dir).unwrap();
        let file = File::create(dir.join(JOB_LOCK)).unwrap();
        let started = Instant::now();
        while file.try_lock().is_err() {
            assert!(started.elapsed() < NEVER, "the lock was never free");
            std::thread::sleep(Duration::from_millis(10));
        }
        file
    }

    /// A child another test forks keeps a copy of a lock until its exec
    /// closes it, so a turn expected free is awaited rather than taken once.
    fn take_eventually(dir: &Utf8Path) -> JobLock {
        let started = Instant::now();
        loop {
            match JobLock::take_at(&discard(), dir, ROOT_EUID) {
                Ok(lock) => return lock,
                Err(reason) => {
                    assert!(started.elapsed() < NEVER, "never free: {reason:?}");
                    std::thread::sleep(Duration::from_millis(10));
                },
            }
        }
    }

    /// Whether another process could take the lock now.
    fn free(dir: &Utf8Path) -> bool {
        File::open(dir.join(JOB_LOCK)).unwrap().try_lock().is_ok()
    }

    #[test]
    fn a_free_lock_is_the_runners_turn() {
        // Kills a turn that never takes the lock, which lets maintenance run
        // during the Job.
        let (_dir, dir) = run_dir();
        let _turn = take_eventually(&dir);
        assert!(!free(&dir));
    }

    #[test]
    fn a_lock_held_elsewhere_pauses_for_maintenance() {
        // Kills a turn that takes a Job while maintenance runs.
        let (_dir, dir) = run_dir();
        let _maintenance = hold(&dir);
        let Err(reason) = JobLock::take_at(&discard(), &dir, ROOT_EUID) else {
            panic!("the runner took a held lock");
        };
        assert_eq!(reason, maintenance(false, true));
    }

    #[test]
    fn a_marker_pauses_without_taking_the_lock() {
        // Kills a runner that takes the lock while maintenance waits for it,
        // which starves maintenance, and one that ignores a drain marker.
        for name in [MARKER, "maintenance.fstrim.service"] {
            let (_dir, dir) = run_dir();
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(name), b"").unwrap();

            let Err(reason) = JobLock::take_at(&discard(), &dir, ROOT_EUID) else {
                panic!("the runner took its turn past the marker {name}");
            };
            assert_eq!(reason, maintenance(true, false), "{name}");
            // Maintenance gets the lock at once.
            hold(&dir);
        }
    }

    #[test]
    fn only_the_maintenance_names_are_markers() {
        // Kills a marker test that any file in the directory trips, such as
        // the locks beside the markers.
        let (_dir, dir) = run_dir();
        std::fs::create_dir_all(&dir).unwrap();
        for name in ["runner.lock", JOB_LOCK, "maintenances", "maintenance-x"] {
            std::fs::write(dir.join(name), b"").unwrap();
        }
        assert!(!marker_present(dir.as_std_path()));
        let _turn = take_eventually(&dir);
        assert!(!free(&dir));
    }

    #[test]
    fn dropping_the_turn_releases_the_lock() {
        // Kills a lock that outlives its turn, which holds maintenance off
        // until the runner exits.
        let (_dir, dir) = run_dir();
        let turn = take_eventually(&dir);
        drop(turn);
        hold(&dir);
    }

    #[test]
    fn a_child_never_holds_the_lock() {
        // Kills a descriptor that is not close-on-exec: the jailed VMM, or any
        // child, would keep maintenance waiting after the Job.
        let (_dir, dir) = run_dir();
        let turn = take_eventually(&dir);
        let mut child = Command::new("sleep")
            .arg("600")
            .stdin(Stdio::null())
            .spawn()
            .unwrap();
        drop(turn);
        let taken = take_eventually(&dir);
        drop(child.kill());
        drop(child.wait());
        drop(taken);
    }

    #[test]
    fn the_lock_and_its_directory_are_root_only() {
        // Kills a lock file or directory other users can open, which lets any
        // of them hold the lock and keep the runner paused.
        let (_dir, dir) = run_dir();
        let _turn = take_eventually(&dir);
        let mode = |path: &Utf8Path| std::fs::metadata(path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode(&dir.join(JOB_LOCK)), 0o600);
        assert_eq!(mode(&dir), 0o700);
    }

    #[test]
    fn a_runner_that_is_not_root_takes_its_turn_without_the_lock() {
        // Kills a lock that stops an unprivileged runner, which can open
        // neither the lock nor the markers' directory.
        let (dir, _run_dir) = run_dir();
        let not_a_dir = Utf8PathBuf::try_from(dir.path().join("file")).unwrap();
        std::fs::write(&not_a_dir, b"").unwrap();

        drop(JobLock::take_at(&discard(), &not_a_dir, 1000).unwrap());
        let Err(reason) = JobLock::take_at(&discard(), &not_a_dir, ROOT_EUID) else {
            panic!("a root runner without the lock must pause");
        };
        assert_eq!(reason, maintenance(false, true));
    }

    #[test]
    fn runner_run_waits_for_maintenance_then_takes_the_lock() {
        // Kills a `runner run` that runs beside maintenance, and one that
        // never takes the lock once maintenance releases it.
        let (_dir, dir) = run_dir();
        let maintenance = hold(&dir);
        let taken = AtomicBool::new(false);
        let started = Instant::now();
        std::thread::scope(|scope| {
            let waiter = scope.spawn(|| {
                let lock =
                    JobLock::wait_at(&discard(), &dir, ROOT_EUID, || started.elapsed() > NEVER);
                taken.store(true, Ordering::SeqCst);
                lock
            });
            std::thread::sleep(WAIT_POLL * 4);
            assert!(!taken.load(Ordering::SeqCst), "ran beside maintenance");
            drop(maintenance);
            let _lock = waiter.join().unwrap().unwrap().unwrap();
            assert!(!free(&dir));
        });
    }

    #[test]
    fn a_stop_ends_the_wait_for_maintenance() {
        // Kills a wait a stop cannot end, which holds a canceled run until
        // maintenance finishes.
        let (_dir, dir) = run_dir();
        let _maintenance = hold(&dir);
        let asked = std::cell::Cell::new(false);
        let stopped = || {
            assert!(!asked.replace(true), "the wait went on past a stop");
            true
        };
        assert!(
            JobLock::wait_at(&discard(), &dir, ROOT_EUID, stopped)
                .unwrap()
                .is_none()
        );
    }
}
