//! The per-job chroot, which the runner fills before the jailer runs because
//! the jailer's `create_dir_all` accepts a path that already exists.

#![expect(clippy::print_stderr, reason = "chroot teardown prints diagnostics")]

use std::fs;
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _, lchown};

use camino::{Utf8Path, Utf8PathBuf};

use crate::error::JailError;
use crate::jail::{CgroupSurvived, JailUser, StateDir, VmId};

/// A job's chroot tree, removed on drop because the jailer cleans up nothing.
#[derive(Debug)]
pub struct JailDir {
    dir: Utf8PathBuf,
    root: Utf8PathBuf,
    cgroup_survived: CgroupSurvived,
}

impl JailDir {
    /// Create the chroot tree for `vm_id` at mode 0700.
    pub fn create(
        state: &StateDir,
        vm_id: &VmId,
        cgroup_survived: CgroupSurvived,
    ) -> Result<Self, JailError> {
        Self::create_with(state, vm_id, cgroup_survived, make_private)
    }

    /// `create` with the mode tightening injectable, so a test can fail it with
    /// the tree already on disk.
    fn create_with<P>(
        state: &StateDir,
        vm_id: &VmId,
        cgroup_survived: CgroupSurvived,
        make_private: P,
    ) -> Result<Self, JailError>
    where
        P: Fn(&Utf8Path) -> Result<(), JailError>,
    {
        // Re-checked per job because `create_dir_all` follows a component
        // swapped for a link after host preparation.
        state.check_root_is_ours()?;

        let dir = state.jail_dir(vm_id);
        let root = state.jail_root(vm_id);

        fs::create_dir_all(&root).map_err(|e| JailError::CreateJail {
            path: root.clone(),
            source: e,
        })?;
        // Built before `make_private` so a failure below is torn down by `Drop`.
        let jail = Self {
            dir,
            root,
            cgroup_survived,
        };
        // Private now, because the runner builds the guest rootfs here before
        // the jailer tightens the root itself.
        make_private(&jail.dir)?;
        make_private(&jail.root)?;

        Ok(jail)
    }

    /// The chroot root, which becomes `/` inside the jail.
    #[must_use]
    pub fn root(&self) -> &Utf8Path {
        &self.root
    }
}

impl Drop for JailDir {
    fn drop(&mut self) {
        // The chroot's name is a later sweep's only handle on a cgroup this job
        // could not remove, so it stays for the next job to reclaim both.
        if self.cgroup_survived.is_set() {
            eprintln!(
                "Warning: leaving jail {} in place because its cgroup could not be removed. The directory names that cgroup, so the next job sweeps both.",
                self.dir
            );
            return;
        }

        if let Err(e) = fs::remove_dir_all(&self.dir)
            && e.kind() != std::io::ErrorKind::NotFound
        {
            eprintln!(
                "Warning: failed to remove jail {}: {e}. It holds a VMM binary and a full guest rootfs; the next job will sweep it.",
                self.dir
            );
        }
    }
}

/// Tighten one directory to 0700 through an `O_NOFOLLOW` descriptor, so a
/// component swapped for a link never has root chmod the link's target.
fn make_private(path: &Utf8Path) -> Result<(), JailError> {
    let opened = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(path)
        .map_err(|e| JailError::CreateJail {
            path: path.to_owned(),
            source: e,
        })?;
    opened
        .set_permissions(fs::Permissions::from_mode(0o700))
        .map_err(|e| JailError::CreateJail {
            path: path.to_owned(),
            source: e,
        })
}

/// Let the jailed VMM read a file that stays owned by root, so the VMM can
/// never write it or chmod it into something it can write.
pub fn grant_jail_read(path: &Utf8Path) -> Result<(), JailError> {
    fs::set_permissions(path, fs::Permissions::from_mode(0o644)).map_err(|e| JailError::ChmodJail {
        path: path.to_owned(),
        source: e,
    })
}

/// Hand a file Firecracker writes to the jail user without following a link,
/// since the jailer's chown of the chroot root is not recursive.
pub fn chown_to_jail(path: &Utf8Path, jail_user: JailUser) -> Result<(), JailError> {
    lchown(path, Some(jail_user.uid()), Some(jail_user.gid())).map_err(|e| JailError::ChownJail {
        path: path.to_owned(),
        source: e,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vm_id() -> VmId {
        VmId::from_chroot_name("vm-1".to_owned()).unwrap()
    }

    fn state_in_tmpdir() -> (tempfile::TempDir, StateDir) {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let state = StateDir::new(root.join("state")).unwrap();
        state.create().unwrap();
        (dir, state)
    }

    #[test]
    fn create_builds_a_private_chroot_tree() {
        let (_dir, state) = state_in_tmpdir();

        let jail = JailDir::create(&state, &vm_id(), CgroupSurvived::default()).unwrap();

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

        JailDir::create(&state, &vm_id(), CgroupSurvived::default()).unwrap();
    }

    #[test]
    fn drop_removes_the_whole_tree() {
        let (_dir, state) = state_in_tmpdir();

        {
            let jail = JailDir::create(&state, &vm_id(), CgroupSurvived::default()).unwrap();
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
    fn a_component_swapped_for_a_link_after_preparation_is_refused() {
        // Fails if a job stops re-checking the state tree, letting a
        // `jail/firecracker` swapped for a link aim the chroot at its target.
        use std::os::unix::fs::symlink;
        let (_dir, state) = state_in_tmpdir();
        let victim = state.path().parent().unwrap().join("victim");
        fs::create_dir_all(victim.join("someone-elses-data")).unwrap();
        fs::remove_dir_all(state.jail_parent()).unwrap();
        symlink(&victim, state.jail_parent()).unwrap();

        let err = JailDir::create(&state, &vm_id(), CgroupSurvived::default()).unwrap_err();

        assert!(
            matches!(err, JailError::SymlinkedStateDir { .. }),
            "a swapped component is refused, not followed: {err}"
        );
        assert!(
            victim.join("someone-elses-data").is_dir(),
            "the link's target must come away untouched"
        );
        assert!(
            !victim.join(vm_id().as_str()).exists(),
            "no chroot may be built through the link"
        );
    }

    #[test]
    fn an_unbuildable_chroot_is_an_error_not_a_warning() {
        // Fails if a chroot that cannot be built degrades into an unjailed run
        // instead of aborting the job.
        let (_dir, state) = state_in_tmpdir();
        fs::write(state.jail_dir(&vm_id()), b"in the way").unwrap();

        JailDir::create(&state, &vm_id(), CgroupSurvived::default()).unwrap_err();
    }

    #[test]
    fn a_chroot_that_could_not_be_finished_is_not_left_behind() {
        // Fails if a step after `create_dir_all` leaks the tree instead of
        // handing it to `Drop`.
        let (_dir, state) = state_in_tmpdir();

        JailDir::create_with(&state, &vm_id(), CgroupSurvived::default(), |path| {
            Err(JailError::CreateJail {
                path: path.to_owned(),
                source: std::io::Error::from(std::io::ErrorKind::PermissionDenied),
            })
        })
        .unwrap_err();

        assert!(
            !state.jail_dir(&vm_id()).exists(),
            "a chroot that could not be finished is reclaimed, not leaked"
        );
        assert!(state.jail_parent().exists(), "only this jail comes away");
    }

    #[test]
    fn a_surviving_cgroup_holds_the_chroot_that_names_it() {
        // Fails if `Drop` removes the chroot while its cgroup survives, leaving
        // a later sweep nothing to find the cgroup by.
        let (_dir, state) = state_in_tmpdir();
        let cgroup_survived = CgroupSurvived::default();
        let jail = JailDir::create(&state, &vm_id(), cgroup_survived.clone()).unwrap();
        fs::write(jail.root().join("rootfs.ext4"), b"guest").unwrap();

        cgroup_survived.set();
        drop(jail);

        assert!(
            state.jail_dir(&vm_id()).exists(),
            "the chroot names the cgroup that still has to be removed"
        );
    }

    #[test]
    fn a_link_is_handed_over_without_its_target() {
        // Fails if the chown follows a link, handing the jail user its target.
        use std::os::unix::fs::{MetadataExt as _, symlink};

        if crate::jail::current_euid() != 0 {
            eprintln!("skipped a_link_is_handed_over_without_its_target: the chown needs root");
            return;
        }
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(dir.path()).unwrap();
        let target = root.join("host-file");
        fs::write(&target, b"host").unwrap();
        let link = root.join("rootfs.ext4");
        symlink(&target, &link).unwrap();

        chown_to_jail(&link, JailUser::new(4242, 4243).unwrap()).unwrap();

        let target = fs::metadata(&target).unwrap();
        assert_eq!(
            (target.uid(), target.gid()),
            (0, 0),
            "the target must stay root's"
        );
        let link = fs::symlink_metadata(&link).unwrap();
        assert_eq!((link.uid(), link.gid()), (4242, 4243));
    }

    #[test]
    fn drop_tolerates_an_already_removed_tree() {
        let (_dir, state) = state_in_tmpdir();
        let jail = JailDir::create(&state, &vm_id(), CgroupSurvived::default()).unwrap();
        fs::remove_dir_all(state.jail_dir(&vm_id())).unwrap();
        drop(jail);
    }
}
