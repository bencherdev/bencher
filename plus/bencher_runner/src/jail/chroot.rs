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
use crate::jail::{JailUser, ReclaimFailed, StateDir, VmId};

/// A job's chroot tree, removed when this value is dropped.
///
/// The jailer cleans up nothing by design, so teardown is the runner's job.
/// `Drop` covers completion, timeout, cancellation, and every error return;
/// the sweep in `prepare_host` covers the exits that never unwind.
#[derive(Debug)]
pub struct JailDir {
    dir: Utf8PathBuf,
    root: Utf8PathBuf,
    reclaim_failed: ReclaimFailed,
}

impl JailDir {
    /// Create the chroot tree for `vm_id` at mode 0700.
    pub fn create(
        state: &StateDir,
        vm_id: &VmId,
        reclaim_failed: ReclaimFailed,
    ) -> Result<Self, JailError> {
        let dir = state.jail_dir(vm_id);
        let root = state.jail_root(vm_id);

        fs::create_dir_all(&root).map_err(|e| JailError::CreateJail {
            path: root.clone(),
            source: e,
        })?;
        // The jailer eventually sets the chroot root to 0700 owned by the jail
        // user, but only once it runs. The runner builds the guest rootfs in
        // here before that, so the tree is private from the moment it exists.
        for path in [&dir, &root] {
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).map_err(|e| {
                JailError::CreateJail {
                    path: path.clone(),
                    source: e,
                }
            })?;
        }

        Ok(Self {
            dir,
            root,
            reclaim_failed,
        })
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
            eprintln!(
                "Warning: failed to remove jail {}: {e}. It holds a VMM binary and a full guest rootfs; the next job will sweep it.",
                self.dir
            );
            self.reclaim_failed.set();
        }
    }
}

/// Let the jailed VMM read a file without giving it away.
///
/// Firecracker only ever reads the kernel image, so it gets read permission
/// and nothing more: the file stays owned by root, which means the VMM cannot
/// write it and cannot chmod it into something it can write. The mode is set
/// explicitly rather than inherited, because a bundled write or a copy from
/// the host can land at 0600 and leave the VMM unable to read its own kernel.
pub fn grant_jail_read(path: &Utf8Path) -> Result<(), JailError> {
    // Reported as a mode failure, not an ownership one. This function
    // deliberately leaves the file owned by root, so an operator sent looking
    // at ownership would be chasing the opposite of what went wrong.
    fs::set_permissions(path, fs::Permissions::from_mode(0o644)).map_err(|e| JailError::ChmodJail {
        path: path.to_owned(),
        source: e,
    })
}

/// Hand a file the runner placed inside the chroot to the jail uid and gid.
///
/// The jailer chowns the chroot root and the device nodes it makes, but that
/// chown is not recursive: files the runner placed inside keep the ownership
/// they were created with, which is root. Every artifact Firecracker *writes*
/// has to be handed over explicitly, and getting it wrong produces an opaque
/// boot failure, so each one is checked. Anything it only reads gets
/// [`grant_jail_read`] instead.
pub fn chown_to_jail(path: &Utf8Path, jail_user: JailUser) -> Result<(), JailError> {
    chown(path, Some(jail_user.uid()), Some(jail_user.gid())).map_err(|e| JailError::ChownJail {
        path: path.to_owned(),
        source: e,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A stand-in identity for tests.
    fn vm_id() -> VmId {
        VmId::from_chroot_name("vm-1".to_owned())
    }

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

        let jail = JailDir::create(&state, &vm_id(), ReclaimFailed::default()).unwrap();

        assert_eq!(jail.root(), state.jail_root(&vm_id()));
        assert!(jail.root().is_dir());
        for path in [state.jail_dir(&vm_id()), state.jail_root(&vm_id())] {
            let mode = fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700, "{path} should be private");
        }
    }

    #[test]
    fn create_tolerates_an_existing_directory() {
        let (_dir, state) = state_in_tmpdir();
        fs::create_dir_all(state.jail_root(&vm_id())).unwrap();

        JailDir::create(&state, &vm_id(), ReclaimFailed::default()).unwrap();
    }

    #[test]
    fn drop_removes_the_whole_tree() {
        let (_dir, state) = state_in_tmpdir();

        {
            let jail = JailDir::create(&state, &vm_id(), ReclaimFailed::default()).unwrap();
            fs::write(jail.root().join("rootfs.ext4"), b"guest").unwrap();
            fs::create_dir_all(jail.root().join("dev")).unwrap();
        }

        assert!(
            !state.jail_dir(&vm_id()).exists(),
            "the chroot is the runner's to reclaim, not the jailer's"
        );
        assert!(state.jail_parent().exists());
    }

    #[test]
    fn an_unbuildable_chroot_is_an_error_not_a_warning() {
        // A chroot that cannot be built is a confinement failure, so it has
        // to abort the job rather than degrade into an unjailed run.
        let (_dir, state) = state_in_tmpdir();
        // A file where the jail directory has to go makes the tree
        // impossible to create.
        fs::write(state.jail_dir(&vm_id()), b"in the way").unwrap();

        JailDir::create(&state, &vm_id(), ReclaimFailed::default()).unwrap_err();
    }

    #[test]
    fn drop_tolerates_an_already_removed_tree() {
        let (_dir, state) = state_in_tmpdir();
        let jail = JailDir::create(&state, &vm_id(), ReclaimFailed::default()).unwrap();
        fs::remove_dir_all(state.jail_dir(&vm_id())).unwrap();
        drop(jail);
    }
}
