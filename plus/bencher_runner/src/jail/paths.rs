//! The two views of every file inside the jail chroot.
//!
//! Once Firecracker is jailed, every path it receives resolves inside the
//! chroot, while the runner reaches the same file from outside. The two views
//! are different types rather than two strings, so handing Firecracker a host
//! path (or the runner a chroot path) is a compile error instead of a boot
//! that hangs waiting for a socket that will never appear.

use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;

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

/// One file in the jail, in both views.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JailFile {
    host: HostPath,
    chroot: ChrootPath,
}

impl JailFile {
    /// Build both views of `name` directly under the chroot root.
    #[must_use]
    pub fn new(jail_root: &Utf8Path, name: &str) -> Self {
        Self {
            host: HostPath(jail_root.join(name)),
            chroot: ChrootPath(Utf8Path::new("/").join(name)),
        }
    }

    /// The path the runner uses.
    #[must_use]
    pub fn host(&self) -> &HostPath {
        &self.host
    }

    /// The path Firecracker receives.
    #[must_use]
    pub fn chroot(&self) -> &ChrootPath {
        &self.chroot
    }
}

/// Every file the runner places in, or reaches inside, a jail chroot.
#[derive(Debug, Clone)]
pub struct JailPaths {
    root: Utf8PathBuf,
    api_socket: JailFile,
    kernel: JailFile,
    rootfs: JailFile,
    vsock: JailFile,
}

impl JailPaths {
    /// Resolve both views of every jail file for a chroot rooted at
    /// `jail_root`.
    #[must_use]
    pub fn new(jail_root: &Utf8Path) -> Self {
        Self {
            root: jail_root.to_owned(),
            api_socket: JailFile::new(jail_root, "api.sock"),
            kernel: JailFile::new(jail_root, "vmlinux"),
            rootfs: JailFile::new(jail_root, "rootfs.ext4"),
            vsock: JailFile::new(jail_root, "v.sock"),
        }
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

    const JAIL_ROOT: &str = "/var/lib/bencher-runner/jail/firecracker/vm-1/root";

    fn paths() -> JailPaths {
        JailPaths::new(Utf8Path::new(JAIL_ROOT))
    }

    #[test]
    fn chroot_view_is_rooted_at_the_chroot() {
        let paths = paths();
        assert_eq!(paths.api_socket().chroot().as_str(), "/api.sock");
        assert_eq!(paths.kernel().chroot().as_str(), "/vmlinux");
        assert_eq!(paths.rootfs().chroot().as_str(), "/rootfs.ext4");
        assert_eq!(paths.vsock().chroot().as_str(), "/v.sock");
    }

    #[test]
    fn host_view_is_under_the_jail_root() {
        let paths = paths();
        assert_eq!(
            paths.api_socket().host().as_str(),
            format!("{JAIL_ROOT}/api.sock")
        );
        assert_eq!(
            paths.kernel().host().as_str(),
            format!("{JAIL_ROOT}/vmlinux")
        );
        assert_eq!(
            paths.rootfs().host().as_str(),
            format!("{JAIL_ROOT}/rootfs.ext4")
        );
        assert_eq!(paths.vsock().host().as_str(), format!("{JAIL_ROOT}/v.sock"));
    }

    #[test]
    fn the_two_views_round_trip_through_the_jail_root() {
        let paths = paths();
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
    fn chroot_paths_serialize_as_bare_strings() {
        let paths = paths();
        assert_eq!(
            serde_json::to_string(paths.rootfs().chroot()).unwrap(),
            "\"/rootfs.ext4\""
        );
    }
}
