//! VMM execution module.
//!
//! This module handles the `vmm` subcommand which runs inside the jail.
//! It performs:
//! 1. Namespace isolation (`unshare` + `fork` for PID namespace)
//! 2. Filesystem isolation (`pivot_root`)
//! 3. Capability and seccomp restrictions
//! 4. VMM execution

use std::time::Instant;

use camino::Utf8PathBuf;

use crate::jail::{
    apply_rlimits, create_other_namespaces, create_user_namespace, do_pivot_root,
    drop_capabilities, fork_into_pid_namespace, get_uid_gid, set_no_new_privs,
    setup_uid_gid_mapping, ResourceLimits,
};
use crate::metrics::{self, RunMetrics};
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

    /// Per-run nonce for HMAC result integrity verification.
    pub nonce: Option<String>,
}

/// Run the VMM inside the jail.
///
/// This function:
/// 1. Creates new namespaces (user, mount, network, UTS, IPC, PID)
/// 2. Sets up UID/GID mapping
/// 3. Forks into the PID namespace (child becomes PID 1)
/// 4. Mounts essential filesystems (fresh procfs, /dev/kvm)
/// 5. Performs `pivot_root`
/// 6. Applies rlimits
/// 7. Sets `PR_SET_NO_NEW_PRIVS`
/// 8. Drops capabilities
/// 9. Applies seccomp (via VMM)
/// 10. Runs the VMM
///
/// Results are printed to stdout by the child process.
/// The parent waits for the child and propagates its exit code.
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

    // Step 3: Create remaining namespaces (mount, network, UTS, IPC, PID)
    // Now that we have CAP_SYS_ADMIN in the user namespace, these will work
    create_other_namespaces()?;

    // Step 3.5: Fork into PID namespace.
    // After unshare(CLONE_NEWPID), only children enter the new namespace.
    // The child becomes PID 1 in the new PID namespace.
    if let Some(exit_code) = fork_into_pid_namespace()? {
        // Parent: propagate child's exit code
        if exit_code != 0 {
            return Err(RunnerError::Jail(format!(
                "VMM child process exited with code {exit_code}"
            )));
        }
        return Ok(());
    }

    // === Child process (PID 1 in new namespace) from here ===

    // Step 4: Set up jail directories and mount filesystems.
    // Now that we have a PID namespace, mount fresh procfs instead of
    // bind-mounting from the host.
    prepare_jail_mounts(&config.jail_root)?;

    // Step 5: Pivot root to the jail
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
        nonce: config.nonce.clone(),
    };

    let start_time = Instant::now();

    let result = bencher_vmm::run_vm(&vm_config);
    let elapsed = start_time.elapsed();

    // Since we are PID 1 in the namespace, we must exit explicitly.
    // The parent will see our exit code via waitpid.
    match result {
        Ok(vm_results) => {
            // Output metrics to stderr
            let run_metrics = RunMetrics {
                wall_clock_ms: elapsed.as_millis() as u64,
                timed_out: false,
                transport: vm_results.transport.to_string(),
                cgroup: None,
            };
            if let Some(line) = metrics::format_metrics(&run_metrics) {
                eprintln!("{line}");
            }

            // Log HMAC verification status to stderr
            match vm_results.hmac_verified {
                Some(true) => eprintln!("[HMAC] Verification passed"),
                Some(false) => eprintln!("[HMAC] WARNING: Verification failed"),
                None => {}
            }

            // Output results to stdout
            println!("{}", vm_results.output);
            std::process::exit(0);
        }
        Err(bencher_vmm::VmmError::Timeout {
            timeout_secs: _,
            partial_output,
        }) => {
            // Output metrics to stderr
            let run_metrics = RunMetrics {
                wall_clock_ms: elapsed.as_millis() as u64,
                timed_out: true,
                transport: "serial".to_owned(),
                cgroup: None,
            };
            if let Some(line) = metrics::format_metrics(&run_metrics) {
                eprintln!("{line}");
            }

            // Print any partial output captured before the timeout
            if !partial_output.is_empty() {
                eprint!("{partial_output}");
            }
            std::process::exit(1);
        }
        Err(e) => {
            eprintln!("VMM error: {e}");
            std::process::exit(1);
        }
    }
}

/// Prepare jail directories and mount filesystems before pivot_root.
///
/// Mounts a fresh procfs (now possible with PID namespace) and
/// bind-mounts /dev/kvm for VMM operation.
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

    // Mount fresh procfs in the jail.
    // This is now possible because we have a matching PID namespace (via fork).
    let jail_proc = jail.join("proc");
    mount(
        Some("proc"),
        &jail_proc,
        Some("proc"),
        MsFlags::MS_NOSUID | MsFlags::MS_NODEV | MsFlags::MS_NOEXEC,
        None::<&str>,
    )
    .map_err(|e| RunnerError::Jail(format!("failed to mount procfs in jail: {e}")))?;

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
