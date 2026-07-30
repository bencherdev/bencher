//! Linux VM execution — runs benchmarks in Firecracker microVMs.

#![expect(clippy::print_stdout, reason = "VM executor prints progress output")]

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use camino::{Utf8Path, Utf8PathBuf};

use crate::error::RunnerError;
use crate::jail::{
    HostPreparation, JailDir, JailLock, JailPaths, ReclaimFailed, StateDir, VmId, chroot, netns,
    state,
};
use crate::run::{RunOutput, prepare_oci_workspace};

/// Execute a single benchmark run in a jailed Firecracker microVM.
pub fn vm_execute(
    config: &crate::Config,
    host: &mut HostPreparation,
    cancel_flag: Option<&Arc<AtomicBool>>,
) -> Result<RunOutput, RunnerError> {
    use crate::firecracker::run_firecracker;

    println!("Executing benchmark run:");
    println!("  OCI image: {}", config.oci_image);
    println!(
        "  Kernel: {}",
        config.kernel.as_ref().map_or("(system)", |p| p.as_str())
    );
    println!("  vCPUs: {}", config.vcpus);
    println!("  Memory: {} MiB", config.memory.to_mib());
    println!("  Timeout: {} seconds", config.timeout_secs);

    let state_dir = StateDir::new(config.state_dir.clone());

    // Prepare the host on demand, before the first jail this process builds.
    // Must come before the job lock is taken: preparation takes the same lock,
    // and `flock` is per open file description, so nesting would block on
    // itself. It is also the cheap check, so a host that cannot jail at all
    // fails here rather than after pulling an image.
    host.ensure(state_dir.path(), config.jail_user)?;

    // Everything that does not touch the jail happens before the lock. The
    // image pull and unpack are the slow part of a job and need nothing from
    // the chroot, so holding an exclusive per-state-directory lock across them
    // would serialize concurrent `runner run` invocations on the download
    // rather than on the jail.
    let workspace = prepare_oci_workspace(config)?;
    let work_dir = &workspace.work_dir;
    let unpack_dir = &workspace.unpack_dir;
    let oci_config = workspace.oci_config;

    let command = oci_config.command;
    let working_dir = &oci_config.working_dir;
    let env = oci_config.env;

    // Write command config for the VM
    println!("Writing init config...");
    write_init_config(
        unpack_dir,
        &command,
        working_dir,
        &env,
        config.file_paths.as_deref(),
        config.max_output_size,
    )?;

    // Step 5: Install init binary
    println!("Installing init binary...");
    install_init_binary(unpack_dir)?;

    // Held from here to the end of the job. Another runner's sweep removes
    // every chroot it finds, so it must not run while this one is live.
    // Declared before the jail guard so the lock outlives the teardown it
    // protects.
    let _lock = JailLock::acquire(state_dir.path())?;

    // Rebuilt per job rather than once per daemon lifetime: the handle lives
    // on a tmpfs and is operator visible, so it has to be self-healing.
    let netns = netns::ensure()?;

    // The jail root is a function of the VM id, and the job's artifacts are
    // built inside it rather than copied in afterwards, so the id is minted
    // before any of them exist. Dropping this guard removes the chroot tree,
    // which is what the workspace temp directory used to cover.
    let vm_id = VmId::new();
    let jail_dir = JailDir::create(&state_dir, &vm_id, host.reclaim_signal())?;
    let jail = JailPaths::new(jail_dir.root())?;
    println!("  Jail: {}", jail.root());

    // Everything Firecracker reads has to be inside the chroot, so the kernel
    // lands in the jail root whatever its source: bundled, supplied by the
    // job, or found on the host.
    let kernel_dest = jail.kernel().host().as_path();
    if let Some(kernel) = &config.kernel {
        println!("  Copying the job's kernel into the jail...");
        copy_file(kernel, kernel_dest)?;
    } else if crate::kernel::KERNEL_BUNDLED {
        crate::kernel::write_kernel_to_file(kernel_dest)?;
        println!("  Extracted bundled kernel into the jail at {kernel_dest}");
    } else {
        println!("  Copying the host's kernel into the jail...");
        copy_file(&find_kernel()?, kernel_dest)?;
    }

    // Step 6: Create the ext4 rootfs directly in the jail root
    let rootfs_dest = jail.rootfs().host().as_path();
    println!(
        "Creating ext4 at {rootfs_dest} ({} MiB)...",
        config.disk.to_mib()
    );
    bencher_rootfs::create_ext4_with_size(unpack_dir, rootfs_dest, config.disk.to_mib())?;

    // The jailer chowns the chroot root and the device nodes it creates, but
    // not what the runner placed inside, so each artifact is handed over
    // explicitly. The rootfs is written by Firecracker and is given away; the
    // kernel is only read, so it stays owned by root and merely becomes
    // readable.
    chroot::chown_to_jail(rootfs_dest, config.jail_user)?;
    chroot::grant_jail_read(kernel_dest)?;

    // Step 7-8: Build Firecracker config and run the microVM
    let fc_config = build_firecracker_config(
        config,
        work_dir,
        vm_id,
        &state_dir,
        jail,
        netns,
        host.reclaim_signal(),
    )?;

    let run_output = run_firecracker(&fc_config, cancel_flag)?;

    Ok(run_output)
}

/// Build the Firecracker job config: stage the binaries and convert types.
fn build_firecracker_config(
    config: &crate::Config,
    work_dir: &Utf8Path,
    vm_id: VmId,
    state_dir: &StateDir,
    jail: JailPaths,
    netns: Utf8PathBuf,
    reclaim_failed: ReclaimFailed,
) -> Result<crate::firecracker::FirecrackerJobConfig, RunnerError> {
    // The jailer copies `--exec-file` into the chroot itself and rejects a
    // multiply linked file, so Firecracker is staged outside the jail and is
    // never placed in the chroot by hand or hardlinked. Its base name is what
    // the jailer derives the chroot layout from, so it is fixed.
    let firecracker_bin = work_dir.join(state::EXEC_FILE_NAME);
    if crate::firecracker_bin::FIRECRACKER_BUNDLED {
        crate::firecracker_bin::write_firecracker_to_file(&firecracker_bin)?;
        println!("  Extracted bundled firecracker to {firecracker_bin}");
    } else {
        copy_binary(&find_firecracker_binary()?, &firecracker_bin)?;
    }

    // The jailer runs outside the chroot and is never copied into it, so it
    // can be used wherever it is found.
    let jailer_bin = if crate::jailer_bin::JAILER_BUNDLED {
        let jailer_dest = work_dir.join("jailer");
        crate::jailer_bin::write_jailer_to_file(&jailer_dest)?;
        println!("  Extracted bundled jailer to {jailer_dest}");
        jailer_dest
    } else {
        find_jailer_binary()?
    };

    println!("Launching jailed Firecracker microVM...");
    let vcpus = u8::try_from(u32::from(config.vcpus)).map_err(|_err| {
        crate::error::ConfigError::OutOfRange {
            name: "vCPU count",
            value: config.vcpus.to_string(),
            range: "0-255",
        }
    })?;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "Practical memory fits in u32 MiB for Firecracker"
    )]
    let memory_mib = config.memory.to_mib() as u32;

    Ok(crate::firecracker::FirecrackerJobConfig {
        firecracker_bin,
        jailer_bin,
        vm_id,
        jail,
        jail_user: config.jail_user,
        chroot_base_dir: state_dir.chroot_base(),
        netns,
        reclaim_failed,
        vcpus,
        memory_mib,
        boot_args: config.kernel_cmdline.clone(),
        timeout_secs: config.timeout_secs,
        cpu_layout: config.cpu_layout.clone(),
        log_level: config.sandbox_log_level,
        max_file_count: config.max_file_count,
        max_content_size: config.max_content_size,
        max_output_size: config.max_output_size,
        grace_period: config.grace_period,
    })
}

/// Copy a file the job needs to a path the runner controls.
///
/// Used both for artifacts placed inside the chroot and for binaries staged
/// outside it, so the message says where the copy landed rather than claiming
/// a destination it does not know about.
fn copy_file(src: &Utf8Path, dest: &Utf8Path) -> Result<(), RunnerError> {
    std::fs::copy(src, dest).map_err(|e| crate::error::ConfigError::CopyFile {
        src: src.to_owned(),
        dest: dest.to_owned(),
        source: e,
    })?;
    println!("  Copied {src} to {dest}");
    Ok(())
}

/// Stage an executable found on the host, preserving its executable bit.
fn copy_binary(src: &Utf8Path, dest: &Utf8Path) -> Result<(), RunnerError> {
    use std::os::unix::fs::PermissionsExt as _;

    copy_file(src, dest)?;
    let mut perms = std::fs::metadata(dest)?.permissions();
    perms.set_mode(0o755);
    std::fs::set_permissions(dest, perms)?;
    Ok(())
}

/// Write the init config for the VM.
///
/// This creates `/etc/bencher/config.json` which is read by `bencher-init`.
fn write_init_config(
    rootfs: &Utf8Path,
    command: &[String],
    workdir: &str,
    env: &[(String, String)],
    file_paths: Option<&[Utf8PathBuf]>,
    max_output_size: usize,
) -> Result<(), RunnerError> {
    use std::fs;

    let config_dir = rootfs.join("etc/bencher");
    fs::create_dir_all(&config_dir)?;

    // Build the config JSON
    let config = serde_json::json!({
        "command": command,
        "workdir": workdir,
        "env": env,
        "file_paths": file_paths,
        "max_output_size": max_output_size,
    });

    let config_path = config_dir.join("config.json");
    let config_str =
        serde_json::to_string_pretty(&config).map_err(crate::error::ConfigError::Serialize)?;
    fs::write(&config_path, config_str)?;

    Ok(())
}

/// Install the bencher-init binary into the rootfs at /init.
///
/// Uses the bundled init binary if available, otherwise falls back to searching on disk.
fn install_init_binary(rootfs: &Utf8Path) -> Result<(), RunnerError> {
    use crate::init;
    use std::os::unix::fs::PermissionsExt as _;

    let dest_path = rootfs.join("init");

    if init::INIT_BUNDLED {
        // Use the bundled init binary
        init::write_init_to_file(&dest_path)?;
    } else {
        // Fall back to searching for the binary on disk
        let init_binary = find_init_binary()?;

        std::fs::copy(&init_binary, &dest_path).map_err(|e| {
            crate::error::ConfigError::CopyInit {
                src: init_binary.clone(),
                dest: dest_path.clone(),
                source: e,
            }
        })?;

        // Make it executable
        let mut perms = std::fs::metadata(&dest_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&dest_path, perms)?;
    }

    Ok(())
}

/// Find the bencher-init binary on disk (fallback when not bundled).
fn find_init_binary() -> Result<Utf8PathBuf, RunnerError> {
    // Look in these locations in order
    let candidates = [
        // Next to the current executable
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("bencher-init")))
            .and_then(|p| Utf8PathBuf::try_from(p).ok()),
        // Common installation paths
        Some(Utf8PathBuf::from("/usr/local/bin/bencher-init")),
        Some(Utf8PathBuf::from("/usr/bin/bencher-init")),
    ];

    for candidate in candidates.into_iter().flatten() {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(crate::error::ConfigError::BinaryNotFound {
        name: "bencher-init".to_owned(),
        hint: "Build with: cargo build -p bencher_init".to_owned(),
    }
    .into())
}

/// Find the Firecracker binary on the system.
fn find_firecracker_binary() -> Result<Utf8PathBuf, RunnerError> {
    let candidates = [
        // Next to the current executable
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("firecracker")))
            .and_then(|p| Utf8PathBuf::try_from(p).ok()),
        // Common installation paths
        Some(Utf8PathBuf::from("/usr/local/bin/firecracker")),
        Some(Utf8PathBuf::from("/usr/bin/firecracker")),
    ];

    for candidate in candidates.into_iter().flatten() {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(crate::error::ConfigError::BinaryNotFound {
        name: "firecracker".to_owned(),
        hint: "Install from: https://github.com/firecracker-microvm/firecracker/releases"
            .to_owned(),
    }
    .into())
}

/// Find the jailer binary on the system (fallback when not bundled).
fn find_jailer_binary() -> Result<Utf8PathBuf, RunnerError> {
    let candidates = [
        // Next to the current executable
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("jailer")))
            .and_then(|p| Utf8PathBuf::try_from(p).ok()),
        // Common installation paths
        Some(Utf8PathBuf::from("/usr/local/bin/jailer")),
        Some(Utf8PathBuf::from("/usr/bin/jailer")),
    ];

    for candidate in candidates.into_iter().flatten() {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(crate::error::ConfigError::BinaryNotFound {
        name: "jailer".to_owned(),
        hint: "Install from: https://github.com/firecracker-microvm/firecracker/releases"
            .to_owned(),
    }
    .into())
}

/// Find the kernel image on the system.
fn find_kernel() -> Result<Utf8PathBuf, RunnerError> {
    let candidates = [
        // Bencher's shared location
        "/usr/local/share/bencher/vmlinux",
        // Next to the current executable
    ];

    for candidate in candidates {
        if Utf8Path::new(candidate).exists() {
            return Ok(Utf8PathBuf::from(candidate));
        }
    }

    // Try next to the current executable
    if let Ok(exe) = std::env::current_exe()
        && let Some(parent) = exe.parent()
    {
        let kernel = parent.join("vmlinux");
        if kernel.exists()
            && let Some(path) = kernel.to_str()
        {
            return Ok(Utf8PathBuf::from(path));
        }
    }

    Err(crate::error::ConfigError::BinaryNotFound {
        name: "vmlinux".to_owned(),
        hint: "Place at /usr/local/share/bencher/vmlinux".to_owned(),
    }
    .into())
}
