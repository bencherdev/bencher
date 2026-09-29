//! Everything the jail needs that must outlive a single job hangs off one
//! state directory.

#![expect(clippy::print_stderr, reason = "host preparation prints diagnostics")]
#![expect(
    clippy::print_stdout,
    reason = "the sweep announces a reclamation that outlives the interval"
)]

use std::fs;
use std::os::unix::fs::{MetadataExt as _, OpenOptionsExt as _, PermissionsExt as _, fchown};

use camino::{Utf8Path, Utf8PathBuf};

use crate::error::JailError;
use crate::jail::lock::LOCK_FILE;
use crate::jail::reap::Reaped;
use crate::jail::{JailLock, VmId};

const CHROOT_BASE: &str = "jail";

const JAIL_ROOT: &str = "root";

/// The jailer builds `<chroot_base>/<exec_file_name>/<id>/root`, so the
/// staged binary must carry exactly this name for the runner and the jailer
/// to agree on the chroot.
pub(crate) const EXEC_FILE_NAME: &str = "firecracker";

/// Root-owned at mode 0700, since it holds every job's chroot with a guest
/// rootfs and a copy of the VMM binary.
#[derive(Debug, Clone)]
pub struct StateDir {
    root: Utf8PathBuf,
}

impl StateDir {
    /// The absolute check and the symlink resolution live here, not in
    /// [`Self::create`], because the job path builds its handle without ever
    /// calling it.
    pub fn new(root: Utf8PathBuf) -> Result<Self, JailError> {
        crate::jail::check_absolute_state_dir(&root)?;
        let root = resolve_symlinked_root(root)?;
        Ok(Self { root })
    }

    #[must_use]
    pub fn path(&self) -> &Utf8Path {
        &self.root
    }

    /// Refuse a root that is not provably the runner's, since chmodding
    /// something like `--state-dir /var/lib` to 0700 takes the host down.
    pub(crate) fn check_root_is_ours(&self) -> Result<(), JailError> {
        let chroot_base = self.chroot_base();
        let jail_parent = self.jail_parent();

        let root_exists = real_dir(&self.root)?;
        let base_exists = real_dir(&chroot_base)?;
        // The one thing that settles it: only this runner builds this path.
        if real_dir(&jail_parent)? {
            return Ok(());
        }

        if root_exists && let Some(entry) = first_foreign_entry(&self.root, &OUR_ROOT_ENTRIES)? {
            return Err(JailError::ForeignStateDir {
                path: self.root.clone(),
                entry,
            });
        }
        // `firecracker` cannot be here yet, but tolerating it states the tree's
        // rule rather than leaning on the order of these checks.
        if base_exists && let Some(entry) = first_foreign_entry(&chroot_base, &[EXEC_FILE_NAME])? {
            return Err(JailError::ForeignStateDir {
                path: self.root.clone(),
                entry: format!("{CHROOT_BASE}/{entry}"),
            });
        }
        Ok(())
    }

    /// The jailer's `--chroot-base-dir`.
    #[must_use]
    pub fn chroot_base(&self) -> Utf8PathBuf {
        self.root.join(CHROOT_BASE)
    }

    /// One subdirectory per jailed VMM, the level the sweep operates on.
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

    /// Each level is taken for root before the next is created, so a link
    /// planted below a taken level is never followed, and one at the root
    /// fails the `O_NOFOLLOW` open in `make_private`.
    pub fn create(&self) -> Result<(), JailError> {
        self.check_root_is_ours()?;
        for dir in [&self.root, &self.chroot_base(), &self.jail_parent()] {
            fs::create_dir_all(dir).map_err(|e| JailError::CreateStateDir {
                path: dir.clone(),
                source: e,
            })?;
            make_private(dir)?;
        }
        Ok(())
    }

    /// A `nodev` or `noexec` mount would otherwise fail only once the guest is
    /// booting, blaming something else.
    pub fn refuse_unusable_mount(&self) -> Result<(), JailError> {
        use nix::sys::statvfs::{FsFlags, statvfs};

        let flags = statvfs(self.root.as_std_path())
            .map_err(|e| JailError::ReadStateDir {
                path: self.root.clone(),
                source: e.into(),
            })?
            .flags();
        for (flag, option) in [(FsFlags::ST_NODEV, "nodev"), (FsFlags::ST_NOEXEC, "noexec")] {
            if flags.contains(flag) {
                return Err(JailError::StateDirMountOption {
                    path: self.root.clone(),
                    option,
                });
            }
        }
        Ok(())
    }

    /// Run under the job lock before the job builds its own jail, so anything
    /// found is stale whichever runner process left it.
    pub fn sweep(&self, _lock: &JailLock) -> Result<(), JailError> {
        let reclaimed = sweep_jails(&self.jail_parent())?;
        if reclaimed > 0 {
            // Each held a VMM binary and a full guest rootfs, so an operator
            // should hear about it.
            println!("  Reclaimed {reclaimed} stale jail(s) from {}", self.root);
        }
        Ok(())
    }
}

/// A link is refused rather than resolved, so the guard never answers for one
/// directory while the chmod and the sweep act on another.
fn real_dir(path: &Utf8Path) -> Result<bool, JailError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(JailError::SymlinkedStateDir {
            path: path.to_owned(),
        }),
        Ok(metadata) if metadata.is_dir() => Ok(true),
        Ok(_) => Err(JailError::ReadStateDir {
            path: path.to_owned(),
            source: std::io::Error::from(std::io::ErrorKind::NotADirectory),
        }),
        // Missing: creating it is the next step.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(JailError::ReadStateDir {
            path: path.to_owned(),
            source: e,
        }),
    }
}

/// Resolve a symlinked root the operator is proven to have chosen, or leave it.
///
/// A dedicated filesystem for the chroots is the recommended setup, since each
/// chroot holds a copy of the VMM binary and a full guest rootfs, and one
/// accepted way to point at one is a root-owned symlink,
/// `/var/lib/bencher-runner -> /mnt/disk/runner`. Following a symlink blindly is
/// the hazard the guard exists for, and following it partway is worse than not
/// following it: `canonicalize` collapses a whole chain, so a single check on the
/// link's own parent would still walk a second hop an attacker planted in a loose
/// intermediate directory it never looked at. The link is followed only when all
/// three of these hold, and on any doubt it is left exactly as given so
/// [`real_dir`] refuses it with [`JailError::SymlinkedStateDir`] at create time,
/// unchanged from a root that was never a symlink:
///
/// 1. The directory that holds the link is writable by nobody but root, so only
///    the operator could have created the link itself. See
///    [`parent_is_operator_controlled`]. Without it, an attacker who owns a
///    world-writable directory plants a link there pointing at another service's
///    directory and redirects root into clobbering it.
/// 2. The link is a single hop to an absolute, already-canonical target: the path
///    stored in the link is the same path `canonicalize` resolves it to. A second
///    symlink further down the chain, a symlink component in the target's own
///    path, or a relative or `..`-laden target all make the two differ, so a hop
///    through a directory this never vetted cannot smuggle a redirection past
///    here. This is what refuses `/var/lib/rs -> /mnt/x/runner` when
///    `/mnt/x/runner` is itself a link an attacker planted in a world-writable
///    `/mnt/x`.
/// 3. The resolved target lives in a subtree writable by nobody but root, top to
///    bottom: the target and every ancestor up to `/`. See
///    [`ancestry_is_operator_controlled`]. `canonicalize` returned a path with no
///    symlink components, so these are real directories; a group- or
///    other-writable one anywhere on the way to `/` is a place an unprivileged
///    user could have arranged what the path resolves to, and vetting the whole
///    ancestry also closes the window between resolving and the 0700 chmod, since
///    nobody but root can then swap the target either.
///
/// Together these make following the link no different from passing the resolved
/// target as `--state-dir` directly: exactly one link, made by root, leads to an
/// absolute canonical directory reachable only through root-controlled
/// directories, so no unprivileged user influenced the result or can race it. The
/// populated-directory check in [`StateDir::check_root_is_ours`] is the remaining
/// layer: it runs on the resolved target, so a root-made link pointing at a
/// populated system directory like `/etc` is still refused for the foreign
/// entries it holds.
///
/// A real-directory root is returned verbatim, never canonicalized: the runner
/// and the jailer share the one string, and its ancestor symlinks are the
/// operator's system layout the guard already trusts. A root that is not there
/// yet is created later, and any read failure, including a dangling link, is left
/// for [`real_dir`] to turn into an error at create time, so it has one home. Only
/// a symlink at the root itself is a candidate for resolution here. The resolved
/// target is stored as the root, so every later step names the concrete path this
/// resolved rather than re-walking the link.
fn resolve_symlinked_root(root: Utf8PathBuf) -> Result<Utf8PathBuf, JailError> {
    resolve_symlinked_root_with(root, operator_controlled)
}

/// The judgment is injectable because only root can build a root-only-writable
/// ancestry, so the unit tests stub it to reach the accept path.
fn resolve_symlinked_root_with<P>(
    root: Utf8PathBuf,
    is_operator_controlled: P,
) -> Result<Utf8PathBuf, JailError>
where
    P: Fn(&Utf8Path) -> Result<bool, JailError>,
{
    let Ok(metadata) = root.symlink_metadata() else {
        return Ok(root);
    };
    if !metadata.file_type().is_symlink() {
        return Ok(root);
    }
    if !parent_is_operator_controlled(&root, &is_operator_controlled)? {
        return Ok(root);
    }
    // Showing (2): a single hop to an absolute, already-canonical target.
    let (Ok(target), Ok(resolved)) = (root.read_link_utf8(), root.canonicalize_utf8()) else {
        return Ok(root);
    };
    if target != resolved {
        return Ok(root);
    }
    if !ancestry_is_operator_controlled(&resolved, &is_operator_controlled)? {
        return Ok(root);
    }
    Ok(resolved)
}

/// Follows symlinks deliberately, since a parent's ancestors are the operator's
/// system layout and the ancestry walk is already canonical.
fn operator_controlled(dir: &Utf8Path) -> Result<bool, JailError> {
    let metadata = dir.metadata().map_err(|source| JailError::ReadStateDir {
        path: dir.to_owned(),
        source,
    })?;
    Ok(owner_only_writable(metadata.uid(), metadata.mode()))
}

fn parent_is_operator_controlled<P>(
    root: &Utf8Path,
    is_operator_controlled: &P,
) -> Result<bool, JailError>
where
    P: Fn(&Utf8Path) -> Result<bool, JailError>,
{
    let Some(parent) = root.parent() else {
        // Only `/` has no parent and it is never a symlink, so refuse rather
        // than assume.
        return Ok(false);
    };
    is_operator_controlled(parent)
}

/// `dir` must be canonical, so the walk judges real directories rather than
/// following a link.
fn ancestry_is_operator_controlled<P>(
    dir: &Utf8Path,
    is_operator_controlled: &P,
) -> Result<bool, JailError>
where
    P: Fn(&Utf8Path) -> Result<bool, JailError>,
{
    for ancestor in dir.ancestors() {
        if !is_operator_controlled(ancestor)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// The sticky bit is deliberately ignored, since it stops removing another
/// user's entries but not creating one's own, so `/tmp` at 0o1777 is rejected.
fn owner_only_writable(uid: u32, mode: u32) -> bool {
    uid == 0 && mode & 0o022 == 0
}

fn first_foreign_entry(dir: &Utf8Path, ours: &[&str]) -> Result<Option<String>, JailError> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        // Missing: creating it is the next step.
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => {
            return Err(JailError::ReadStateDir {
                path: dir.to_owned(),
                source: e,
            });
        },
    };

    for entry in entries {
        let entry = entry.map_err(|e| JailError::ReadStateDir {
            path: dir.to_owned(),
            source: e,
        })?;
        let name = entry.file_name();
        if ours
            .iter()
            .chain(&BENIGN_ENTRIES)
            .any(|known| name == **known)
        {
            continue;
        }
        // Lossy is safe here because the name is only shown to a person, never
        // rebuilt into a path.
        return Ok(Some(name.to_string_lossy().into_owned()));
    }
    Ok(None)
}

/// An `EPERM` from the chown is tolerated because no jail is ever built
/// without root (see [`crate::jail::HostPreparation::ensure`]), and the chmod
/// alone still tightens the tree.
fn make_private(dir: &Utf8Path) -> Result<(), JailError> {
    let opened = fs::OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(dir)
        .map_err(|e| JailError::CreateStateDir {
            path: dir.to_owned(),
            source: e,
        })?;
    match fchown(&opened, Some(0), Some(0)) {
        Ok(()) => {},
        Err(e) if e.raw_os_error() == Some(libc::EPERM) => {},
        Err(e) => {
            return Err(JailError::CreateStateDir {
                path: dir.to_owned(),
                source: e,
            });
        },
    }
    opened
        .set_permissions(fs::Permissions::from_mode(0o700))
        .map_err(|e| JailError::CreateStateDir {
            path: dir.to_owned(),
            source: e,
        })
}

/// Only what a filesystem creates itself belongs here, since anything a person
/// or program put there is what the guard exists to refuse.
const BENIGN_ENTRIES: [&str; 1] = ["lost+found"];

/// Never proof of ownership, only tolerated so the runner's own leftovers, such
/// as the lock after `rm -rf <state_dir>/jail`, cannot disown its root.
const OUR_ROOT_ENTRIES: [&str; 2] = [CHROOT_BASE, LOCK_FILE];

/// Needed because `Drop` never runs on SIGKILL, a crash, or a self-update's
/// `exec`, and jobs run serially, so anything here is stale.
fn sweep_jails(jail_parent: &Utf8Path) -> Result<usize, JailError> {
    sweep_jails_with(
        jail_parent,
        super::reap::reap_jailed_vmm,
        super::cgroup::remove_stale_cgroup,
    )
}

/// Injectable because a test cannot make a VMM survive a real kill, and the
/// real cgroup removal would read the host's `/sys/fs/cgroup`.
fn sweep_jails_with<R, C>(
    jail_parent: &Utf8Path,
    reap: R,
    remove_cgroup: C,
) -> Result<usize, JailError>
where
    R: Fn(&Utf8Path) -> Reaped,
    C: Fn(&VmId) -> Result<(), JailError>,
{
    sweep_jails_removing(jail_parent, reap, remove_cgroup, |jail_dir: &Utf8Path| {
        fs::remove_dir_all(jail_dir)
    })
}

/// The chroot removal is injectable because root can remove any directory a
/// test makes, so a failing removal has to be supplied.
fn sweep_jails_removing<R, C, D>(
    jail_parent: &Utf8Path,
    reap: R,
    remove_cgroup: C,
    remove_chroot: D,
) -> Result<usize, JailError>
where
    R: Fn(&Utf8Path) -> Reaped,
    C: Fn(&VmId) -> Result<(), JailError>,
    D: Fn(&Utf8Path) -> std::io::Result<()>,
{
    // Only absence means nothing to sweep, since "could not look" must never
    // reach the caller as "nothing was there".
    let entries = match fs::read_dir(jail_parent) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(0),
        Err(e) => {
            return Err(JailError::ReadJailParent {
                path: jail_parent.to_owned(),
                source: e,
            });
        },
    };

    // Collected first because directory offsets are not stable under removal,
    // and a skipped neighbor may hold a live VMM.
    let entries: Vec<_> = entries.collect();

    let mut reclaimed = 0;
    // One failing jail must not leave every other stale jail unreaped, so the
    // first failure is kept and the sweep goes on.
    let mut failure = None;

    for entry in entries {
        let outcome = match entry {
            Ok(entry) => match jail_id(jail_parent, &entry) {
                // One reclamation can run for minutes under the lock, and
                // silence that long looks like a wedge.
                Ok(Some(vm_id)) => {
                    let jail_dir = jail_parent.join(vm_id.as_str());
                    super::lock::while_waiting(
                        super::lock::ANNOUNCE_EVERY,
                        || println!("  Still reclaiming stale jail {jail_dir}..."),
                        || reclaim_one(&jail_dir, &vm_id, &reap, &remove_cgroup, &remove_chroot),
                    )
                },
                Ok(None) => continue,
                Err(e) => Reclamation::Failed(e),
            },
            // A listing that breaks off partway leaves jails unexamined, so it
            // fails the sweep rather than passing for one that found nothing.
            Err(e) => {
                eprintln!(
                    "Warning: failed to read an entry under {jail_parent}: {e}. Jails there may not have been examined."
                );
                Reclamation::Failed(JailError::ReadJailParent {
                    path: jail_parent.to_owned(),
                    source: e,
                })
            },
        };

        match outcome {
            Reclamation::Reclaimed => reclaimed += 1,
            Reclamation::LeftBehind => {},
            Reclamation::Failed(e) => {
                if failure.is_none() {
                    failure = Some(e);
                }
            },
        }
    }

    match failure {
        // Only an `Ok` sweep reaches the caller that reports reclamations, so a
        // failed one announces its own.
        Some(e) => {
            if reclaimed > 0 {
                eprintln!(
                    "Reclaimed {reclaimed} stale jail(s) from {jail_parent} before the sweep failed."
                );
            }
            Err(e)
        },
        None => Ok(reclaimed),
    }
}

/// The three variants are the three columns of the table in [`crate::jail`], so
/// a step added to [`reclaim_one`] has to pick one.
enum Reclamation {
    /// The cgroup and the chroot are both gone.
    Reclaimed,
    /// Costs disk rather than fidelity, so the job may run and the next sweep
    /// tries again.
    LeftBehind,
    /// The host cannot be trusted to measure until this is resolved.
    Failed(JailError),
}

/// An entry whose kind cannot be read is an error, not `Ok(None)`, since it may
/// be a jail holding a live VMM.
fn jail_id(jail_parent: &Utf8Path, entry: &fs::DirEntry) -> Result<Option<VmId>, JailError> {
    match entry.file_type() {
        Ok(file_type) if file_type.is_dir() => {},
        Ok(_) => return Ok(None),
        Err(e) => {
            eprintln!(
                "Warning: cannot tell what {} under {jail_parent} is: {e}. If it is a jail, it was not examined.",
                entry.file_name().display()
            );
            return Err(JailError::ReadJailParent {
                path: jail_parent.to_owned(),
                source: e,
            });
        },
    }

    // Skipped rather than lossily converted, since a lossy name rebuilds into a
    // path that does not exist, and the reap would report a live VMM clear.
    let file_name = entry.file_name();
    let Some(name) = file_name.to_str() else {
        eprintln!(
            "Warning: skipping an entry with a non-UTF-8 name under {jail_parent}; the runner did not create it"
        );
        return Ok(None);
    };
    // A name this runner could not have minted is skipped too, since the id is
    // joined into both a chroot path and a cgroup path.
    Ok(VmId::from_chroot_name(name.to_owned()))
}

fn reclaim_one<R, C, D>(
    jail_dir: &Utf8Path,
    vm_id: &VmId,
    reap: &R,
    remove_cgroup: &C,
    remove_chroot: &D,
) -> Reclamation
where
    R: Fn(&Utf8Path) -> Reaped,
    C: Fn(&VmId) -> Result<(), JailError>,
    D: Fn(&Utf8Path) -> std::io::Result<()>,
{
    // Fatal to the job because a stray VMM contends the benchmark cores and,
    // with no exclusive cpuset on these cgroups, nothing downstream notices.
    match reap(&jail_dir.join(JAIL_ROOT)) {
        Reaped::Clear => {},
        Reaped::StillRunning { pid } => {
            eprintln!(
                "Warning: leaving stale jail {jail_dir} in place because VMM pid {pid} is still running on the benchmark cores."
            );
            return Reclamation::Failed(JailError::JailStillRunning {
                path: jail_dir.to_owned(),
                pid,
            });
        },
        Reaped::Unexaminable => {
            eprintln!(
                "Warning: leaving stale jail {jail_dir} in place because whether a VMM is still running in it could not be determined."
            );
            return Reclamation::Failed(JailError::JailUnexaminable {
                path: jail_dir.to_owned(),
            });
        },
    }

    // The chroot outlives the cgroup because its name is the only handle a
    // later sweep has for finding that cgroup again.
    if let Err(e) = remove_cgroup(vm_id) {
        eprintln!(
            "Warning: leaving stale jail {jail_dir} in place because its cgroup could not be removed: {e}"
        );
        return Reclamation::Failed(e);
    }

    match remove_chroot(jail_dir) {
        Ok(()) => Reclamation::Reclaimed,
        Err(e) => {
            eprintln!(
                "Warning: failed to sweep stale jail {jail_dir}: {e}. It holds a VMM binary and a full guest rootfs; the next job will try again."
            );
            Reclamation::LeftBehind
        },
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::symlink;
    use std::process::Command;

    use super::*;

    fn temp_root() -> (tempfile::TempDir, Utf8PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        (dir, root)
    }

    /// Made directly under `/`, the only root-only-writable ancestry short of
    /// special mounts, so an elevated test run really writes to the filesystem
    /// root.
    struct RootOnlyBase(Utf8PathBuf);

    impl Drop for RootOnlyBase {
        fn drop(&mut self) {
            drop(fs::remove_dir_all(&self.0));
        }
    }

    fn root_only_base() -> RootOnlyBase {
        use std::sync::atomic::{AtomicU32, Ordering};
        static COUNTER: AtomicU32 = AtomicU32::new(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let base = Utf8PathBuf::from(format!(
            "/bencher-runner-test-{}-{unique}",
            std::process::id()
        ));
        drop(fs::remove_dir_all(&base));
        fs::create_dir(&base).unwrap();
        fs::set_permissions(&base, fs::Permissions::from_mode(0o755)).unwrap();
        RootOnlyBase(base)
    }

    /// The tempdir is canonicalized first, or an ancestor the platform symlinks
    /// would trip showing (2) in every test aimed at (1) or (3).
    fn linked_root() -> (tempfile::TempDir, Utf8PathBuf, Utf8PathBuf, Utf8PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf())
            .unwrap()
            .canonicalize_utf8()
            .unwrap();
        let parent = root.join("operator");
        fs::create_dir(&parent).unwrap();
        let target = root.join("target");
        fs::create_dir(&target).unwrap();
        let link = parent.join("state");
        symlink(&target, &link).unwrap();
        (dir, link, parent, target)
    }

    #[test]
    fn create_is_idempotent_and_private() {
        let (_dir, root) = temp_root();
        let state = StateDir::new(root.join("state")).unwrap();

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
        let state = StateDir::new(root.join("state")).unwrap();
        fs::create_dir_all(state.path()).unwrap();
        fs::set_permissions(state.path(), fs::Permissions::from_mode(0o755)).unwrap();

        state.create().unwrap();

        let mode = fs::metadata(state.path()).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
    }

    #[test]
    fn create_takes_ownership_of_a_directory_it_was_handed() {
        // A handed-over directory's owner could chmod the 0700 back and plant
        // links, so it must be chowned to root.
        use std::os::unix::fs::chown;
        if crate::jail::current_euid() != 0 {
            eprintln!(
                "skipped create_takes_ownership_of_a_directory_it_was_handed: the chown needs root"
            );
            return;
        }
        let (_dir, root) = temp_root();
        let state = StateDir::new(root.join("state")).unwrap();
        fs::create_dir_all(state.jail_parent()).unwrap();
        for dir in [state.path(), &state.chroot_base(), &state.jail_parent()] {
            chown(dir, Some(1000), Some(1000)).unwrap();
        }

        state.create().unwrap();

        for dir in [state.path(), &state.chroot_base(), &state.jail_parent()] {
            let metadata = fs::metadata(dir).unwrap();
            assert_eq!(metadata.uid(), 0, "{dir} must be taken for root");
            assert_eq!(metadata.gid(), 0, "{dir} must be taken for root");
            assert_eq!(metadata.permissions().mode() & 0o777, 0o700);
        }
    }

    #[test]
    fn a_populated_foreign_directory_is_refused() {
        // `--state-dir /var/lib` must not chmod a system directory to 0700.
        let (_dir, root) = temp_root();
        let foreign = root.join("var-lib");
        fs::create_dir_all(foreign.join("dpkg")).unwrap();
        fs::create_dir_all(foreign.join("systemd")).unwrap();

        StateDir::new(foreign.clone())
            .unwrap()
            .create()
            .unwrap_err();

        let mode = fs::metadata(&foreign).unwrap().permissions().mode();
        assert_ne!(mode & 0o777, 0o700, "a refused root must not be chmodded");
    }

    #[test]
    fn a_root_that_cannot_be_read_is_not_assumed_to_be_ours() {
        // A failed read, such as `ENOTDIR` from a file in the way, must not pass
        // for an empty directory.
        let (_dir, root) = temp_root();
        let not_a_dir = root.join("state");
        fs::write(&not_a_dir, b"operator note").unwrap();

        let err = StateDir::new(not_a_dir).unwrap().create().unwrap_err();

        assert!(
            matches!(err, JailError::ReadStateDir { .. }),
            "a read that failed is reported, not swallowed: {err}"
        );
    }

    #[test]
    fn a_dedicated_filesystem_is_ours_to_take() {
        // A fresh ext4 volume's `lost+found` must not refuse the recommended
        // dedicated-filesystem setup.
        let (_dir, root) = temp_root();
        let volume = root.join("volume");
        fs::create_dir_all(volume.join("lost+found")).unwrap();

        StateDir::new(volume.clone()).unwrap().create().unwrap();

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

        StateDir::new(foreign.clone())
            .unwrap()
            .create()
            .unwrap_err();

        let mode = fs::metadata(&foreign).unwrap().permissions().mode();
        assert_ne!(mode & 0o777, 0o700, "a refused root must not be chmodded");
    }

    #[test]
    fn an_empty_directory_is_ours_to_take() {
        let (_dir, root) = temp_root();
        let empty = root.join("empty");
        fs::create_dir_all(&empty).unwrap();

        StateDir::new(empty).unwrap().create().unwrap();
    }

    #[test]
    fn a_directory_the_runner_already_used_is_ours() {
        let (_dir, root) = temp_root();
        let state = StateDir::new(root.join("state")).unwrap();
        state.create().unwrap();
        // Something the host put there afterwards does not disown it.
        fs::write(state.path().join("notes.txt"), b"operator note").unwrap();

        state.create().unwrap();
    }

    #[test]
    fn a_directory_named_like_ours_is_not_ours() {
        // The runner's entry names are tolerated only in a root holding nothing
        // else, never taken as proof of ownership.
        let (_dir, root) = temp_root();
        let foreign = root.join("var-lib");
        fs::create_dir_all(foreign.join("jail")).unwrap();
        fs::create_dir_all(foreign.join("dpkg")).unwrap();
        fs::write(foreign.join(".lock"), b"").unwrap();

        let err = StateDir::new(foreign.clone())
            .unwrap()
            .create()
            .unwrap_err();

        assert!(
            err.to_string().contains("dpkg"),
            "an operator refused over one entry has to be told which: {err}"
        );
        let mode = fs::metadata(&foreign).unwrap().permissions().mode();
        assert_ne!(mode & 0o777, 0o700, "a refused root must not be chmodded");
    }

    #[test]
    fn a_foreign_jail_directory_is_not_our_tree() {
        // Somebody else's entries under a `jail` of that name must be refused
        // and left untouched.
        let (_dir, root) = temp_root();
        let foreign = root.join("var-lib");
        fs::create_dir_all(foreign.join("jail").join("mail-server")).unwrap();

        let err = StateDir::new(foreign.clone())
            .unwrap()
            .create()
            .unwrap_err();

        assert!(
            err.to_string().contains("mail-server"),
            "the refusal names what tripped it: {err}"
        );
        let mode = fs::metadata(foreign.join("jail"))
            .unwrap()
            .permissions()
            .mode();
        assert_ne!(mode & 0o777, 0o700, "a refused tree must not be chmodded");
        assert!(foreign.join("jail").join("mail-server").is_dir());
    }

    #[test]
    fn the_tree_is_what_proves_the_directory_is_ours() {
        // `jail/firecracker` proves the root is the runner's, whatever the
        // operator added beside it.
        let (_dir, root) = temp_root();
        let state = root.join("state");
        fs::create_dir_all(state.join("jail").join("firecracker")).unwrap();
        fs::write(state.join("notes.txt"), b"operator note").unwrap();

        StateDir::new(state.clone()).unwrap().create().unwrap();

        let mode = fs::metadata(&state).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700);
    }

    #[test]
    fn a_half_built_tree_is_ours() {
        // A runner killed between two levels of `create` must not latch its
        // state directory as somebody else's.
        let (_dir, root) = temp_root();
        let state = StateDir::new(root.join("state")).unwrap();
        fs::create_dir_all(state.chroot_base()).unwrap();

        state.create().unwrap();

        for dir in [state.path(), &state.chroot_base(), &state.jail_parent()] {
            let mode = fs::metadata(dir).unwrap().permissions().mode();
            assert_eq!(mode & 0o777, 0o700, "{dir} should be private");
        }
    }

    #[test]
    fn a_root_holding_only_the_lock_is_ours() {
        // The real lock left by `rm -rf <state_dir>/jail` must not stop a
        // rebuild, which also pins the guard to the lock module's file name.
        let (_dir, root) = temp_root();
        let state = StateDir::new(root.join("state")).unwrap();
        state.create().unwrap();
        drop(JailLock::acquire(state.path()).unwrap());
        fs::remove_dir_all(state.chroot_base()).unwrap();

        state.create().unwrap();

        assert!(state.jail_parent().is_dir(), "the tree is rebuilt");
    }

    #[test]
    fn a_symlinked_component_is_refused() {
        // A link at any component, under a world-writable base so the result
        // holds whatever the euid, must be refused before the chmod or the sweep
        // reaches its target.
        for component in ["state", "state/jail", "state/jail/firecracker"] {
            let (_dir, root) = temp_root();
            fs::set_permissions(&root, fs::Permissions::from_mode(0o777)).unwrap();
            let victim = root.join("victim");
            fs::create_dir_all(victim.join("someone-elses-data")).unwrap();

            let planted = root.join(component);
            fs::create_dir_all(planted.parent().unwrap()).unwrap();
            symlink(&victim, &planted).unwrap();

            let err = StateDir::new(root.join("state"))
                .unwrap()
                .create()
                .unwrap_err();

            assert!(
                matches!(err, JailError::SymlinkedStateDir { .. }),
                "{component}: a link is refused, not resolved: {err}"
            );
            assert!(
                err.to_string().contains(planted.as_str()),
                "{component}: the refusal names the component: {err}"
            );
            let mode = fs::metadata(&victim).unwrap().permissions().mode();
            assert_ne!(
                mode & 0o777,
                0o700,
                "{component}: the target of a link must not be chmodded"
            );
            assert!(
                victim.join("someone-elses-data").is_dir(),
                "{component}: the target of a link must not be swept"
            );
        }
    }

    #[test]
    fn owner_only_writable_gates_on_uid_and_write_bits() {
        // Only a root-owned directory closed to group and other writes passes,
        // whatever its file-type or sticky bits.
        assert!(owner_only_writable(0, 0o755));
        assert!(owner_only_writable(0, 0o700));
        assert!(owner_only_writable(0, 0o711));
        assert!(owner_only_writable(0, 0o040_755));
        assert!(!owner_only_writable(0, 0o775), "group-writable");
        assert!(!owner_only_writable(0, 0o757), "other-writable");
        assert!(!owner_only_writable(0, 0o777), "group- and other-writable");
        assert!(
            !owner_only_writable(0, 0o1777),
            "sticky does not launder /tmp"
        );
        assert!(!owner_only_writable(1000, 0o755));
        assert!(!owner_only_writable(1000, 0o700));
    }

    #[test]
    fn ancestry_is_operator_controlled_walks_the_whole_path() {
        // A loose directory anywhere on the path must fail the walk, while `/`
        // itself passes.
        assert!(ancestry_is_operator_controlled(Utf8Path::new("/"), &operator_controlled).unwrap());
        let (_dir, root) = temp_root();
        let open = root.join("open");
        fs::create_dir(&open).unwrap();
        fs::set_permissions(&open, fs::Permissions::from_mode(0o777)).unwrap();
        let leaf = open.join("leaf");
        fs::create_dir(&leaf).unwrap();
        assert!(
            !ancestry_is_operator_controlled(&leaf, &operator_controlled).unwrap(),
            "a writable directory on the path is refused"
        );
    }

    #[test]
    fn a_link_passing_every_showing_resolves_to_its_target() {
        // A link passing all three showings must resolve to its target rather
        // than stay the link.
        let (_dir, link, _parent, target) = linked_root();

        let resolved = resolve_symlinked_root_with(link, |_: &Utf8Path| Ok(true)).unwrap();

        assert_eq!(resolved, target, "the target becomes the root");
    }

    #[test]
    fn a_parent_the_judgment_refuses_leaves_the_link_unresolved() {
        // Showings (2) and (3) both hold here, so a resolution would mean (1)
        // was not required.
        let (_dir, link, parent, _target) = linked_root();

        let unresolved =
            resolve_symlinked_root_with(link.clone(), |dir: &Utf8Path| Ok(dir != parent.as_path()))
                .unwrap();

        assert_eq!(unresolved, link, "refused by returning the root as given");
    }

    #[test]
    fn a_second_hop_leaves_the_link_unresolved_whatever_the_judgment() {
        // A second hop must be refused by showing (2) even when the judgment
        // accepts every directory.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf())
            .unwrap()
            .canonicalize_utf8()
            .unwrap();
        let parent = root.join("operator");
        fs::create_dir(&parent).unwrap();
        let target = root.join("target");
        fs::create_dir(&target).unwrap();
        let mid = root.join("mid");
        symlink(&target, &mid).unwrap();
        let link = parent.join("state");
        symlink(&mid, &link).unwrap();

        let unresolved =
            resolve_symlinked_root_with(link.clone(), |_: &Utf8Path| Ok(true)).unwrap();

        assert_eq!(unresolved, link, "refused by returning the root as given");
    }

    #[test]
    fn a_relative_target_leaves_the_link_unresolved_whatever_the_judgment() {
        // A relative stored target must be refused by showing (2) even though it
        // lands on the right directory.
        let (_dir, link, _parent, target) = linked_root();
        fs::remove_file(&link).unwrap();
        symlink(Utf8Path::new("../target"), &link).unwrap();
        assert_eq!(
            link.canonicalize_utf8().unwrap(),
            target,
            "the chain still lands on the target, so only (2) can refuse it"
        );

        let unresolved =
            resolve_symlinked_root_with(link.clone(), |_: &Utf8Path| Ok(true)).unwrap();

        assert_eq!(unresolved, link, "refused by returning the root as given");
    }

    #[test]
    fn a_target_the_judgment_refuses_leaves_the_link_unresolved() {
        // Showing (3) must judge the resolved target itself, not only its
        // ancestors.
        let (_dir, link, _parent, target) = linked_root();

        let unresolved =
            resolve_symlinked_root_with(link.clone(), |dir: &Utf8Path| Ok(dir != target.as_path()))
                .unwrap();

        assert_eq!(unresolved, link, "refused by returning the root as given");
    }

    #[test]
    fn the_ancestry_walk_reaches_the_filesystem_root() {
        // A judgment refusing only `/` proves showing (3) walks the whole
        // ancestry.
        let (_dir, link, _parent, _target) = linked_root();

        let unresolved = resolve_symlinked_root_with(link.clone(), |dir: &Utf8Path| {
            Ok(dir != Utf8Path::new("/"))
        })
        .unwrap();

        assert_eq!(unresolved, link, "refused by returning the root as given");
    }

    #[test]
    fn a_real_directory_root_is_used_verbatim() {
        // Only a symlink at the root is resolved, so a real directory's
        // ancestor symlinks, the operator's own layout, must survive.
        let (_dir, root) = temp_root();
        let real = root.join("real");
        fs::create_dir(&real).unwrap();

        let state = StateDir::new(real.clone()).unwrap();
        assert_eq!(state.path(), real, "a real directory root is left as given");
    }

    #[test]
    fn a_nonexistent_root_is_left_as_given() {
        // The first-run case: a missing root must come back as given for
        // `create` to make.
        let (_dir, root) = temp_root();
        let missing = root.join("not-here-yet");

        let state = StateDir::new(missing.clone()).unwrap();
        assert_eq!(state.path(), missing);
        state.create().unwrap();
        assert!(missing.join("jail").join("firecracker").is_dir());
    }

    #[test]
    fn a_symlinked_root_under_a_writable_parent_is_refused() {
        // A parent others can write, even with the sticky bit `/tmp` carries,
        // must leave the link refused and its target neither chmodded nor swept.
        let (_dir, root) = temp_root();
        let parent = root.join("open");
        fs::create_dir(&parent).unwrap();
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o1777)).unwrap();
        let victim = root.join("victim");
        fs::create_dir_all(victim.join("someone-elses-data")).unwrap();
        let link = parent.join("state");
        symlink(&victim, &link).unwrap();

        let err = StateDir::new(link.clone()).unwrap().create().unwrap_err();

        assert!(
            matches!(err, JailError::SymlinkedStateDir { .. }),
            "a link under a writable parent is refused, not resolved: {err}"
        );
        assert!(
            err.to_string().contains(link.as_str()),
            "the refusal names the link: {err}"
        );
        let mode = fs::metadata(&victim).unwrap().permissions().mode();
        assert_ne!(mode & 0o777, 0o700, "the link target must not be chmodded");
        assert!(
            victim.join("someone-elses-data").is_dir(),
            "the link target must not be swept"
        );
    }

    #[test]
    fn a_symlink_resolving_through_a_writable_ancestor_is_refused() {
        // A link whose second hop an attacker planted in a world-writable
        // intermediate must not be followed to the victim.
        let (_dir, root) = temp_root();
        let intermediate = root.join("intermediate");
        fs::create_dir(&intermediate).unwrap();
        fs::set_permissions(&intermediate, fs::Permissions::from_mode(0o777)).unwrap();
        let victim = root.join("victim");
        fs::create_dir_all(victim.join("someone-elses-data")).unwrap();
        let second = intermediate.join("runner");
        symlink(&victim, &second).unwrap();
        let link = root.join("state");
        symlink(&second, &link).unwrap();

        let err = StateDir::new(link).unwrap().create().unwrap_err();

        assert!(
            matches!(err, JailError::SymlinkedStateDir { .. }),
            "a chain through a writable ancestor is refused, not followed: {err}"
        );
        let mode = fs::metadata(&victim).unwrap().permissions().mode();
        assert_ne!(mode & 0o777, 0o700, "the victim must not be chmodded");
        assert!(
            victim.join("someone-elses-data").is_dir(),
            "the victim must not be swept"
        );
    }

    #[test]
    fn an_operator_controlled_symlinked_root_is_resolved() {
        // The recommended setup, a root-owned link to a root-owned dedicated
        // directory, must resolve and build under the target.
        if crate::jail::current_euid() != 0 {
            eprintln!(
                "skipped an_operator_controlled_symlinked_root_is_resolved: a root-only-writable ancestry needs root to build"
            );
            return;
        }
        let base = root_only_base();
        let base = &base.0;
        let parent = base.join("operator");
        fs::create_dir(&parent).unwrap();
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o755)).unwrap();
        let target = base.join("target");
        fs::create_dir(&target).unwrap();

        let link = parent.join("state");
        symlink(&target, &link).unwrap();

        let state = StateDir::new(link).unwrap();
        let resolved = target.canonicalize_utf8().unwrap();
        assert_eq!(state.path(), resolved, "the target becomes the root");

        state.create().unwrap();
        let mode = fs::metadata(&resolved).unwrap().permissions().mode();
        assert_eq!(mode & 0o777, 0o700, "the resolved target is tightened");
        assert!(
            resolved.join("jail").join("firecracker").is_dir(),
            "the tree is built under the resolved target"
        );
    }

    #[test]
    fn an_interior_symlink_is_refused_even_under_a_resolved_root() {
        // A resolved root must not lower the bar for an interior link.
        if crate::jail::current_euid() != 0 {
            eprintln!(
                "skipped an_interior_symlink_is_refused_even_under_a_resolved_root: a root-only-writable ancestry needs root to build"
            );
            return;
        }
        let base = root_only_base();
        let base = &base.0;
        let parent = base.join("operator");
        fs::create_dir(&parent).unwrap();
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o755)).unwrap();
        let target = base.join("target");
        fs::create_dir(&target).unwrap();

        let victim = base.join("victim");
        fs::create_dir_all(victim.join("someone-elses-data")).unwrap();
        symlink(&victim, target.join("jail")).unwrap();

        let link = parent.join("state");
        symlink(&target, &link).unwrap();

        let err = StateDir::new(link).unwrap().create().unwrap_err();

        assert!(
            matches!(err, JailError::SymlinkedStateDir { .. }),
            "an interior link is refused even under a resolved root: {err}"
        );
        let mode = fs::metadata(&victim).unwrap().permissions().mode();
        assert_ne!(mode & 0o777, 0o700, "the link target must not be chmodded");
        assert!(
            victim.join("someone-elses-data").is_dir(),
            "the link target must not be swept"
        );
    }

    struct Tmpfs(Utf8PathBuf);

    impl Tmpfs {
        fn mount(at: &Utf8Path, options: &str) -> Self {
            let status = Command::new("mount")
                .args(["-t", "tmpfs", "-o", options, "tmpfs", at.as_str()])
                .status()
                .unwrap();
            assert!(status.success(), "mount -o {options} {at} failed");
            Self(at.to_owned())
        }
    }

    impl Drop for Tmpfs {
        fn drop(&mut self) {
            drop(Command::new("umount").arg(self.0.as_str()).status());
        }
    }

    #[test]
    fn a_filesystem_the_jail_cannot_run_from_is_refused_by_its_mount_option() {
        // A `nodev` or `noexec` state filesystem would otherwise fail the job at
        // boot with KVM blaming its ACL.
        if crate::jail::current_euid() != 0 {
            eprintln!(
                "skipped a_filesystem_the_jail_cannot_run_from_is_refused_by_its_mount_option: mounting needs root"
            );
            return;
        }
        for option in ["nodev", "noexec"] {
            let (_dir, root) = temp_root();
            let _mount = Tmpfs::mount(&root, &format!("{option},size=1m"));
            let state = StateDir::new(root.join("state")).unwrap();
            state.create().unwrap();

            let err = state.refuse_unusable_mount().unwrap_err();

            assert!(
                matches!(err, JailError::StateDirMountOption { option: refused, .. } if refused == option),
                "{option} must be refused by name: {err}"
            );
        }

        let (_dir, root) = temp_root();
        let _mount = Tmpfs::mount(&root, "size=1m");
        let state = StateDir::new(root.join("state")).unwrap();
        state.create().unwrap();
        state.refuse_unusable_mount().unwrap();
    }

    #[test]
    fn a_relative_state_directory_is_refused() {
        // The jailer resolves a relative path against its own working
        // directory, so it would name a different place than the runner's.
        StateDir::new(Utf8PathBuf::from("bencher-runner")).unwrap_err();
        StateDir::new(Utf8PathBuf::from("./bencher-runner")).unwrap_err();
        StateDir::new(Utf8PathBuf::from("/var/lib/bencher-runner")).unwrap();
    }

    #[test]
    fn sweep_removes_stale_jails() {
        let (_dir, root) = temp_root();
        let state = StateDir::new(root.join("state")).unwrap();
        state.create().unwrap();

        fs::create_dir_all(state.jail_root(&VmId::from_chroot_name("one".to_owned()).unwrap()))
            .unwrap();
        fs::write(
            state
                .jail_root(&VmId::from_chroot_name("one".to_owned()).unwrap())
                .join("rootfs.ext4"),
            b"stale",
        )
        .unwrap();
        fs::create_dir_all(state.jail_dir(&VmId::from_chroot_name("two".to_owned()).unwrap()))
            .unwrap();

        assert_eq!(
            sweep_jails_with(&state.jail_parent(), |_j| Reaped::Clear, |_v| Ok(())).unwrap(),
            2
        );
        assert!(
            !state
                .jail_dir(&VmId::from_chroot_name("one".to_owned()).unwrap())
                .exists()
        );
        assert!(
            !state
                .jail_dir(&VmId::from_chroot_name("two".to_owned()).unwrap())
                .exists()
        );
        assert!(state.jail_parent().exists());
    }

    #[test]
    fn one_sweep_reclaims_every_jail_however_many_there_are() {
        // Enough jails to span several kernel batches, so a listing walked live
        // across the removals could skip one.
        let (_dir, root) = temp_root();
        let state = StateDir::new(root.join("state")).unwrap();
        state.create().unwrap();
        let ids: Vec<VmId> = std::iter::repeat_with(VmId::new).take(1000).collect();
        for id in &ids {
            fs::create_dir_all(state.jail_root(id)).unwrap();
        }

        let reclaimed =
            sweep_jails_with(&state.jail_parent(), |_j| Reaped::Clear, |_v| Ok(())).unwrap();

        assert_eq!(reclaimed, ids.len());
        for id in &ids {
            assert!(
                !state.jail_dir(id).exists(),
                "{id} must be reclaimed by the same sweep that listed it"
            );
        }
    }

    #[test]
    fn sweep_leaves_unrelated_entries_alone() {
        let (_dir, root) = temp_root();
        let state = StateDir::new(root.join("state")).unwrap();
        state.create().unwrap();

        let note = state.jail_parent().join("NOTES.txt");
        fs::write(&note, b"not a jail").unwrap();
        fs::create_dir_all(state.jail_dir(&VmId::from_chroot_name("stale".to_owned()).unwrap()))
            .unwrap();

        assert_eq!(
            sweep_jails_with(&state.jail_parent(), |_j| Reaped::Clear, |_v| Ok(())).unwrap(),
            1
        );
        assert!(
            !state
                .jail_dir(&VmId::from_chroot_name("stale".to_owned()).unwrap())
                .exists()
        );
        assert!(note.exists(), "non-directory entries are not the sweep's");
    }

    #[test]
    fn a_jail_whose_vmm_survives_is_left_in_place() {
        // Removing the tree would not stop the VMM, and it would destroy the
        // only handle for identifying that process on a later sweep.
        let (_dir, root) = temp_root();
        let state = StateDir::new(root.join("state")).unwrap();
        state.create().unwrap();
        let live = VmId::from_chroot_name("live".to_owned()).unwrap();
        let dead = VmId::from_chroot_name("dead".to_owned()).unwrap();
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
        // job, not once.
        let (_dir, root) = temp_root();
        let state = StateDir::new(root.join("state")).unwrap();
        state.create().unwrap();
        let live = VmId::from_chroot_name("live".to_owned()).unwrap();
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
        // The chroot is the only handle for its cgroup, and one stuck cgroup
        // must not abandon the rest of the sweep.
        let (_dir, root) = temp_root();
        let state = StateDir::new(root.join("state")).unwrap();
        state.create().unwrap();
        let stuck = VmId::from_chroot_name("stuck".to_owned()).unwrap();
        let clear = VmId::from_chroot_name("clear".to_owned()).unwrap();
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
    fn a_chroot_that_will_not_go_away_does_not_fail_the_job() {
        // A chroot that will not go away costs disk, not fidelity, so it must
        // not fail the job.
        let (_dir, root) = temp_root();
        let state = StateDir::new(root.join("state")).unwrap();
        state.create().unwrap();
        let stuck = VmId::from_chroot_name("stuck".to_owned()).unwrap();
        fs::create_dir_all(state.jail_root(&stuck)).unwrap();

        let reclaimed = sweep_jails_removing(
            &state.jail_parent(),
            |_jail_root| Reaped::Clear,
            |_vm_id| Ok(()),
            |_jail_dir| Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied)),
        )
        .unwrap();

        assert_eq!(reclaimed, 0);
    }

    #[test]
    fn a_jail_that_could_not_be_examined_is_left_in_place() {
        // An unexaminable jail must get the same answer as one known to hold a
        // live VMM.
        let (_dir, root) = temp_root();
        let state = StateDir::new(root.join("state")).unwrap();
        state.create().unwrap();
        let unknown = VmId::from_chroot_name("unknown".to_owned()).unwrap();
        fs::create_dir_all(state.jail_root(&unknown)).unwrap();

        let err = sweep_jails_with(
            &state.jail_parent(),
            |_jail_root| Reaped::Unexaminable,
            |_vm_id| Ok(()),
        )
        .unwrap_err();

        assert!(
            state.jail_dir(&unknown).exists(),
            "not ours to delete blind"
        );
        assert!(
            matches!(err, JailError::JailUnexaminable { .. }),
            "a jail that could not be checked fails the job: {err}"
        );
    }

    #[test]
    fn a_cleared_jail_is_still_swept() {
        let (_dir, root) = temp_root();
        let state = StateDir::new(root.join("state")).unwrap();
        state.create().unwrap();
        fs::create_dir_all(state.jail_root(&VmId::from_chroot_name("one".to_owned()).unwrap()))
            .unwrap();

        let reclaimed = sweep_jails_with(
            &state.jail_parent(),
            |_jail_root| Reaped::Clear,
            |_vm_id| Ok(()),
        )
        .unwrap();

        assert_eq!(reclaimed, 1);
    }

    #[test]
    fn a_jail_parent_that_cannot_be_read_is_not_an_empty_one() {
        // A read the sweep could not perform must not reach the caller as a
        // clean host.
        let (_dir, root) = temp_root();
        let not_a_dir = root.join("firecracker");
        fs::write(&not_a_dir, b"in the way").unwrap();

        let err = sweep_jails_with(&not_a_dir, |_j| Reaped::Clear, |_v| Ok(())).unwrap_err();

        assert!(
            matches!(err, JailError::ReadJailParent { .. }),
            "a read that failed is reported, not counted as zero jails: {err}"
        );
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
