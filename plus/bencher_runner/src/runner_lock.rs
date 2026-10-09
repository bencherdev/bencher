//! One runner per host, from startup until the process dies.

use std::fs::{DirBuilder, File, OpenOptions, TryLockError};
use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _};

use camino::Utf8Path;
use slog::{Logger, warn};

use crate::error::LockError;

/// On tmpfs, so no lock file outlives a reboot, and outside every state
/// directory, so runners with different ones still collide.
const RUNNER_LOCK_PATH: &str = "/run/bencher/runner.lock";

/// The kernel drops the lock when the process dies, SIGKILL included, and the
/// descriptor is close-on-exec, so no child and no self-update's `exec` keeps
/// it.
#[must_use]
pub(crate) struct RunnerLock {
    file: Option<File>,
}

impl RunnerLock {
    /// A held lock fails closed: a host runs one runner, whatever its state
    /// directory.
    pub(crate) fn acquire(log: &Logger) -> Result<Self, LockError> {
        Self::acquire_at(
            log,
            Utf8Path::new(RUNNER_LOCK_PATH),
            crate::jail::current_euid(),
        )
    }

    /// The uid is a parameter so tests reach the root arm without root.
    fn acquire_at(log: &Logger, path: &Utf8Path, euid: u32) -> Result<Self, LockError> {
        let file = match open(path) {
            Ok(file) => file,
            // Without root a runner can neither sandbox, tune, nor create a
            // cgroup, and the lock file is root's alone.
            Err(e) if euid != 0 => {
                warn!(log, "Runner lock not taken, so another runner may share this host";
                    "path" => path.as_str(),
                    "error" => %e,
                );
                return Ok(Self { file: None });
            },
            Err(source) => {
                return Err(LockError::Open {
                    path: path.to_owned(),
                    source,
                });
            },
        };
        match file.try_lock() {
            Ok(()) => Ok(Self { file: Some(file) }),
            Err(TryLockError::WouldBlock) => Err(LockError::Held {
                path: path.to_owned(),
            }),
            Err(TryLockError::Error(source)) => Err(LockError::Lock {
                path: path.to_owned(),
                source,
            }),
        }
    }

    /// Only a runner holding the lock may take what it finds under the cgroup
    /// base for an orphan.
    pub(crate) fn is_held(&self) -> bool {
        self.file.is_some()
    }
}

/// Root's alone, directory and file, since any process that can open a lock
/// file can hold the lock.
pub(crate) fn open(path: &Utf8Path) -> std::io::Result<File> {
    if let Some(dir) = path.parent() {
        DirBuilder::new().recursive(true).mode(0o700).create(dir)?;
    }
    OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path)
}

#[cfg(test)]
mod tests {
    use std::process::{Command, Stdio};
    use std::time::{Duration, Instant};

    use camino::Utf8PathBuf;

    use super::*;
    use crate::error::JailError;
    use crate::log::discard;

    const ROOT_EUID: u32 = 0;

    /// A bound only a broken lock reaches, so a failure is a failure and not a
    /// hang.
    const NEVER: Duration = Duration::from_secs(30);

    fn lock_path() -> (tempfile::TempDir, Utf8PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = Utf8PathBuf::try_from(dir.path().join("bencher").join("runner.lock")).unwrap();
        (dir, path)
    }

    /// Killed and reaped on drop, so a failed assertion leaves no process
    /// behind.
    struct Reaped(std::process::Child);

    impl Drop for Reaped {
        fn drop(&mut self) {
            drop(self.0.kill());
            drop(self.0.wait());
        }
    }

    /// A child another test forks keeps a copy of a probe's lock until its
    /// exec closes it, so a lock expected free is awaited rather than probed
    /// once.
    fn acquire_eventually(path: &Utf8Path) -> RunnerLock {
        let started = Instant::now();
        loop {
            match RunnerLock::acquire_at(&discard(), path, ROOT_EUID) {
                Ok(lock) => return lock,
                Err(LockError::Held { .. }) if started.elapsed() < NEVER => {
                    std::thread::sleep(Duration::from_millis(10));
                },
                Err(e) => panic!("the lock never came free: {e}"),
            }
        }
    }

    #[test]
    fn a_held_lock_refuses_a_second_runner_by_its_path() {
        // Kills a lock that lets a second runner through.
        let (_dir, path) = lock_path();
        let _first = RunnerLock::acquire_at(&discard(), &path, ROOT_EUID).unwrap();

        let Err(err) = RunnerLock::acquire_at(&discard(), &path, ROOT_EUID) else {
            panic!("a second runner must not take a held lock");
        };

        assert!(matches!(err, LockError::Held { .. }), "{err}");
        assert!(
            err.to_string().contains(path.as_str()),
            "the error must name the lock: {err}"
        );
    }

    #[test]
    fn a_lock_whose_holder_was_killed_is_taken() {
        // Kills a lock that a file's existence holds, which a SIGKILLed
        // holder leaves behind.
        let (_dir, path) = lock_path();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        // The shell's lock is on its own descriptor, which it keeps across the
        // exec, so the one process the kill ends is the only holder.
        let mut holder = Reaped(
            Command::new("/bin/sh")
                .arg("-c")
                .arg(r#"exec 9>"$0" && flock 9 && exec sleep 600"#)
                .arg(path.as_str())
                .stdin(Stdio::null())
                .spawn()
                .unwrap(),
        );
        let started = Instant::now();
        loop {
            match RunnerLock::acquire_at(&discard(), &path, ROOT_EUID) {
                Err(LockError::Held { .. }) => break,
                Ok(probe) => drop(probe),
                Err(e) => panic!("the probe failed: {e}"),
            }
            assert!(
                holder.0.try_wait().unwrap().is_none(),
                "the holder exited before it took the lock"
            );
            assert!(started.elapsed() < NEVER, "the holder never took the lock");
            std::thread::sleep(Duration::from_millis(10));
        }

        holder.0.kill().unwrap();
        holder.0.wait().unwrap();

        drop(acquire_eventually(&path));
    }

    #[test]
    fn a_child_the_runner_spawns_never_holds_its_lock() {
        // Kills a descriptor that is not close-on-exec: an orphaned VMM would
        // keep the lock, and no later runner could start to reap it.
        let (_dir, path) = lock_path();
        let lock = RunnerLock::acquire_at(&discard(), &path, ROOT_EUID).unwrap();
        let child = Reaped(
            Command::new("sleep")
                .arg("600")
                .stdin(Stdio::null())
                .spawn()
                .unwrap(),
        );

        drop(lock);
        drop(acquire_eventually(&path));
        drop(child);
    }

    #[test]
    fn a_root_runner_that_cannot_open_the_lock_fails_closed() {
        // Kills a root runner that runs on without the lock it could not open.
        let (dir, _path) = lock_path();
        let not_a_dir = Utf8PathBuf::try_from(dir.path().join("file")).unwrap();
        std::fs::write(&not_a_dir, b"").unwrap();
        let path = not_a_dir.join("runner.lock");

        let Err(err) = RunnerLock::acquire_at(&discard(), &path, ROOT_EUID) else {
            panic!("a root runner without the lock must not run");
        };

        assert!(matches!(err, LockError::Open { .. }), "{err}");
        assert!(err.to_string().contains(path.as_str()), "{err}");
    }

    #[test]
    fn a_runner_that_is_not_root_runs_without_a_lock_it_cannot_open() {
        // Kills a lock that stops an unprivileged runner, which serves only
        // Jobs with no sandbox and may share a host with a root one.
        let (dir, _path) = lock_path();
        let not_a_dir = Utf8PathBuf::try_from(dir.path().join("file")).unwrap();
        std::fs::write(&not_a_dir, b"").unwrap();

        drop(RunnerLock::acquire_at(&discard(), &not_a_dir.join("runner.lock"), 1000).unwrap());
    }

    #[test]
    fn a_runner_without_the_lock_neither_checks_nor_sweeps() {
        // Kills a startup that takes the cgroups under the base for orphans
        // without the lock, when they may hold a root runner's Jobs.
        let (dir, _path) = lock_path();
        let not_a_dir = Utf8PathBuf::try_from(dir.path().join("file")).unwrap();
        std::fs::write(&not_a_dir, b"").unwrap();
        let unheld =
            RunnerLock::acquire_at(&discard(), &not_a_dir.join("runner.lock"), 1000).unwrap();

        crate::jail::prepare_at_startup_with(
            &discard(),
            &unheld,
            true,
            || panic!("checked the kernel"),
            |_log| panic!("swept the cgroups"),
        )
        .unwrap();
    }

    #[test]
    fn only_a_sandboxed_runner_needs_the_kill_to_start() {
        // Kills a check that stops a runner serving only Jobs with no sandbox,
        // and a sandboxed one that starts without it.
        let (_dir, path) = lock_path();
        let held = RunnerLock::acquire_at(&discard(), &path, ROOT_EUID).unwrap();
        let refused = || {
            Err(JailError::NoCgroupKill {
                path: Utf8PathBuf::from("/sys/fs/cgroup/bencher/cgroup.kill"),
            })
        };
        let swept = std::cell::Cell::new(0);
        let sweep = |_log: &Logger| {
            swept.set(swept.get() + 1);
            Ok(0)
        };

        let err = crate::jail::prepare_at_startup_with(&discard(), &held, true, refused, sweep)
            .unwrap_err();
        assert!(matches!(err, JailError::NoCgroupKill { .. }), "{err}");
        assert_eq!(swept.get(), 0, "a refused runner sweeps nothing");

        crate::jail::prepare_at_startup_with(&discard(), &held, false, refused, sweep).unwrap();
        assert_eq!(swept.get(), 1, "a runner with no sandbox still sweeps");
    }

    #[test]
    fn a_sweep_that_fails_at_startup_still_lets_the_runner_start() {
        // Kills a startup that a stuck orphan stops, although the next
        // sandboxed Job's sweep tries again.
        let (_dir, path) = lock_path();
        let held = RunnerLock::acquire_at(&discard(), &path, ROOT_EUID).unwrap();

        crate::jail::prepare_at_startup_with(
            &discard(),
            &held,
            true,
            || Ok(()),
            |_log| {
                Err(JailError::CgroupNotEmptied {
                    path: Utf8PathBuf::from("/sys/fs/cgroup/bencher/stuck"),
                    timeout_secs: 5,
                })
            },
        )
        .unwrap();
    }

    #[test]
    fn the_lock_file_opens_for_its_owner_alone() {
        // Kills a lock file other users can open, which lets any of them hold
        // it and keep the runner from starting.
        use std::os::unix::fs::PermissionsExt as _;

        let (_dir, path) = lock_path();
        let _lock = RunnerLock::acquire_at(&discard(), &path, ROOT_EUID).unwrap();

        let mode = std::fs::metadata(&path).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "{path}");
    }
}
