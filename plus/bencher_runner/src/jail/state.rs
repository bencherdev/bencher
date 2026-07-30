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
use crate::jail::VmId;
use crate::jail::lock::LOCK_FILE;
use crate::jail::reap::Reaped;

/// Subdirectory of the state directory used as the jailer's chroot base.
const CHROOT_BASE: &str = "jail";

/// The chroot directory inside a jail, which the jailer makes `/`.
const JAIL_ROOT: &str = "root";

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
    ///
    /// A read that fails for any reason other than absence is refused rather
    /// than treated as an empty directory. It is not evidence that the
    /// directory is empty, and the chmod that follows is the thing this guard
    /// exists to keep off a directory that is not the runner's: a path that is
    /// really a file, one whose contents cannot be listed, or a listing that
    /// breaks off partway would otherwise be taken on the strength of a failed
    /// check. A listing that ends early is the same failure as one that never
    /// started, so it is reported rather than dropped.
    ///
    /// What the filesystem itself put there is not somebody else's data. See
    /// [`BENIGN_ENTRIES`].
    fn check_root_is_ours(&self) -> Result<(), JailError> {
        let entries = match fs::read_dir(&self.root) {
            Ok(entries) => entries,
            // Missing: creating it is the next step.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(e) => {
                return Err(JailError::ReadStateDir {
                    path: self.root.clone(),
                    source: e,
                });
            },
        };
        let mut populated = false;
        for entry in entries {
            let entry = entry.map_err(|e| JailError::ReadStateDir {
                path: self.root.clone(),
                source: e,
            })?;
            let name = entry.file_name();
            if RUNNER_ENTRIES.iter().any(|ours| name == *ours) {
                return Ok(());
            }
            if BENIGN_ENTRIES.iter().any(|benign| name == *benign) {
                continue;
            }
            populated = true;
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
    pub fn jail_dir(&self, vm_id: &VmId) -> Utf8PathBuf {
        self.jail_parent().join(vm_id.as_str())
    }

    /// The chroot root for a VM, which becomes `/` inside the jail.
    #[must_use]
    pub fn jail_root(&self, vm_id: &VmId) -> Utf8PathBuf {
        self.jail_dir(vm_id).join(JAIL_ROOT)
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

/// Entries that do not make a directory somebody else's.
///
/// A dedicated filesystem is the natural home for the chroots, since each holds
/// a copy of the VMM binary and a full guest rootfs, and moving that traffic
/// off the system disk is the recommended answer to its effect on a run. A
/// freshly created ext4 volume already contains `lost+found` at its mount
/// point, so counting that as somebody else's data would refuse the exact setup
/// the state directory exists to support, and would do it with an error saying
/// the directory was not created by the runner.
///
/// Only what the filesystem itself creates belongs here. Anything a person or
/// another program put there is what the guard is for.
const BENIGN_ENTRIES: [&str; 1] = ["lost+found"];

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
pub fn sweep_jails(jail_parent: &Utf8Path) -> Result<usize, JailError> {
    sweep_jails_with(
        jail_parent,
        super::reap::reap_jailed_vmm,
        super::cgroup::remove_stale_cgroup,
    )
}

/// The sweep, with the reap and the cgroup removal injectable.
///
/// The branch that refuses to remove a directory is the one preventing a
/// destructive action, and it only runs when a real VMM survives a real kill.
/// Manufacturing that would be testing fault injection rather than this code,
/// so the reap is a parameter and the tests supply the answer.
///
/// The cgroup removal is a parameter for a different reason: it reaches into
/// `/sys/fs/cgroup` on the machine running the tests, and a unit test that
/// passes only because a given id happens not to exist on a dev box is
/// reading host state, not this code.
fn sweep_jails_with<R, C>(
    jail_parent: &Utf8Path,
    reap: R,
    remove_cgroup: C,
) -> Result<usize, JailError>
where
    R: Fn(&Utf8Path) -> Reaped,
    C: Fn(&VmId) -> Result<(), JailError>,
{
    let Ok(entries) = fs::read_dir(jail_parent) else {
        return Ok(0);
    };

    let mut swept = 0;
    // The first failure is remembered but does not abandon the rest: one jail
    // whose cgroup will not go away must not leave every other stale jail
    // unreaped, with its chroot and cgroup still in place.
    let mut failure = None;

    for entry in entries.flatten() {
        if !entry.file_type().is_ok_and(|file_type| file_type.is_dir()) {
            continue;
        }
        // Skipped rather than lossily converted. A lossy name rebuilds into a
        // path naming a different file, and everything downstream then works
        // on the wrong one: the reap stats a path that does not exist and
        // reports the jail clear, so a live VMM is neither reaped nor
        // mentioned, and the cgroup removal targets a name nobody created.
        // The runner only ever creates UTF-8 names here, so anything else is
        // not ours to touch.
        let file_name = entry.file_name();
        let Some(name) = file_name.to_str() else {
            eprintln!(
                "Warning: skipping an entry with a non-UTF-8 name under {jail_parent}; the runner did not create it"
            );
            continue;
        };
        let vm_id = VmId::from_chroot_name(name.to_owned());
        let jail_dir = jail_parent.join(vm_id.as_str());

        // Reap before removing, and only remove once the jail is clear.
        // Deleting the tree under a live VMM would not stop it, and it would
        // destroy the only handle for identifying that process later: without
        // the directory the next sweep never sees this id, never removes its
        // cgroup, and the cgroup leaks for good.
        if let Reaped::StillRunning { pid } = reap(&jail_dir.join(JAIL_ROOT)) {
            // Fatal to the job, not to the runner. A stray VMM runs untrusted
            // guest code on the benchmark cores, and nothing downstream
            // catches it: these cgroups claim no exclusive cpuset, so the next
            // job's cpuset applies and verifies cleanly while being contended
            // the whole time. Refusing to measure is the only honest answer.
            //
            // Every surviving jail is reported and the first becomes the
            // error, so an operator sees each one on every attempt rather than
            // once.
            eprintln!(
                "Warning: leaving stale jail {jail_dir} in place because VMM pid {pid} is still running on the benchmark cores."
            );
            if failure.is_none() {
                failure = Some(JailError::JailStillRunning {
                    path: jail_dir.clone(),
                    pid,
                });
            }
            continue;
        }

        // The cgroup goes first, and the chroot only once the cgroup is gone.
        // The two are named by the same id, and the directory is the only
        // handle a later sweep has for finding the cgroup again, so removing
        // the directory while the cgroup survives strands that cgroup for
        // good: the next sweep never sees the id, never retries the removal,
        // and something may still be running on the benchmark cores under it.
        // A leftover cgroup claims nothing, since these cgroups set no
        // exclusive cpuset, but a removal that fails usually means something is
        // still in it, which is why it is reported rather than swallowed.
        if let Err(e) = remove_cgroup(&vm_id) {
            eprintln!(
                "Warning: leaving stale jail {jail_dir} in place because its cgroup could not be removed: {e}"
            );
            if failure.is_none() {
                failure = Some(e);
            }
            continue;
        }

        // A chroot that will not go away costs disk. Worth a warning, not
        // worth refusing to run.
        match fs::remove_dir_all(&jail_dir) {
            Ok(()) => swept += 1,
            Err(e) => eprintln!("Warning: failed to sweep stale jail {jail_dir}: {e}"),
        }
    }

    match failure {
        Some(e) => Err(e),
        None => Ok(swept),
    }
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
            state.jail_dir(&VmId::from_chroot_name("abc".to_owned())),
            "/var/lib/bencher-runner/jail/firecracker/abc"
        );
        // <chroot_base>/<exec_file_name>/<id>/root
        assert_eq!(
            state.jail_root(&VmId::from_chroot_name("abc".to_owned())),
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
    fn a_root_that_cannot_be_read_is_not_assumed_to_be_ours() {
        // A failed read is not an empty directory. A file where the state
        // directory should be reads back `ENOTDIR`, the same way an unlistable
        // directory reads back `EACCES`, and neither says the path is the
        // runner's to chmod.
        let (_dir, root) = temp_root();
        let not_a_dir = root.join("state");
        fs::write(&not_a_dir, b"operator note").unwrap();

        let err = StateDir::new(not_a_dir).create().unwrap_err();

        assert!(
            matches!(err, JailError::ReadStateDir { .. }),
            "a read that failed is reported, not swallowed: {err}"
        );
    }

    #[test]
    fn a_dedicated_filesystem_is_ours_to_take() {
        // A freshly created ext4 volume mounted at the state directory holds
        // `lost+found`, which the filesystem made, not an operator. Refusing it
        // would block the recommended setup with an error blaming the operator
        // for a directory they did not populate.
        let (_dir, root) = temp_root();
        let volume = root.join("volume");
        fs::create_dir_all(volume.join("lost+found")).unwrap();

        StateDir::new(volume.clone()).create().unwrap();

        let mode = fs::metadata(&volume).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
        assert!(volume.join("lost+found").exists(), "left where it was");
    }

    #[test]
    fn a_benign_entry_does_not_launder_a_populated_directory() {
        // The exemption covers what a filesystem creates, not the directory it
        // happens to sit in.
        let (_dir, root) = temp_root();
        let foreign = root.join("var-lib");
        fs::create_dir_all(foreign.join("lost+found")).unwrap();
        fs::create_dir_all(foreign.join("dpkg")).unwrap();

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
        fs::create_dir_all(state.jail_root(&VmId::from_chroot_name("one".to_owned()))).unwrap();
        fs::write(
            state
                .jail_root(&VmId::from_chroot_name("one".to_owned()))
                .join("rootfs.ext4"),
            b"stale",
        )
        .unwrap();
        fs::create_dir_all(state.jail_dir(&VmId::from_chroot_name("two".to_owned()))).unwrap();

        assert_eq!(
            sweep_jails_with(&state.jail_parent(), |_j| Reaped::Clear, |_v| Ok(())).unwrap(),
            2
        );
        assert!(
            !state
                .jail_dir(&VmId::from_chroot_name("one".to_owned()))
                .exists()
        );
        assert!(
            !state
                .jail_dir(&VmId::from_chroot_name("two".to_owned()))
                .exists()
        );
        assert!(state.jail_parent().exists());
    }

    #[test]
    fn sweep_leaves_unrelated_entries_alone() {
        let (_dir, root) = temp_root();
        let state = StateDir::new(root.join("state"));
        state.create().unwrap();

        let note = state.jail_parent().join("NOTES.txt");
        fs::write(&note, b"not a jail").unwrap();
        fs::create_dir_all(state.jail_dir(&VmId::from_chroot_name("stale".to_owned()))).unwrap();

        assert_eq!(
            sweep_jails_with(&state.jail_parent(), |_j| Reaped::Clear, |_v| Ok(())).unwrap(),
            1
        );
        assert!(
            !state
                .jail_dir(&VmId::from_chroot_name("stale".to_owned()))
                .exists()
        );
        assert!(note.exists(), "non-directory entries are not the sweep's");
    }

    #[test]
    fn a_jail_whose_vmm_survives_is_left_in_place() {
        // Removing the tree would not stop the VMM, and it would destroy the
        // only handle for identifying that process on a later sweep.
        let (_dir, root) = temp_root();
        let state = StateDir::new(root.join("state"));
        state.create().unwrap();
        let live = VmId::from_chroot_name("live".to_owned());
        let dead = VmId::from_chroot_name("dead".to_owned());
        fs::create_dir_all(state.jail_root(&live)).unwrap();
        fs::create_dir_all(state.jail_root(&dead)).unwrap();

        let err = sweep_jails_with(
            &state.jail_parent(),
            |jail_root| {
                if jail_root.as_str().contains("live") {
                    Reaped::StillRunning { pid: 4242 }
                } else {
                    Reaped::Clear
                }
            },
            |_vm_id| Ok(()),
        )
        .unwrap_err();

        assert!(
            state.jail_dir(&live).exists(),
            "a jail with a live VMM must not be removed"
        );
        assert!(
            !state.jail_dir(&dead).exists(),
            "one unreapable jail must not abandon the rest of the sweep"
        );
        let message = err.to_string();
        assert!(message.contains("4242"), "names the pid: {message}");
        assert!(message.contains("live"), "names the jail: {message}");
    }

    #[test]
    fn a_surviving_vmm_fails_every_attempt_not_just_the_first() {
        // A host that can never clear a jail has to tell the operator on every
        // job, not once. Nothing latches, so the sweep is re-attempted and
        // reports again.
        let (_dir, root) = temp_root();
        let state = StateDir::new(root.join("state"));
        state.create().unwrap();
        let live = VmId::from_chroot_name("live".to_owned());
        fs::create_dir_all(state.jail_root(&live)).unwrap();

        let stuck = |_jail_root: &Utf8Path| Reaped::StillRunning { pid: 7 };
        for attempt in 1..=3 {
            let err = sweep_jails_with(&state.jail_parent(), stuck, |_vm_id| Ok(())).unwrap_err();
            assert!(
                err.to_string().contains('7'),
                "attempt {attempt} must report the pid"
            );
            assert!(state.jail_dir(&live).exists());
        }
    }

    #[test]
    fn a_jail_whose_cgroup_survives_keeps_the_chroot_that_names_it() {
        // The chroot name is the only handle a later sweep has for finding the
        // cgroup, so a directory removed while its cgroup survives strands
        // that cgroup for good: nothing ever sees the id again. One stuck
        // cgroup must still not abandon the rest of the sweep.
        let (_dir, root) = temp_root();
        let state = StateDir::new(root.join("state"));
        state.create().unwrap();
        let stuck = VmId::from_chroot_name("stuck".to_owned());
        let clear = VmId::from_chroot_name("clear".to_owned());
        fs::create_dir_all(state.jail_root(&stuck)).unwrap();
        fs::create_dir_all(state.jail_root(&clear)).unwrap();

        let err = sweep_jails_with(
            &state.jail_parent(),
            |_jail_root| Reaped::Clear,
            |vm_id| {
                if vm_id.as_str() == "stuck" {
                    Err(JailError::StaleCgroup {
                        path: Utf8PathBuf::from("/sys/fs/cgroup/bencher/stuck"),
                        source: std::io::Error::from(std::io::ErrorKind::DirectoryNotEmpty),
                    })
                } else {
                    Ok(())
                }
            },
        )
        .unwrap_err();

        assert!(
            state.jail_dir(&stuck).exists(),
            "the chroot names the cgroup that has to be retried"
        );
        assert!(
            !state.jail_dir(&clear).exists(),
            "one stuck cgroup must not abandon the rest of the sweep"
        );
        assert!(err.to_string().contains("stuck"), "names the cgroup: {err}");
    }

    #[test]
    fn a_cleared_jail_is_still_swept() {
        let (_dir, root) = temp_root();
        let state = StateDir::new(root.join("state"));
        state.create().unwrap();
        fs::create_dir_all(state.jail_root(&VmId::from_chroot_name("one".to_owned()))).unwrap();

        let swept = sweep_jails_with(
            &state.jail_parent(),
            |_jail_root| Reaped::Clear,
            |_vm_id| Ok(()),
        )
        .unwrap();

        assert_eq!(swept, 1);
    }

    #[test]
    fn sweep_missing_parent_is_zero() {
        let (_dir, root) = temp_root();
        assert_eq!(
            sweep_jails_with(&root.join("nope"), |_j| Reaped::Clear, |_v| Ok(())).unwrap(),
            0
        );
    }
}
