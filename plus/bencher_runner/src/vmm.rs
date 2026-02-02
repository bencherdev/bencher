//! VMM execution module.
//!
//! This module handles the `vmm` subcommand which runs inside the jail.
//! It performs:
//! 1. Namespace isolation (`unshare`)
//! 2. Filesystem isolation (`pivot_root`)
//! 3. Capability and seccomp restrictions
//! 4. VMM execution

use camino::Utf8PathBuf;

use crate::jail::{
    apply_rlimits, create_other_namespaces, create_user_namespace, do_pivot_root,
    drop_capabilities, mount_essential_filesystems, set_no_new_privs, setup_uid_gid_mapping,
    ResourceLimits,
};
use crate::RunnerError;

/// Configuration for the VMM subcommand.
#[derive(Debug, Clone)]
pub struct VmmConfig {
    /// Path to the jail root directory.
    pub jail_root: Utf8PathBuf,

    /// Path to the kernel (relative to jail root after pivot).
    pub kernel_path: Utf8PathBuf,

    /// Path to the rootfs (relative to jail root after pivot).
    pub rootfs_path: Utf8PathBuf,

    /// Path to vsock socket (relative to jail root after pivot).
    pub vsock_path: Option<Utf8PathBuf>,

    /// Number of vCPUs.
    pub vcpus: u8,

    /// Memory in MiB.
    pub memory_mib: u32,

    /// Kernel command line.
    pub kernel_cmdline: String,

    /// Timeout in seconds.
    pub timeout_secs: u64,

    /// Resource limits.
    pub limits: ResourceLimits,
}

/// Run the VMM inside the jail.
///
/// This function:
/// 1. Creates new namespaces
/// 2. Sets up UID/GID mapping
/// 3. Performs `pivot_root`
/// 4. Mounts essential filesystems
/// 5. Applies rlimits
/// 6. Sets `PR_SET_NO_NEW_PRIVS`
/// 7. Drops capabilities
/// 8. Applies seccomp (via VMM)
/// 9. Runs the VMM
///
/// Results are printed to stdout.
pub fn run_vmm(config: &VmmConfig) -> Result<(), RunnerError> {
    // Step 1: Create user namespace first (required before setting up UID/GID mapping)
    create_user_namespace()?;

    // Step 2: Set up UID/GID mapping (run as root inside the namespace)
    // This must be done BEFORE creating other namespaces like mount/network
    setup_uid_gid_mapping()?;

    // Step 3: Create remaining namespaces (mount, network, UTS, IPC)
    // Now that we have CAP_SYS_ADMIN in the user namespace, these will work
    create_other_namespaces()?;

    // Step 4: Pivot root to the jail
    do_pivot_root(&config.jail_root)?;

    // Step 5: Mount essential filesystems (/proc, /dev)
    mount_essential_filesystems()?;

    // Step 6: Apply rlimits
    apply_rlimits(&config.limits)?;

    // Step 7: Set PR_SET_NO_NEW_PRIVS
    set_no_new_privs()?;

    // Step 8: Drop all capabilities
    drop_capabilities()?;

    // Step 9 & 10: Create VMM config and run
    // The VMM will apply its own seccomp filters
    let vm_config = bencher_vmm::VmConfig {
        kernel_path: config.kernel_path.clone(),
        rootfs_path: config.rootfs_path.clone(),
        vcpus: config.vcpus,
        memory_mib: config.memory_mib,
        kernel_cmdline: config.kernel_cmdline.clone(),
        vsock_path: config.vsock_path.clone(),
        timeout_secs: config.timeout_secs,
    };

    let results = bencher_vmm::run_vm(&vm_config)?;

    // Output results to stdout
    println!("{results}");

    Ok(())
}
