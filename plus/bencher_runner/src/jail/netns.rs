//! The empty network namespace the jailed VMM joins.
//!
//! The guest has no network device and never will; this namespace is for the
//! VMM process itself. A compromised Firecracker with host network access can
//! exfiltrate, and an empty namespace removes that reach. The vsock transport
//! is unaffected: its host side is filesystem-scoped Unix domain sockets, not
//! network-namespace-scoped.

use std::fs;
use std::os::unix::fs::MetadataExt as _;

use camino::{Utf8Path, Utf8PathBuf};
use nix::mount::{MntFlags, MsFlags, mount, umount2};
use nix::sched::{CloneFlags, unshare};

use crate::error::JailError;

/// Directory holding named network namespace handles.
///
/// This follows the `ip netns` convention (iproute2's `NETNS_RUN_DIR`), so the
/// runner's namespace shows up in `ip netns list` for operators. `/run` is a
/// tmpfs, so handles do not survive a reboot, which is exactly right for a
/// handle onto a kernel object.
const NETNS_DIR: &str = "/run/netns";

/// Name of the empty network namespace the jailed VMM joins.
const NETNS_NAME: &str = "bencher-jail";

/// The runner's own network namespace, used as the reference for deciding
/// whether a handle is a live namespace distinct from the host's.
const SELF_NETNS: &str = "/proc/self/ns/net";

/// How many stacked mounts to unwind at the handle before giving up.
///
/// Bounded rather than unbounded: a path that keeps reporting a successful
/// unmount forever is a kernel fault, and the failed unlink that follows
/// reports the real state either way.
const MAX_STACKED_MOUNTS: usize = 32;

/// The calling *thread's* network namespace.
///
/// `/proc/self` resolves through the thread group leader, so it must not be
/// used from the namespace-creating thread: it would name the runner's own
/// namespace and pin the host network instead of the new one.
const THREAD_NETNS: &str = "/proc/thread-self/ns/net";

/// The path of the network namespace handle.
#[must_use]
pub fn handle_path() -> Utf8PathBuf {
    Utf8Path::new(NETNS_DIR).join(NETNS_NAME)
}

/// Ensure the empty network namespace exists, returning its handle path.
///
/// Idempotent: a handle that is already a live namespace distinct from the
/// runner's own is reused. Anything else at the path (a leftover placeholder
/// file, or a handle whose mount is gone) is cleared and recreated.
pub fn ensure() -> Result<Utf8PathBuf, JailError> {
    let handle = handle_path();

    fs::create_dir_all(NETNS_DIR).map_err(|e| JailError::NetnsDir {
        path: Utf8PathBuf::from(NETNS_DIR),
        source: e,
    })?;

    if is_live_netns(&handle) {
        return Ok(handle);
    }

    clear(&handle)?;

    // The bind mount needs a regular file to land on.
    fs::File::create(&handle).map_err(|e| JailError::NetnsHandle {
        path: handle.clone(),
        source: e,
    })?;

    if let Err(e) = create(&handle) {
        drop(fs::remove_file(&handle));
        return Err(e);
    }

    Ok(handle)
}

/// Remove whatever is at the handle path, mounts included.
///
/// Bind mounting over a file does not report `EBUSY`, so mounts stack: a
/// handle that has been recreated more than once carries more than one. A
/// single detach unwinds only the top mount, the unlink of the still-mounted
/// path then fails with `EBUSY`, and `File::create` on the surviving nsfs
/// mount fails with `EPERM` even as root. Unwinding one mount at a time and
/// reporting a failed unlink is what keeps a stacked handle from wedging the
/// host: without it, `ensure` fails permanently and every sandboxed job with
/// it, until an operator loops `umount` by hand.
fn clear(handle: &Utf8Path) -> Result<(), JailError> {
    for _ in 0..MAX_STACKED_MOUNTS {
        if umount2(handle.as_std_path(), MntFlags::MNT_DETACH).is_err() {
            break;
        }
    }

    match fs::remove_file(handle) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(JailError::NetnsHandle {
            path: handle.to_owned(),
            source: e,
        }),
    }
}

/// Whether `handle` is a live network namespace other than the runner's own.
///
/// Every namespace inode lives on the single kernel `nsfs`, so sharing a
/// device with a known namespace proves the handle is one, and a differing
/// inode proves it is not the host namespace the runner itself is in. A
/// leftover placeholder file sits on the `/run` tmpfs and fails the device
/// check.
fn is_live_netns(handle: &Utf8Path) -> bool {
    let (Ok(own), Ok(candidate)) = (fs::metadata(SELF_NETNS), fs::metadata(handle)) else {
        return false;
    };
    own.dev() == candidate.dev() && own.ino() != candidate.ino()
}

/// Create the namespace and bind its handle into place.
///
/// The namespace is unshared on a dedicated thread rather than in the runner
/// itself. Network namespaces are per-task, so only this thread moves and the
/// runner stays on the host network; the bind mount then holds a reference
/// that keeps the namespace alive once the thread exits. The thread is not
/// reused for anything else, precisely because it never returns to the host
/// namespace.
fn create(handle: &Utf8Path) -> Result<(), JailError> {
    let target = handle.to_owned();
    std::thread::spawn(move || -> Result<(), JailError> {
        unshare(CloneFlags::CLONE_NEWNET).map_err(JailError::Unshare)?;
        mount(
            Some(THREAD_NETNS),
            target.as_std_path(),
            None::<&str>,
            MsFlags::MS_BIND,
            None::<&str>,
        )
        .map_err(|e| JailError::BindNetns {
            path: target.clone(),
            source: e,
        })
    })
    .join()
    .map_err(|_panic| JailError::NetnsThread)?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn handle_follows_the_ip_netns_convention() {
        assert_eq!(handle_path(), "/run/netns/bencher-jail");
    }

    #[test]
    fn a_plain_file_is_not_a_live_netns() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let path = root.join("net");
        fs::write(&path, b"").unwrap();

        assert!(!is_live_netns(&path));
    }

    #[test]
    fn a_missing_handle_is_not_a_live_netns() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

        assert!(!is_live_netns(&root.join("absent")));
    }

    #[test]
    fn clear_removes_a_plain_handle() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let path = root.join("net");
        fs::write(&path, b"").unwrap();

        clear(&path).unwrap();

        assert!(!path.exists());
    }

    #[test]
    fn clear_is_idempotent_on_a_missing_handle() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

        clear(&root.join("absent")).unwrap();
    }

    #[test]
    fn clear_reports_a_handle_it_cannot_remove() {
        // A directory stands in for the unremovable handle: the real case is
        // a still-mounted path, which unlinks with EBUSY. Either way the
        // failure has to surface rather than be swallowed into a confusing
        // File::create error further down.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let path = root.join("net");
        fs::create_dir(&path).unwrap();

        clear(&path).unwrap_err();
    }

    #[test]
    fn the_runners_own_namespace_is_not_a_distinct_netns() {
        // The handle must be a namespace *other* than the one the runner is
        // in, or the VMM would keep host network reach.
        assert!(!is_live_netns(Utf8Path::new(SELF_NETNS)));
    }
}
