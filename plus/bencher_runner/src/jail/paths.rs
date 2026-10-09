//! Each jail file's host, chroot, and socket paths as distinct types, so
//! handing one view where another belongs fails to compile.

use std::fs::File;
use std::os::fd::AsRawFd as _;
use std::os::unix::fs::{FileTypeExt as _, OpenOptionsExt as _};

use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;

use crate::error::JailError;

/// Size of `sockaddr_un.sun_path` on Linux.
const SUN_PATH_LEN: usize = 108;

/// Leaves room for the NUL, which the standard library requires even though
/// Linux accepts 108 unterminated bytes.
const MAX_SOCKET_PATH: usize = SUN_PATH_LEN - 1;

/// A path as the runner sees it: the host filesystem, outside the chroot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostPath(Utf8PathBuf);

impl HostPath {
    #[must_use]
    pub fn as_path(&self) -> &Utf8Path {
        &self.0
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Display for HostPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// A path as the jailed Firecracker process sees it, rooted at the chroot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ChrootPath(Utf8PathBuf);

impl ChrootPath {
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl std::fmt::Display for ChrootPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// A path for `bind` or `connect` only, naming a descriptor its [`JailPaths`]
/// holds open, so it silently names something else once that value is dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocketPath(String);

impl SocketPath {
    /// Check a path against the `sun_path` limit.
    fn new(path: String) -> Result<Self, JailError> {
        let length = path.len();
        if length > MAX_SOCKET_PATH {
            return Err(JailError::SocketPathTooLong {
                path,
                length,
                limit: MAX_SOCKET_PATH,
            });
        }
        Ok(Self(path))
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for SocketPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// A socket inode held open, so a compromised VMM that later swaps the name
/// for a link to a host socket is never followed.
#[derive(Debug)]
pub struct PinnedSocket {
    /// Held for as long as `path` names it.
    _file: File,
    path: SocketPath,
}

impl PinnedSocket {
    /// Pin the socket a view names, refusing anything that is not one, a link
    /// included.
    pub fn pin(view: &SocketPath) -> Result<Self, JailError> {
        let pin_failed = |source| JailError::PinSocket {
            path: view.clone(),
            source,
        };
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_PATH | libc::O_NOFOLLOW)
            .open(view.as_str())
            .map_err(pin_failed)?;
        if !file.metadata().map_err(pin_failed)?.file_type().is_socket() {
            return Err(JailError::NotASocket { path: view.clone() });
        }
        let path = SocketPath::new(format!("/proc/self/fd/{}", file.as_raw_fd()))?;
        Ok(Self { _file: file, path })
    }

    #[must_use]
    pub fn path(&self) -> &SocketPath {
        &self.path
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JailFile {
    host: HostPath,
    chroot: ChrootPath,
    socket: SocketPath,
}

impl JailFile {
    #[must_use]
    pub fn host(&self) -> &HostPath {
        &self.host
    }

    #[must_use]
    pub fn chroot(&self) -> &ChrootPath {
        &self.chroot
    }

    #[must_use]
    pub fn socket(&self) -> &SocketPath {
        &self.socket
    }
}

#[derive(Debug)]
pub struct JailPaths {
    root: Utf8PathBuf,
    /// The socket views name this descriptor by number, so closing it would
    /// leave them addressing whatever the kernel reuses that number for.
    _dir: File,
    api_socket: JailFile,
    kernel: JailFile,
    rootfs: JailFile,
    results: JailFile,
}

impl JailPaths {
    /// Resolve every view of every jail file for a chroot that already exists.
    pub fn new(jail_root: &Utf8Path) -> Result<Self, JailError> {
        let dir = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_PATH | libc::O_DIRECTORY)
            .open(jail_root)
            .map_err(|e| JailError::OpenJailRoot {
                path: jail_root.to_owned(),
                source: e,
            })?;

        // A descriptor keeps socket paths within the 108-byte `sun_path` however
        // deep the operator's state directory is.
        let dir_path = format!("/proc/self/fd/{}", dir.as_raw_fd());

        let file = |name: &str| -> Result<JailFile, JailError> {
            Ok(JailFile {
                host: HostPath(jail_root.join(name)),
                chroot: ChrootPath(Utf8Path::new("/").join(name)),
                socket: SocketPath::new(format!("{dir_path}/{name}"))?,
            })
        };

        Ok(Self {
            root: jail_root.to_owned(),
            api_socket: file("api.sock")?,
            kernel: file("vmlinux")?,
            rootfs: file("rootfs.ext4")?,
            results: file("results.img")?,
            _dir: dir,
        })
    }

    /// The chroot root on the host, which becomes `/` inside the jail.
    #[must_use]
    pub fn root(&self) -> &Utf8Path {
        &self.root
    }

    #[must_use]
    pub fn api_socket(&self) -> &JailFile {
        &self.api_socket
    }

    #[must_use]
    pub fn kernel(&self) -> &JailFile {
        &self.kernel
    }

    #[must_use]
    pub fn rootfs(&self) -> &JailFile {
        &self.rootfs
    }

    /// The drive the guest leaves its results on.
    #[must_use]
    pub fn results(&self) -> &JailFile {
        &self.results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn jail_in_tmpdir() -> (tempfile::TempDir, JailPaths) {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(dir.path()).unwrap();
        let paths = JailPaths::new(root).unwrap();
        (dir, paths)
    }

    #[test]
    fn the_chroot_and_host_views_round_trip_through_the_jail_root() {
        let (_dir, paths) = jail_in_tmpdir();
        for file in [
            paths.api_socket(),
            paths.kernel(),
            paths.rootfs(),
            paths.results(),
        ] {
            let relative = Utf8Path::new(file.chroot().as_str())
                .strip_prefix("/")
                .unwrap();
            assert_eq!(
                paths.root().join(relative),
                file.host().as_path(),
                "the chroot view of {file:?} must resolve to its host view"
            );
        }
    }

    #[test]
    fn the_socket_view_resolves_to_the_same_file_as_the_host_view() {
        // If the views diverge, the runner and Firecracker stop meeting.
        let (_dir, paths) = jail_in_tmpdir();
        std::fs::write(paths.rootfs().host().as_path(), b"guest").unwrap();

        let through_socket_view = std::fs::read(paths.rootfs().socket().as_str()).unwrap();

        assert_eq!(through_socket_view, b"guest");
    }

    #[test]
    fn the_socket_view_stops_naming_the_jail_once_the_paths_are_dropped() {
        // Pins the use-after-drop hazard: a released descriptor number is
        // reused, so the same string silently names another directory.
        let jail = tempfile::tempdir().unwrap();
        let jail_root = Utf8Path::from_path(jail.path()).unwrap();
        std::fs::write(jail_root.join("rootfs.ext4"), b"the jail").unwrap();

        let socket_view = {
            let paths = JailPaths::new(jail_root).unwrap();
            let view = paths.rootfs().socket().as_str().to_owned();
            assert_eq!(std::fs::read(&view).unwrap(), b"the jail");
            view
        };

        let impostor = tempfile::tempdir().unwrap();
        let impostor_root = Utf8Path::from_path(impostor.path()).unwrap();
        std::fs::write(impostor_root.join("rootfs.ext4"), b"somewhere else").unwrap();
        let _claim = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_PATH | libc::O_DIRECTORY)
            .open(impostor_root)
            .unwrap();

        let stale = std::fs::read(&socket_view);
        assert_ne!(
            stale.unwrap_or_default(),
            b"the jail",
            "a dropped descriptor must not leave the socket view pointing at the jail"
        );
    }

    #[test]
    fn the_socket_view_survives_a_jail_root_far_past_the_limit() {
        // Fails if the socket view is built from the host path rather than the
        // descriptor.
        let dir = tempfile::tempdir().unwrap();
        let mut root = Utf8Path::from_path(dir.path()).unwrap().to_owned();
        for _ in 0..8 {
            root = root.join("a-fairly-long-directory-name");
        }
        std::fs::create_dir_all(&root).unwrap();
        assert!(
            root.as_str().len() > MAX_SOCKET_PATH,
            "the host path has to be over the limit for this to prove anything"
        );

        let paths = JailPaths::new(&root).unwrap();

        assert!(paths.api_socket().socket().as_str().len() <= MAX_SOCKET_PATH);
    }

    #[test]
    fn an_oversized_socket_path_names_the_limit_and_the_length() {
        let err = SocketPath::new("/x".repeat(80)).unwrap_err();
        let message = err.to_string();

        assert!(message.contains("107"), "the limit is named: {message}");
        assert!(message.contains("160"), "the length is named: {message}");
    }

    #[test]
    fn a_socket_swapped_for_a_link_is_not_pinned() {
        // Later API calls trust the pin, so a link must be refused, not followed.
        use std::os::unix::fs::symlink;
        use std::os::unix::net::UnixListener;

        let (dir, paths) = jail_in_tmpdir();
        let elsewhere = Utf8Path::from_path(dir.path())
            .unwrap()
            .join("elsewhere.sock");
        let _listener = UnixListener::bind(&elsewhere).unwrap();
        symlink(&elsewhere, paths.api_socket().host().as_path()).unwrap();

        let err = PinnedSocket::pin(paths.api_socket().socket()).unwrap_err();

        assert!(
            matches!(err, JailError::NotASocket { .. }),
            "a link is refused, not followed: {err}"
        );
    }

    #[test]
    fn a_socket_path_at_the_limit_is_accepted_and_one_past_it_refused() {
        // Kills an off-by-one at the limit, either way.
        SocketPath::new("a".repeat(MAX_SOCKET_PATH)).unwrap();
        SocketPath::new("a".repeat(MAX_SOCKET_PATH + 1)).unwrap_err();
    }

    #[test]
    fn a_missing_jail_root_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(dir.path()).unwrap().join("absent");

        JailPaths::new(&root).unwrap_err();
    }
}
