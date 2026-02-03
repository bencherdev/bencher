//! Firecracker REST API configuration types.

use serde::Serialize;

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
    /// Path to the kernel image on the host.
    pub kernel_image_path: String,
    /// Kernel boot arguments.
    pub boot_args: String,
}

/// Block device (drive) configuration.
#[derive(Debug, Serialize)]
pub struct Drive {
    /// Unique drive identifier.
    pub drive_id: String,
    /// Path to the disk image on the host.
    pub path_on_host: String,
    /// Whether this is the root device.
    pub is_root_device: bool,
    /// Whether the drive is read-only.
    pub is_read_only: bool,
}

/// Vsock device configuration.
#[derive(Debug, Serialize)]
pub struct VsockConfig {
    /// Guest CID (must be >= 3 for Firecracker).
    pub guest_cid: u32,
    /// Path to the Unix domain socket on the host.
    pub uds_path: String,
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
    /// Send Ctrl+Alt+Del to the guest (graceful shutdown).
    SendCtrlAltDel,
}
