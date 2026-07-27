//! The per-job chroot the jailer confines Firecracker to.
//!
//! The runner builds the job's artifacts directly inside the chroot rather
//! than copying them in afterwards, which is legal because the jailer uses
//! `create_dir_all` for the chroot and does nothing if the path already
//! exists. Because the artifacts no longer live in a `TempDir`, this type
//! carries the cleanup responsibility that `TempDir` used to.

#![expect(clippy::print_stderr, reason = "chroot teardown prints diagnostics")]

use std::fs;
use std::os::unix::fs::{PermissionsExt as _, chown};

use camino::{Utf8Path, Utf8PathBuf};

use crate::error::JailError;
use crate::jail::{JAIL_GID, JAIL_UID, StateDir};

/// A job's chroot tree, removed when this value is dropped.
///
/// The jailer cleans up nothing by design, so teardown is the runner's job.
/// `Drop` covers completion, timeout, cancellation, and every error return;
/// the sweep in `prepare_host` covers the exits that never unwind.
#[derive(Debug)]
pub struct JailDir {
    dir: Utf8PathBuf,
    root: Utf8PathBuf,
}

impl JailDir {
    /// Create the chroot tree for `vm_id` at mode 0700.
    pub fn create(state: &StateDir, vm_id: &str) -> Result<Self, JailError> {
        let dir = state.jail_dir(vm_id);
        let root = state.jail_root(vm_id);

        fs::create_dir_all(&root).map_err(|e| JailError::CreateJail {
            path: root.clone(),
            source: e,
        })?;
        // The jailer chowns the chroot root to the jail uid but does not
        // change the mode of a directory that already exists, so the runner
        // sets it. The tree holds the guest rootfs.
        for path in [&dir, &root] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|e| {
                JailError::CreateJail {
                    path: path.clone(),
                    source: e,
                }
            })?;
        }

        Ok(Self { dir, root })
    }

    /// The chroot root, which becomes `/` inside the jail.
    #[must_use]
    pub fn root(&self) -> &Utf8Path {
        &self.root
    }
}

impl Drop for JailDir {
    fn drop(&mut self) {
        if let Err(e) = fs::remove_dir_all(&self.dir)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!("Warning: failed to remove jail {}: {e}", self.dir);
        }
    }
}

/// Hand a file the runner placed inside the chroot to the jail uid and gid.
///
/// The jailer creates and chowns the chroot root and the device nodes it
/// makes, but it does not recursively chown what the runner put there. Every
/// artifact Firecracker touches has to be handed over explicitly, and getting
/// it wrong produces an opaque boot failure, so each one is checked.
pub fn chown_to_jail(path: &Utf8Path) -> Result<(), JailError> {
    chown(path, Some(JAIL_UID), Some(JAIL_GID)).map_err(|e| JailError::ChownJail {
        path: path.to_owned(),
        source: e,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state_in_tmpdir() -> (tempfile::TempDir, StateDir) {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let state = StateDir::new(root.join("state"));
        state.create().unwrap();
        (dir, state)
    }

    #[test]
    fn create_builds_a_private_chroot_tree() {
        let (_dir, state) = state_in_tmpdir();

        let jail = JailDir::create(&state, "vm-1").unwrap();

        assert_eq!(jail.root(), state.jail_root("vm-1"));
        assert!(jail.root().is_dir());
        for path in [state.jail_dir("vm-1"), state.jail_root("vm-1")] {
            let mode = fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700, "{path} should be private");
        }
    }

    #[test]
    fn create_tolerates_an_existing_directory() {
        let (_dir, state) = state_in_tmpdir();
        fs::create_dir_all(state.jail_root("vm-1")).unwrap();

        JailDir::create(&state, "vm-1").unwrap();
    }

    #[test]
    fn drop_removes_the_whole_tree() {
        let (_dir, state) = state_in_tmpdir();

        {
            let jail = JailDir::create(&state, "vm-1").unwrap();
            fs::write(jail.root().join("rootfs.ext4"), b"guest").unwrap();
            fs::create_dir_all(jail.root().join("dev")).unwrap();
        }

        assert!(
            !state.jail_dir("vm-1").exists(),
            "the chroot is the runner's to reclaim, not the jailer's"
        );
        assert!(state.jail_parent().exists());
    }

    #[test]
    fn drop_tolerates_an_already_removed_tree() {
        let (_dir, state) = state_in_tmpdir();
        let jail = JailDir::create(&state, "vm-1").unwrap();
        fs::remove_dir_all(state.jail_dir("vm-1")).unwrap();
        drop(jail);
    }
}
