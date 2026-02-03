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
    drop_capabilities, get_uid_gid, set_no_new_privs, setup_uid_gid_mapping, ResourceLimits,
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
    // Step 0: Capture UID/GID BEFORE entering new user namespace.
    // After unshare(CLONE_NEWUSER), getuid()/getgid() return the overflow
    // UID/GID (65534) until uid_map/gid_map are written.
    let (uid, gid) = get_uid_gid();

    // Step 1: Create user namespace first (required before setting up UID/GID mapping)
    create_user_namespace()?;

    // Step 2: Set up UID/GID mapping (run as root inside the namespace)
    // This must be done BEFORE creating other namespaces like mount/network
    setup_uid_gid_mapping(uid, gid)?;

    // Step 3: Create remaining namespaces (mount, network, UTS, IPC)
    // Now that we have CAP_SYS_ADMIN in the user namespace, these will work
    create_other_namespaces()?;

    // Step 3.5: Set up jail directories and bind-mount host filesystems
    // before pivot_root. Mounting proc fresh requires a PID namespace (which
    // needs fork), so we bind-mount from the host instead. All /dev setup is
    // also done here since /dev/kvm must persist through pivot_root.
    prepare_jail_mounts(&config.jail_root)?;

    // Step 4: Pivot root to the jail
    do_pivot_root(&config.jail_root)?;

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

    match bencher_vmm::run_vm(&vm_config) {
        Ok(results) => {
            // Output results to stdout
            println!("{results}");
            Ok(())
        }
        Err(bencher_vmm::VmmError::Timeout {
            timeout_secs,
            partial_output,
        }) => {
            // Print any partial output captured before the timeout
            if !partial_output.is_empty() {
                eprint!("{partial_output}");
            }
            Err(bencher_vmm::VmmError::Timeout {
                timeout_secs,
                partial_output: String::new(),
            }
            .into())
        }
        Err(e) => Err(e.into()),
    }
}

/// Prepare jail directories and bind-mount host filesystems before pivot_root.
///
/// Mounting a fresh procfs inside a user namespace requires a matching PID
/// namespace (i.e., `CLONE_NEWPID` + `fork()`). Since the VMM doesn't need its
/// own PID namespace and we want to avoid the fork complexity, we bind-mount
/// the host's `/proc` into the jail instead.
fn prepare_jail_mounts(jail_root: &camino::Utf8Path) -> Result<(), RunnerError> {
    use nix::mount::{mount, MsFlags};
    use std::fs;
    use std::path::Path;

    let jail = Path::new(jail_root.as_str());

    // Create essential directories
    for dir in ["proc", "dev", "tmp", "run"] {
        let path = jail.join(dir);
        if !path.exists() {
            fs::create_dir_all(&path).map_err(|e| {
                RunnerError::Jail(format!("failed to create {dir} in jail: {e}"))
            })?;
        }
    }

    // Bind-mount host /proc into the jail.
    // Mounting a fresh procfs requires a matching PID namespace, so we
    // bind-mount from the host instead.
    let jail_proc = jail.join("proc");
    mount(
        Some("/proc"),
        &jail_proc,
        None::<&str>,
        MsFlags::MS_BIND | MsFlags::MS_REC,
        None::<&str>,
    )
    .map_err(|e| RunnerError::Jail(format!("failed to bind-mount /proc into jail: {e}")))?;

    // Bind-mount /dev/kvm into the jail (critical for VMM).
    // We must do this before pivot_root since /dev/kvm won't be accessible after.
    let jail_kvm = jail.join("dev").join("kvm");
    fs::write(&jail_kvm, "")
        .map_err(|e| RunnerError::Jail(format!("failed to create /dev/kvm mount point: {e}")))?;
    mount(
        Some("/dev/kvm"),
        &jail_kvm,
        None::<&str>,
        MsFlags::MS_BIND,
        None::<&str>,
    )
    .map_err(|e| RunnerError::Jail(format!("failed to bind-mount /dev/kvm: {e}")))?;

    Ok(())
}
