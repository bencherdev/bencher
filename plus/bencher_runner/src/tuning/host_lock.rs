//! Advisory lock serializing host-global tuning across runner processes.
//!
//! Host tuning mutates host-global state (sysctls, IRQ affinities, THP,
//! the cpuset partition). Two concurrent runner processes would fight:
//! the second one's shutdown restores settings out from under the first.
//! An advisory `flock` turns the single-runner assumption into an
//! enforced invariant: a runner that cannot take the lock skips host
//! tuning with a warning and keeps only its per-run isolation. The
//! kernel releases the lock when the holder exits or dies, so a crashed
//! runner cannot wedge future runs.

/// Lock file path. `/run` is root-writable tmpfs: it cannot be symlink
/// attacked like `/tmp` and clears on reboot. Unprivileged runners fail
/// to open it and proceed unserialized, which is harmless because their
/// host tuning writes fail anyway.
#[cfg(target_os = "linux")]
const LOCK_PATH: &str = "/run/bencher_runner_tuning.lock";

/// Holds the host tuning lock (or records why it could not be taken).
///
/// Must be declared before the `TuningGuard` at the call site so the
/// lock is released only after the guard has restored all settings.
/// On non-Linux platforms this is a no-op that always allows tuning,
/// mirroring tuning itself being a no-op there.
pub struct HostTuningLock {
    /// The locked file, held only for its `flock`; `None` when the lock
    /// is contended or the file could not be opened.
    #[cfg(target_os = "linux")]
    _file: Option<std::fs::File>,
    /// Whether host tuning may proceed.
    allows_tuning: bool,
}

impl HostTuningLock {
    /// Try to take the host tuning lock.
    #[must_use]
    pub fn acquire() -> Self {
        #[cfg(target_os = "linux")]
        {
            Self::acquire_at(camino::Utf8Path::new(LOCK_PATH))
        }
        #[cfg(not(target_os = "linux"))]
        {
            Self {
                allows_tuning: true,
            }
        }
    }

    /// Try to take the lock at an explicit path (tests pass a tempdir).
    ///
    /// An unopenable lock file allows tuning: serialization degrades to
    /// the pre-lock behavior instead of disabling tuning outright.
    #[cfg(target_os = "linux")]
    fn acquire_at(path: &camino::Utf8Path) -> Self {
        use std::os::fd::AsRawFd as _;

        let file = match std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(path)
        {
            Ok(file) => file,
            Err(e) => {
                println!("  Tuning: host lock - unavailable ({path}: {e})");
                return Self {
                    _file: None,
                    allows_tuning: true,
                };
            },
        };

        #[expect(
            unsafe_code,
            reason = "flock has no std wrapper; the fd is owned and valid"
        )]
        // SAFETY: `file` is an open, owned descriptor for the duration of
        // the call; flock does not touch memory.
        let ret = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };

        if ret == 0 {
            Self {
                _file: Some(file),
                allows_tuning: true,
            }
        } else {
            Self {
                _file: None,
                allows_tuning: false,
            }
        }
    }

    /// Whether this process may apply host-global tuning.
    #[must_use]
    pub fn allows_tuning(&self) -> bool {
        self.allows_tuning
    }

    /// The tuning configuration to actually apply under this lock.
    ///
    /// Contended lock: everything is disabled, because another runner
    /// owns the host settings. Otherwise the requested configuration
    /// passes through unchanged.
    #[must_use]
    #[expect(clippy::print_stdout, reason = "prints why host tuning is skipped")]
    pub fn effective_tuning(&self, requested: &super::TuningConfig) -> super::TuningConfig {
        if self.allows_tuning() {
            requested.clone()
        } else {
            println!("  Tuning: skipped (another bencher runner is active on this host)");
            super::TuningConfig::disabled()
        }
    }
}

#[cfg(test)]
#[cfg(target_os = "linux")]
mod tests {
    use camino::Utf8PathBuf;

    use super::*;

    fn lock_path() -> (tempfile::TempDir, Utf8PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = Utf8PathBuf::try_from(dir.path().join("tuning.lock")).unwrap();
        (dir, path)
    }

    #[test]
    fn first_acquire_allows_tuning() {
        let (_dir, path) = lock_path();
        let lock = HostTuningLock::acquire_at(&path);
        assert!(lock.allows_tuning());
    }

    #[test]
    fn contended_lock_denies_tuning() {
        let (_dir, path) = lock_path();
        let first = HostTuningLock::acquire_at(&path);
        assert!(first.allows_tuning());

        // A second open file description on the same inode contends.
        let second = HostTuningLock::acquire_at(&path);
        assert!(!second.allows_tuning());
    }

    #[test]
    fn dropped_lock_can_be_reacquired() {
        let (_dir, path) = lock_path();
        drop(HostTuningLock::acquire_at(&path));

        let again = HostTuningLock::acquire_at(&path);
        assert!(again.allows_tuning());
    }

    #[test]
    fn unopenable_path_still_allows_tuning() {
        let lock = HostTuningLock::acquire_at(camino::Utf8Path::new(
            "/nonexistent/dir/bencher_runner_tuning.lock",
        ));
        assert!(lock.allows_tuning());
    }

    #[test]
    fn effective_tuning_passes_through_when_held() {
        let (_dir, path) = lock_path();
        let lock = HostTuningLock::acquire_at(&path);
        let config = lock.effective_tuning(&crate::tuning::TuningConfig::default());
        assert!(config.disable_aslr);
    }

    #[test]
    fn effective_tuning_disabled_when_contended() {
        let (_dir, path) = lock_path();
        let _first = HostTuningLock::acquire_at(&path);
        let second = HostTuningLock::acquire_at(&path);

        let config = second.effective_tuning(&crate::tuning::TuningConfig::default());
        assert!(!config.disable_aslr);
        assert!(!config.cpuset_partition);
    }
}
