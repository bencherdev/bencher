//! The runner's persistent state directory.
//!
//! Everything the jail needs that must outlive a single job hangs off one
//! directory: the chroot base the jailer builds under, and the sweep that
//! reclaims chroots left behind by a runner that exited without unwinding.

#![expect(clippy::print_stderr, reason = "host preparation prints diagnostics")]

use std::fs;
use std::os::unix::fs::PermissionsExt as _;

use camino::{Utf8Path, Utf8PathBuf};

use crate::error::JailError;

/// Subdirectory of the state directory used as the jailer's chroot base.
const CHROOT_BASE: &str = "jail";

/// The jail lock file, which lives beside the chroot base.
///
/// Named here as well as in the lock module so the state directory knows
/// which of its entries it created.
const LOCK_FILE: &str = ".lock";

/// The `--exec-file` base name the jailer derives the chroot layout from.
///
/// The jailer builds `<chroot_base>/<exec_file_name>/<id>/root`, so the
/// staged Firecracker binary must be named exactly this for the runner and
/// the jailer to agree on where the chroot lives.
pub(crate) const EXEC_FILE_NAME: &str = "firecracker";

/// The runner's persistent state directory.
///
/// Created at mode 0700 owned by root: it holds every job's chroot, which
/// contains the guest rootfs and the copied VMM binary.
#[derive(Debug, Clone)]
pub struct StateDir {
    root: Utf8PathBuf,
}

impl StateDir {
    /// Create a handle for the state directory rooted at `root`.
    #[must_use]
    pub fn new(root: Utf8PathBuf) -> Self {
        Self { root }
    }

    /// The state directory itself.
    #[must_use]
    pub fn path(&self) -> &Utf8Path {
        &self.root
    }

    /// Refuse a state directory that belongs to the host rather than to us.
    ///
    /// A path that does not exist, or exists and is empty, is ours to take. A
    /// populated one is only ours if it already carries something the runner
    /// put there. Without this, `--state-dir /var/lib` would be chmodded to
    /// 0700 and take the host down with it.
    fn check_root_is_ours(&self) -> Result<(), JailError> {
        let Ok(mut entries) = fs::read_dir(&self.root) else {
            // Missing, or unreadable: creating it is the next step and will
            // report the real error.
            return Ok(());
        };
        let mut populated = false;
        for entry in entries.by_ref().flatten() {
            populated = true;
            let name = entry.file_name();
            if RUNNER_ENTRIES.iter().any(|ours| name == *ours) {
                return Ok(());
            }
        }
        if populated {
            return Err(JailError::ForeignStateDir {
                path: self.root.clone(),
            });
        }
        Ok(())
    }

    /// The jailer's `--chroot-base-dir`.
    #[must_use]
    pub fn chroot_base(&self) -> Utf8PathBuf {
        self.root.join(CHROOT_BASE)
    }

    /// The directory holding one subdirectory per jailed VMM.
    ///
    /// This is the level the sweep operates on.
    #[must_use]
    pub fn jail_parent(&self) -> Utf8PathBuf {
        self.chroot_base().join(EXEC_FILE_NAME)
    }

    /// The jail directory for a VM, the tree teardown removes.
    #[must_use]
    pub fn jail_dir(&self, vm_id: &str) -> Utf8PathBuf {
        self.jail_parent().join(vm_id)
    }

    /// The chroot root for a VM, which becomes `/` inside the jail.
    #[must_use]
    pub fn jail_root(&self, vm_id: &str) -> Utf8PathBuf {
        self.jail_dir(vm_id).join("root")
    }

    /// Create the state directory tree at mode 0700.
    ///
    /// Idempotent. The mode is applied on every call so a directory created
    /// with a laxer mode by an older runner is tightened on upgrade. That
    /// tightening is why the root has to be one the runner owns: pointed at a
    /// populated system directory it would otherwise chmod that directory to
    /// 0700 and break the host.
    pub fn create(&self) -> Result<(), JailError> {
        self.check_root_is_ours()?;
        for dir in [&self.root, &self.chroot_base(), &self.jail_parent()] {
            fs::create_dir_all(dir).map_err(|e| JailError::CreateStateDir {
                path: dir.clone(),
                source: e,
            })?;
            fs::set_permissions(dir, fs::Permissions::from_mode(0o700)).map_err(|e| {
                JailError::CreateStateDir {
                    path: dir.clone(),
                    source: e,
                }
            })?;
        }
        Ok(())
    }
}

/// Entries the runner creates directly in its state directory.
///
/// Their presence is what distinguishes a directory the runner has used from
/// one that belongs to the host.
const RUNNER_ENTRIES: [&str; 2] = [CHROOT_BASE, LOCK_FILE];

/// Remove every jail directory under `jail_parent`, returning how many were
/// reclaimed.
///
/// Jobs run serially, so anything found here is stale by construction. The
/// runner disappears without unwinding in several ordinary ways, including
/// SIGKILL, a crash, and the `exec` in a self-update, and `Drop` runs in
/// none of them. Each leftover chroot holds a copy of the VMM binary and a
/// full guest rootfs image, so leaving them is not an option.
///
/// Non-directory entries are left alone: the jailer only ever creates
/// directories here, so anything else was put there by someone else.
pub fn sweep_jails(jail_parent: &Utf8Path) -> usize {
    let Ok(entries) = fs::read_dir(jail_parent) else {
        return 0;
    };

    let mut swept = 0;
    for entry in entries.flatten() {
        let path = entry.path();
        if !entry.file_type().is_ok_and(|file_type| file_type.is_dir()) {
            continue;
        }
        match fs::remove_dir_all(&path) {
            Ok(()) => swept += 1,
            Err(e) => eprintln!(
                "Warning: failed to sweep stale jail {}: {e}",
                path.display()
            ),
        }
    }
    swept
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_root() -> (tempfile::TempDir, Utf8PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        (dir, root)
    }

    #[test]
    fn jail_layout_matches_jailer_template() {
        let state = StateDir::new(Utf8PathBuf::from("/var/lib/bencher-runner"));
        assert_eq!(state.chroot_base(), "/var/lib/bencher-runner/jail");
        assert_eq!(
            state.jail_parent(),
            "/var/lib/bencher-runner/jail/firecracker"
        );
        assert_eq!(
            state.jail_dir("abc"),
            "/var/lib/bencher-runner/jail/firecracker/abc"
        );
        // <chroot_base>/<exec_file_name>/<id>/root
        assert_eq!(
            state.jail_root("abc"),
            state
                .chroot_base()
                .join(EXEC_FILE_NAME)
                .join("abc")
                .join("root")
        );
    }

    #[test]
    fn create_is_idempotent_and_private() {
        let (_dir, root) = temp_root();
        let state = StateDir::new(root.join("state"));

        state.create().unwrap();
        state.create().unwrap();

        for dir in [state.path(), &state.chroot_base(), &state.jail_parent()] {
            let mode = fs::metadata(dir).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700, "{dir} should be private");
        }
    }

    #[test]
    fn create_tightens_a_lax_directory() {
        let (_dir, root) = temp_root();
        let state = StateDir::new(root.join("state"));
        fs::create_dir_all(state.path()).unwrap();
        fs::set_permissions(state.path(), fs::Permissions::from_mode(0o755)).unwrap();

        state.create().unwrap();

        let mode = fs::metadata(state.path()).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
    }

    #[test]
    fn a_populated_foreign_directory_is_refused() {
        // The mode tightening would otherwise chmod a system directory to
        // 0700: `--state-dir /var/lib` must not take the host down.
        let (_dir, root) = temp_root();
        let foreign = root.join("var-lib");
        fs::create_dir_all(foreign.join("dpkg")).unwrap();
        fs::create_dir_all(foreign.join("systemd")).unwrap();

        StateDir::new(foreign.clone()).create().unwrap_err();

        let mode = fs::metadata(&foreign).unwrap().permissions().mode();
        assert_ne!(mode & 0o777, 0o700, "a refused root must not be chmodded");
    }

    #[test]
    fn an_empty_directory_is_ours_to_take() {
        let (_dir, root) = temp_root();
        let empty = root.join("empty");
        fs::create_dir_all(&empty).unwrap();

        StateDir::new(empty).create().unwrap();
    }

    #[test]
    fn a_directory_the_runner_already_used_is_ours() {
        let (_dir, root) = temp_root();
        let state = StateDir::new(root.join("state"));
        state.create().unwrap();
        // Something the host put there afterwards does not disown it.
        fs::write(state.path().join("notes.txt"), b"operator note").unwrap();

        state.create().unwrap();
    }

    #[test]
    fn a_directory_holding_only_the_lock_is_ours() {
        let (_dir, root) = temp_root();
        let state = root.join("state");
        fs::create_dir_all(&state).unwrap();
        fs::write(state.join(".lock"), b"").unwrap();

        StateDir::new(state).create().unwrap();
    }

    #[test]
    fn sweep_removes_stale_jails() {
        let (_dir, root) = temp_root();
        let state = StateDir::new(root.join("state"));
        state.create().unwrap();

        // Two stale jails, one with a nested chroot tree.
        fs::create_dir_all(state.jail_root("one")).unwrap();
        fs::write(state.jail_root("one").join("rootfs.ext4"), b"stale").unwrap();
        fs::create_dir_all(state.jail_dir("two")).unwrap();

        assert_eq!(sweep_jails(&state.jail_parent()), 2);
        assert!(!state.jail_dir("one").exists());
        assert!(!state.jail_dir("two").exists());
        assert!(state.jail_parent().exists());
    }

    #[test]
    fn sweep_leaves_unrelated_entries_alone() {
        let (_dir, root) = temp_root();
        let state = StateDir::new(root.join("state"));
        state.create().unwrap();

        let note = state.jail_parent().join("NOTES.txt");
        fs::write(&note, b"not a jail").unwrap();
        fs::create_dir_all(state.jail_dir("stale")).unwrap();

        assert_eq!(sweep_jails(&state.jail_parent()), 1);
        assert!(!state.jail_dir("stale").exists());
        assert!(note.exists(), "non-directory entries are not the sweep's");
    }

    #[test]
    fn sweep_missing_parent_is_zero() {
        let (_dir, root) = temp_root();
        assert_eq!(sweep_jails(&root.join("nope")), 0);
    }
}
