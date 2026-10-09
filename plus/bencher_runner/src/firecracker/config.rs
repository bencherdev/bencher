//! Firecracker REST API configuration types.

use serde::Serialize;

use crate::jail::ChrootPath;

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

/// VM action request.
#[derive(Debug, Serialize)]
pub struct Action {
    /// The type of action to perform.
    pub action_type: ActionType,
}

/// Action types supported by Firecracker.
#[derive(Debug, Serialize)]
pub enum ActionType {
    /// Start the VM instance.
    InstanceStart,
}
