//! An empty network namespace for the VMM process, so a compromised Firecracker
//! cannot reach the host network; vsock still works because its host side is
//! Unix domain sockets.

use std::fs;
use std::os::fd::AsRawFd as _;
use std::os::unix::fs::MetadataExt as _;

use camino::{Utf8Path, Utf8PathBuf};
use nix::mount::{MntFlags, MsFlags, mount, umount2};
use nix::sched::{CloneFlags, unshare};
use slog::{Logger, info};

use crate::error::JailError;

/// iproute2's `NETNS_RUN_DIR`, so operators see the namespace in
/// `ip netns list`.
const NETNS_DIR: &str = "/run/netns";

const NETNS_NAME: &str = "bencher-jail";

const SELF_NETNS: &str = "/proc/self/ns/net";

/// Bounded because an unmount that succeeds forever is a kernel fault, and the
/// unlink that follows reports the real state either way.
const MAX_STACKED_MOUNTS: usize = 32;

const NETNS_LOCK_PATH: &str = "/run/bencher_runner_netns.lock";

/// Not `/proc/self`, which resolves through the thread group leader and would
/// pin the runner's host namespace instead of the new one.
const THREAD_NETNS: &str = "/proc/thread-self/ns/net";

#[must_use]
pub fn handle_path() -> Utf8PathBuf {
    Utf8Path::new(NETNS_DIR).join(NETNS_NAME)
}

/// Always rebuilt rather than reused, since a handle proven to be a namespace
/// is not proven empty and a leftover one could hand the VMM host network reach.
pub fn ensure(log: &Logger) -> Result<Utf8PathBuf, JailError> {
    let handle = handle_path();

    fs::create_dir_all(NETNS_DIR).map_err(|e| JailError::NetnsDir {
        path: Utf8PathBuf::from(NETNS_DIR),
        source: e,
    })?;

    let _lock = NetnsLock::acquire(log)?;

    clear(&handle)?;

    // The bind mount needs a regular file to land on.
    fs::File::create(&handle).map_err(|e| JailError::NetnsHandle {
        path: handle.clone(),
        source: e,
    })?;

    // Always `clear`, never a bare unlink: a mounted handle unlinks with
    // `EBUSY`, and one left mounted makes every later `ensure` fail.
    if let Err(e) = create(&handle) {
        drop(clear(&handle));
        return Err(e);
    }

    // Anything but a namespace distinct from the runner's own would leave the
    // VMM with host network reach.
    if !is_live_netns(&handle) {
        drop(clear(&handle));
        return Err(JailError::NetnsNotDistinct { path: handle });
    }

    Ok(handle)
}

/// Unwinds one mount at a time, because bind mounts over a file stack silently
/// and a single detach leaves the path mounted and unremovable.
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

/// Spans runners with different state directories, which share this one
/// handle, but covers only the rebuild, so a concurrent rebuild can still fail
/// another runner's jailer loudly with `ENOENT`.
struct NetnsLock {
    _file: fs::File,
}

impl NetnsLock {
    fn acquire(log: &Logger) -> Result<Self, JailError> {
        let path = Utf8Path::new(NETNS_LOCK_PATH);
        let file = fs::OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(false)
            .open(path)
            .map_err(|e| JailError::OpenNetnsLock {
                path: path.to_owned(),
                source: e,
            })?;

        // Try once without blocking, so the wait can be announced rather than
        // looking like a hang.
        if flock_nonblocking(&file).is_ok() {
            return Ok(Self { _file: file });
        }
        info!(log, "Waiting for the network namespace lock"; "path" => path.as_str());

        flock_exclusive(&file).map_err(|e| JailError::NetnsLock {
            path: path.to_owned(),
            source: e,
        })?;
        Ok(Self { _file: file })
    }
}

fn flock_exclusive(file: &fs::File) -> std::io::Result<()> {
    flock(file, libc::LOCK_EX)
}

fn flock_nonblocking(file: &fs::File) -> std::io::Result<()> {
    flock(file, libc::LOCK_EX | libc::LOCK_NB)
}

fn flock(file: &fs::File, operation: libc::c_int) -> std::io::Result<()> {
    loop {
        #[expect(
            unsafe_code,
            reason = "flock has no std wrapper; the fd is owned and valid"
        )]
        // SAFETY: `file` is an open, owned descriptor for the duration of the
        // call; flock does not touch memory.
        let ret = unsafe { libc::flock(file.as_raw_fd(), operation) };
        if ret == 0 {
            return Ok(());
        }
        let err = std::io::Error::last_os_error();
        if err.kind() != std::io::ErrorKind::Interrupted {
            return Err(err);
        }
    }
}

/// Every namespace inode lives on the one kernel `nsfs`, so a shared device
/// proves `handle` is a namespace and a different inode proves it is not the
/// runner's own.
fn is_live_netns(handle: &Utf8Path) -> bool {
    let (Ok(own), Ok(candidate)) = (fs::metadata(SELF_NETNS), fs::metadata(handle)) else {
        return false;
    };
    own.dev() == candidate.dev() && own.ino() != candidate.ino()
}

/// Unshares on a throwaway thread, since network namespaces are per task and
/// the thread never returns to the host namespace.
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
        // A directory stands in for a still-mounted handle, whose failed unlink
        // must surface rather than be swallowed.
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
