//! Each jail file's host and chroot paths as distinct types, so handing one
//! view where another belongs fails to compile.

use camino::{Utf8Path, Utf8PathBuf};
use serde::Serialize;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JailFile {
    host: HostPath,
    chroot: ChrootPath,
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
}

#[derive(Debug)]
pub struct JailPaths {
    root: Utf8PathBuf,
    vm_config: JailFile,
    kernel: JailFile,
    rootfs: JailFile,
    results: JailFile,
}

impl JailPaths {
    /// Resolve both views of every jail file under a chroot root.
    #[must_use]
    pub fn new(jail_root: &Utf8Path) -> Self {
        let file = |name: &str| JailFile {
            host: HostPath(jail_root.join(name)),
            chroot: ChrootPath(Utf8Path::new("/").join(name)),
        };

        Self {
            root: jail_root.to_owned(),
            vm_config: file("vm-config.json"),
            kernel: file("vmlinux"),
            rootfs: file("rootfs.ext4"),
            results: file("results.img"),
        }
    }

    /// The chroot root on the host, which becomes `/` inside the jail.
    #[must_use]
    pub fn root(&self) -> &Utf8Path {
        &self.root
    }

    /// The configuration Firecracker boots the VM from.
    #[must_use]
    pub fn vm_config(&self) -> &JailFile {
        &self.vm_config
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

    #[test]
    fn the_chroot_and_host_views_round_trip_through_the_jail_root() {
        let dir = tempfile::tempdir().unwrap();
        let paths = JailPaths::new(Utf8Path::from_path(dir.path()).unwrap());
        for file in [
            paths.vm_config(),
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
}
