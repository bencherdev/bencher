//! Linux VM execution — runs benchmarks in Firecracker microVMs.

#![expect(clippy::print_stdout, reason = "VM executor prints progress output")]

use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use camino::{Utf8Path, Utf8PathBuf};

use crate::error::RunnerError;
use crate::run::{RunOutput, resolve_oci_config, resolve_oci_image};

/// Execute a single benchmark run in a Firecracker microVM.
pub fn vm_execute(
    config: &crate::Config,
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

    // Create a temporary work directory
    let temp_dir = tempfile::tempdir().map_err(crate::error::ConfigError::TempDir)?;
    let work_dir =
        Utf8Path::from_path(temp_dir.path()).ok_or(crate::error::ConfigError::NonUtf8TempDir)?;

    let unpack_dir = work_dir.join("rootfs");
    let rootfs_path = work_dir.join("rootfs.ext4");

    // Get kernel path - use bundled, provided, or find system kernel
    let kernel_path = if let Some(kernel) = &config.kernel {
        kernel.clone()
    } else if crate::kernel::KERNEL_BUNDLED {
        let kernel_dest = work_dir.join("vmlinux");
        crate::kernel::write_kernel_to_file(&kernel_dest)?;
        println!("  Extracted bundled kernel to {kernel_dest}");
        kernel_dest
    } else {
        find_kernel()?
    };

    // Step 1: Resolve OCI image (local path or pull from registry)
    let oci_image_path = resolve_oci_image(
        &config.oci_image,
        config.token.as_ref().map(AsRef::as_ref),
        config.registry_scheme,
        work_dir,
    )?;

    // Step 2: Parse OCI image config to get the command
    println!("Parsing OCI image config...");
    let oci_image = bencher_oci::OciImage::parse(&oci_image_path)?;
    let oci_config = resolve_oci_config(&oci_image, config)?;

    let command = oci_config.command;
    let working_dir = &oci_config.working_dir;
    let env = oci_config.env;

    println!("  Command: {}", command.join(" "));
    println!("  WorkDir: {working_dir}");
    if !env.is_empty() {
        println!("  Env: {} variables", env.len());
    }

    // Step 3: Unpack OCI image
    println!("Unpacking OCI image to {unpack_dir}...");
    bencher_oci::unpack(&oci_image_path, &unpack_dir)?;

    // Step 4: Write command config for the VM
    println!("Writing init config...");
    write_init_config(
        &unpack_dir,
        &command,
        working_dir,
        &env,
        config.file_paths.as_deref(),
        config.max_output_size,
    )?;

    // Step 5: Install init binary
    println!("Installing init binary...");
    install_init_binary(&unpack_dir)?;

    // Step 6: Create ext4 rootfs
    println!(
        "Creating ext4 at {rootfs_path} ({} MiB)...",
        config.disk.to_mib()
    );
    bencher_rootfs::create_ext4_with_size(&unpack_dir, &rootfs_path, config.disk.to_mib())?;

    // Step 7–8: Build Firecracker config and run the microVM
    let fc_config = build_firecracker_config(config, work_dir, kernel_path, rootfs_path)?;

    let run_output = run_firecracker(&fc_config, cancel_flag)?;

    Ok(run_output)
}

/// Build the Firecracker job config: resolve the binary and convert types.
fn build_firecracker_config(
    config: &crate::Config,
    work_dir: &Utf8Path,
    kernel_path: Utf8PathBuf,
    rootfs_path: Utf8PathBuf,
) -> Result<crate::firecracker::FirecrackerJobConfig, RunnerError> {
    let firecracker_bin = if crate::firecracker_bin::FIRECRACKER_BUNDLED {
        let fc_dest = work_dir.join("firecracker");
        crate::firecracker_bin::write_firecracker_to_file(&fc_dest)?;
        println!("  Extracted bundled firecracker to {fc_dest}");
        fc_dest
    } else {
        find_firecracker_binary()?
    };

    println!("Launching Firecracker microVM...");
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
        kernel_path,
        rootfs_path,
        vcpus,
        memory_mib,
        boot_args: config.kernel_cmdline.clone(),
        timeout_secs: config.timeout_secs,
        work_dir: work_dir.to_owned(),
        cpu_layout: config.cpu_layout.clone(),
        log_level: config.firecracker_log_level,
        max_file_count: config.max_file_count,
        max_content_size: config.max_content_size,
        max_output_size: config.max_output_size,
        grace_period: config.grace_period,
    })
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
