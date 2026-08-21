//! The views of every file inside the jail chroot.
//!
//! Once Firecracker is jailed, every path it receives resolves inside the
//! chroot, while the runner reaches the same file from outside. The views are
//! different types rather than strings, so handing Firecracker a host path (or
//! the runner a chroot path) is a compile error instead of a boot that hangs
//! waiting for a socket that will never appear.
//!
//! There is a third view because of a hard kernel limit. `sockaddr_un.sun_path`
//! is 108 bytes, and the limit applies to the string handed to `bind` and
//! `connect`, before any resolution. A jail deep under an operator's
//! `--state-dir` blows that limit long before it comes near `PATH_MAX`, so the
//! runner addresses the jail's sockets through a descriptor it holds open on
//! the chroot. Resolution happens after the length check, so a short string
//! naming a long directory is exactly what is needed.

use std::fs::File;
use std::os::fd::AsRawFd as _;
use std::os::unix::fs::OpenOptionsExt as _;

use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;

use crate::error::JailError;

/// Size of `sockaddr_un.sun_path` on Linux.
const SUN_PATH_LEN: usize = 108;

/// Longest path a socket name may occupy, leaving room for the NUL.
///
/// Linux accepts a full 108 unterminated bytes when `addrlen` says so, but
/// `unix(7)` warns against relying on it and the standard library rejects
/// anything that does not leave room for the terminator.
const MAX_SOCKET_PATH: usize = SUN_PATH_LEN - 1;

/// Room reserved after every socket path for a `_<port>` suffix.
///
/// Sized for the widest port a `u32` can print rather than for the ports in
/// use, so adding a port can never silently eat the margin. Reserved on every
/// socket path rather than on the vsock base alone, because that is what makes
/// [`SocketPath::with_port`] infallible: the widest suffix it can produce is
/// the one every base was already checked with room for. The socket views name
/// a descriptor rather than the operator's state directory, so the eleven bytes
/// come out of a margin nothing is ever close to spending.
const PORT_SUFFIX_RESERVE: usize = "_4294967295".len();

/// A path as the runner sees it: the host filesystem, outside the chroot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HostPath(Utf8PathBuf);

impl HostPath {
    /// The path as a [`Utf8Path`].
    #[must_use]
    pub fn as_path(&self) -> &Utf8Path {
        &self.0
    }

    /// The path as a string.
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
///
/// Serializes as the bare path, since these are what the Firecracker API
/// request bodies carry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ChrootPath(Utf8PathBuf);

impl ChrootPath {
    /// The path as a string.
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

/// A path the runner may hand to `bind` or `connect`.
///
/// The value names a descriptor held open by the [`JailPaths`] it came from,
/// and it is only valid while that value is alive. That invariant is upheld by
/// ordering rather than by the compiler: `SocketPath` could borrow, which would
/// make a use-after-drop impossible, but the lifetime would ripple through
/// [`JailFile`], the Firecracker job config, the jailer spawn, and everything
/// that owns them. It is deliberately not enforced, not overlooked, and it is
/// a reasonable follow-up.
///
/// Two things hold the line in the meantime. Only `bind` and `connect` take
/// this view, because they are the only callers subject to the length limit;
/// everything else, unlinking above all, takes the host view, which cannot go
/// stale. And a test drops the paths, claims the released descriptor number
/// with another directory, and asserts the same string no longer names the
/// jail, so the hazard is at least pinned.
///
/// The type makes the length limit unforgeable: every value has been checked
/// against `sun_path` with room for the port it may later take, so a path that
/// would not fit is reported when the jail is built, naming the limit and the
/// offending string, rather than surfacing later as a socket that never becomes
/// ready. Growing one is [`Self::with_port`], which cannot spend more than was
/// reserved, so the check holds for every string this type hands out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SocketPath(String);

impl SocketPath {
    /// Check a path against the `sun_path` limit.
    ///
    /// Checked with [`PORT_SUFFIX_RESERVE`] in hand, so a base that fits only
    /// while it is bare is rejected rather than admitted and then grown past
    /// the limit by [`Self::with_port`].
    fn new(path: String) -> Result<Self, JailError> {
        let length = path.len() + PORT_SUFFIX_RESERVE;
        if length > MAX_SOCKET_PATH {
            return Err(JailError::SocketPathTooLong {
                path,
                length,
                limit: MAX_SOCKET_PATH,
            });
        }
        Ok(Self(path))
    }

    /// The path as a string.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// This path with a vsock port appended.
    ///
    /// The runner binds `{base}_{port}` for each vsock port. A port is the only
    /// thing this type will append, and every base was checked with room for
    /// the widest one a `u32` can print, so the result is within the limit by
    /// construction rather than by a promise the caller has to keep. A free
    /// `&str` suffix could make no such claim: nothing held it to the reserve,
    /// so an over-long one would fit under a shallow state directory and fail
    /// at `bind` under a deep one, which is a defect that shows up on some
    /// hosts only.
    #[must_use]
    pub fn with_port(&self, port: u32) -> String {
        format!("{}_{port}", self.0)
    }
}

impl std::fmt::Display for SocketPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// One file in the jail, in every view that reaches it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JailFile {
    host: HostPath,
    chroot: ChrootPath,
    socket: SocketPath,
}

impl JailFile {
    /// The path the runner uses to create, read, and own the file.
    #[must_use]
    pub fn host(&self) -> &HostPath {
        &self.host
    }

    /// The path Firecracker receives.
    #[must_use]
    pub fn chroot(&self) -> &ChrootPath {
        &self.chroot
    }

    /// The path the runner binds or connects to.
    #[must_use]
    pub fn socket(&self) -> &SocketPath {
        &self.socket
    }
}

/// Every file the runner places in, or reaches inside, a jail chroot.
#[derive(Debug)]
pub struct JailPaths {
    root: Utf8PathBuf,
    /// Held open for the life of the job.
    ///
    /// The socket views name this descriptor by number, so closing it would
    /// leave them addressing whatever directory the kernel hands that number
    /// to next. `O_PATH` because the runner never reads or writes through it:
    /// it exists only to be named.
    _dir: File,
    api_socket: JailFile,
    kernel: JailFile,
    rootfs: JailFile,
    vsock: JailFile,
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

        // Naming the directory by descriptor keeps the string short however
        // deep the operator's state directory is. The descriptor also pins the
        // directory's identity for the whole job.
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
            vsock: file("v.sock")?,
            _dir: dir,
        })
    }

    /// The chroot root on the host, which becomes `/` inside the jail.
    #[must_use]
    pub fn root(&self) -> &Utf8Path {
        &self.root
    }

    /// The Firecracker REST API socket.
    #[must_use]
    pub fn api_socket(&self) -> &JailFile {
        &self.api_socket
    }

    /// The guest kernel image, which Firecracker reads.
    #[must_use]
    pub fn kernel(&self) -> &JailFile {
        &self.kernel
    }

    /// The guest rootfs image, which Firecracker reads and writes.
    #[must_use]
    pub fn rootfs(&self) -> &JailFile {
        &self.rootfs
    }

    /// The base path of the vsock Unix domain sockets.
    #[must_use]
    pub fn vsock(&self) -> &JailFile {
        &self.vsock
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
    fn chroot_view_is_rooted_at_the_chroot() {
        let (_dir, paths) = jail_in_tmpdir();
        assert_eq!(paths.api_socket().chroot().as_str(), "/api.sock");
        assert_eq!(paths.kernel().chroot().as_str(), "/vmlinux");
        assert_eq!(paths.rootfs().chroot().as_str(), "/rootfs.ext4");
        assert_eq!(paths.vsock().chroot().as_str(), "/v.sock");
    }

    #[test]
    fn host_view_is_under_the_jail_root() {
        let (_dir, paths) = jail_in_tmpdir();
        let root = paths.root().to_owned();
        assert_eq!(paths.api_socket().host().as_path(), root.join("api.sock"));
        assert_eq!(paths.kernel().host().as_path(), root.join("vmlinux"));
        assert_eq!(paths.rootfs().host().as_path(), root.join("rootfs.ext4"));
        assert_eq!(paths.vsock().host().as_path(), root.join("v.sock"));
    }

    #[test]
    fn the_chroot_and_host_views_round_trip_through_the_jail_root() {
        let (_dir, paths) = jail_in_tmpdir();
        for file in [
            paths.api_socket(),
            paths.kernel(),
            paths.rootfs(),
            paths.vsock(),
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
        // The whole point: a short string naming the same inode. If this ever
        // stops holding, the runner and Firecracker stop meeting.
        let (_dir, paths) = jail_in_tmpdir();
        std::fs::write(paths.rootfs().host().as_path(), b"guest").unwrap();

        let through_socket_view = std::fs::read(paths.rootfs().socket().as_str()).unwrap();

        assert_eq!(through_socket_view, b"guest");
    }

    #[test]
    fn the_socket_view_stops_naming_the_jail_once_the_paths_are_dropped() {
        // This is why `bind` and `connect` are the only callers of the socket
        // view: the descriptor number is reused the moment it is released, and
        // the identical string then resolves to a different directory with no
        // error at all. Anything that only needs a path, unlinking above all,
        // uses the host view, which cannot go stale.
        let jail = tempfile::tempdir().unwrap();
        let jail_root = Utf8Path::from_path(jail.path()).unwrap();
        std::fs::write(jail_root.join("rootfs.ext4"), b"the jail").unwrap();

        let socket_view = {
            let paths = JailPaths::new(jail_root).unwrap();
            let view = paths.rootfs().socket().as_str().to_owned();
            assert_eq!(std::fs::read(&view).unwrap(), b"the jail");
            view
        };

        // Claim the number the jail's descriptor just released.
        let impostor = tempfile::tempdir().unwrap();
        let impostor_root = Utf8Path::from_path(impostor.path()).unwrap();
        std::fs::write(impostor_root.join("rootfs.ext4"), b"somewhere else").unwrap();
        let _claim = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(libc::O_PATH | libc::O_DIRECTORY)
            .open(impostor_root)
            .unwrap();

        // The same string no longer names the jail. It either names the
        // impostor or fails; what it must never do is still work.
        let stale = std::fs::read(&socket_view);
        assert_ne!(
            stale.unwrap_or_default(),
            b"the jail",
            "a dropped descriptor must not leave the socket view pointing at the jail"
        );
    }

    #[test]
    fn every_socket_view_fits_the_sun_path_limit_with_its_widest_port() {
        // The reserve is what makes `with_port` infallible, and it is kept on
        // every socket path rather than only the one that grows today: a base
        // admitted without it would take a port and fit under a shallow state
        // directory while failing at `bind` under a deep one.
        let (_dir, paths) = jail_in_tmpdir();
        for file in [
            paths.api_socket(),
            paths.kernel(),
            paths.rootfs(),
            paths.vsock(),
        ] {
            assert!(
                file.socket().as_str().len() <= MAX_SOCKET_PATH,
                "{} must fit sun_path",
                file.socket()
            );
            let widest = file.socket().with_port(u32::MAX);
            assert!(widest.len() <= MAX_SOCKET_PATH, "{widest} must fit");
        }
    }

    #[test]
    fn the_socket_view_survives_a_jail_root_far_past_the_limit() {
        // A deep state directory is exactly the case that produced a five
        // second timeout pointing at Firecracker instead of at the path.
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
        assert!(message.contains("171"), "the length is named: {message}");
    }

    #[test]
    fn a_base_that_fits_only_while_it_is_bare_is_refused() {
        // The port suffix counts against the limit at construction, because
        // that is the only place a caller is left with anywhere to go. Admitted
        // bare, the same path would be over the limit the moment it took the
        // port it exists to take.
        let widest = MAX_SOCKET_PATH - PORT_SUFFIX_RESERVE;
        let fits = "/a".repeat(48);
        assert_eq!(fits.len(), widest);

        SocketPath::new(fits).unwrap();
        SocketPath::new(format!("{}b", "/a".repeat(48))).unwrap_err();
    }

    #[test]
    fn a_missing_jail_root_is_reported() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(dir.path()).unwrap().join("absent");

        JailPaths::new(&root).unwrap_err();
    }

    #[test]
    fn chroot_paths_serialize_as_bare_strings() {
        let (_dir, paths) = jail_in_tmpdir();
        assert_eq!(
            serde_json::to_string(paths.rootfs().chroot()).unwrap(),
            "\"/rootfs.ext4\""
        );
    }
}
