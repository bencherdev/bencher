//! The configuration Firecracker boots the VM from, a file in its jail.

use std::fs::{OpenOptions, Permissions};
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};

use camino::Utf8Path;
use serde::Serialize;

use crate::firecracker::error::FirecrackerError;
use crate::jail::ChrootPath;

/// What `--config-file` reads, in place of the API calls it replaces.
#[derive(Debug, Serialize)]
pub struct VmConfig {
    #[serde(rename = "boot-source")]
    pub boot_source: BootSource,
    pub drives: Vec<Drive>,
    #[serde(rename = "machine-config")]
    pub machine_config: MachineConfig,
}

/// Machine configuration for Firecracker.
#[derive(Debug, Serialize)]
pub struct MachineConfig {
    /// Number of vCPUs.
    pub vcpu_count: u8,
    /// Memory size in MiB.
    pub mem_size_mib: u32,
    /// Whether to enable SMT (simultaneous multithreading).
    pub smt: bool,
}

/// Boot source configuration.
#[derive(Debug, Serialize)]
pub struct BootSource {
    pub kernel_image_path: ChrootPath,
    /// Kernel boot arguments.
    pub boot_args: String,
}

/// Block device (drive) configuration.
#[derive(Debug, Serialize)]
pub struct Drive {
    /// Unique drive identifier.
    pub drive_id: String,
    /// The chroot view, despite Firecracker's name for the field.
    pub path_on_host: ChrootPath,
    /// Whether this is the root device.
    pub is_root_device: bool,
    /// Whether the drive is read-only.
    pub is_read_only: bool,
}

impl VmConfig {
    /// A new file only, never one already at `path`, left root's and readable
    /// by the jail user, like the kernel.
    pub fn write(&self, path: &Utf8Path) -> Result<(), FirecrackerError> {
        let failed = |source| FirecrackerError::WriteVmConfig {
            path: path.to_owned(),
            source,
        };
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(path)
            .map_err(failed)?;
        serde_json::to_writer(&file, self)
            .map_err(std::io::Error::from)
            .map_err(failed)?;
        // On the descriptor, so the jail user can read it whatever the umask.
        file.set_permissions(Permissions::from_mode(0o644))
            .map_err(failed)
    }
}

#[cfg(test)]
mod tests {
    use std::os::unix::fs::{MetadataExt as _, symlink};

    use camino::Utf8PathBuf;

    use super::*;
    use crate::jail::JailPaths;

    fn config(jail: &JailPaths) -> VmConfig {
        VmConfig {
            boot_source: BootSource {
                kernel_image_path: jail.kernel().chroot().clone(),
                boot_args: "console=ttyS0".to_owned(),
            },
            drives: Vec::new(),
            machine_config: MachineConfig {
                vcpu_count: 1,
                mem_size_mib: 512,
                smt: false,
            },
        }
    }

    #[test]
    fn the_vm_config_is_readable_by_the_jail_user_and_writable_only_by_root() {
        // Kills a file the jailed VMM cannot read, which fails every Job, and
        // one it could rewrite.
        let dir = tempfile::tempdir().unwrap();
        let jail = JailPaths::new(Utf8Path::from_path(dir.path()).unwrap());
        let path = jail.vm_config().host().as_path();

        config(&jail).write(path).unwrap();

        let written = std::fs::metadata(path).unwrap();
        assert_eq!(written.mode() & 0o777, 0o644);
        assert_eq!(written.uid(), crate::jail::current_euid());
        let parsed: serde_json::Value =
            serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
        assert_eq!(parsed["boot-source"]["kernel_image_path"], "/vmlinux");
    }

    #[test]
    fn a_vm_config_path_already_taken_is_refused_not_reused() {
        // Kills writing through a file or link planted at the path, which would
        // hand the VMM a configuration, or overwrite a host file, the runner
        // never chose.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        let outside = root.join("outside");
        std::fs::write(&outside, b"host file").unwrap();

        for planted in ["file", "link"] {
            let jail_root = root.join(planted);
            std::fs::create_dir(&jail_root).unwrap();
            let jail = JailPaths::new(&jail_root);
            let path = jail.vm_config().host().as_path();
            if planted == "file" {
                std::fs::write(path, b"planted").unwrap();
            } else {
                symlink(&outside, path).unwrap();
            }

            let Err(err) = config(&jail).write(path) else {
                panic!("a {planted} at the path must be refused");
            };

            assert!(
                matches!(&err, FirecrackerError::WriteVmConfig { source, .. }
                    if source.kind() == std::io::ErrorKind::AlreadyExists),
                "{planted}: {err}"
            );
        }
        assert_eq!(
            std::fs::read(root.join("file").join("vm-config.json")).unwrap(),
            b"planted"
        );
        assert_eq!(std::fs::read(&outside).unwrap(), b"host file");
    }
}
