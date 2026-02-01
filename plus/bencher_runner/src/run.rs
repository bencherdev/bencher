#![expect(clippy::print_stdout)]

use camino::{Utf8Path, Utf8PathBuf};

use crate::error::RunnerError;

/// Arguments for the `run` subcommand.
#[derive(Debug, Clone)]
pub struct RunArgs {
    /// OCI image (local path or registry reference).
    pub image: String,
    /// JWT token for registry authentication.
    pub token: Option<String>,
    /// Number of vCPUs.
    pub vcpus: u8,
    /// Memory in MiB.
    pub memory_mib: u32,
    /// Execution timeout in seconds.
    pub timeout_secs: u64,
    /// Output file path inside guest.
    pub output_file: Option<String>,
}

/// Run the `run` subcommand with parsed arguments.
///
/// This prepares the jail environment and execs to the `vmm` subcommand.
#[cfg(target_os = "linux")]
pub async fn run_with_args(args: &RunArgs) -> Result<(), RunnerError> {
    // Build config from args
    let config = crate::Config {
        oci_image: args.image.clone(),
        token: args.token.clone(),
        kernel: None,
        vcpus: args.vcpus,
        memory_mib: args.memory_mib,
        kernel_cmdline: "console=ttyS0 reboot=k panic=1 pci=off root=/dev/vda ro".to_owned(),
        timeout_secs: args.timeout_secs,
        output_file: args.output_file.clone(),
    };

    // Prepare the environment (creates rootfs, writes init script, etc.)
    // then exec to the vmm subcommand
    exec_to_vmm(&config).await
}

/// Prepare the jail environment and exec to the vmm subcommand.
///
/// This function:
/// 1. Creates a temporary work directory (jail root)
/// 2. Resolves OCI image (local or registry)
/// 3. Creates squashfs rootfs
/// 4. Writes kernel to jail root
/// 5. Execs to `bencher-runner vmm` with the jail root
#[cfg(target_os = "linux")]
async fn exec_to_vmm(config: &crate::Config) -> Result<(), RunnerError> {
    use std::fs;
    use std::os::unix::process::CommandExt as _;
    use std::process::Command;

    println!("Preparing benchmark run:");
    println!("  OCI image: {}", config.oci_image);
    println!("  vCPUs: {}", config.vcpus);
    println!("  Memory: {} MiB", config.memory_mib);
    println!("  Timeout: {} seconds", config.timeout_secs);

    // Clean up any stale jail directories from previous runs
    cleanup_stale_jails();

    // Create a persistent work directory for the jail
    // We use a directory under /tmp that won't be cleaned up when we exec
    let run_id = uuid::Uuid::new_v4();
    let jail_root = Utf8PathBuf::from(format!("/tmp/bencher-runner/{run_id}"));
    fs::create_dir_all(&jail_root)?;

    let unpack_dir = jail_root.join("unpack");
    let rootfs_path = Utf8PathBuf::from("/rootfs.squashfs"); // Path inside jail after pivot_root
    let kernel_path = Utf8PathBuf::from("/vmlinux"); // Path inside jail after pivot_root
    let vsock_path = Utf8PathBuf::from("/vsock.sock"); // Path inside jail after pivot_root

    // Actual paths on host filesystem (in jail_root)
    let host_rootfs_path = jail_root.join("rootfs.squashfs");
    let host_kernel_path = jail_root.join("vmlinux");
    let host_vsock_path = jail_root.join("vsock.sock");

    // Step 1: Resolve OCI image (local path or pull from registry)
    let cache_dir = config.cache_dir();
    let oci_image_path =
        resolve_oci_image(&config.oci_image, config.token.as_deref(), &cache_dir).await?;

    // Step 2: Parse OCI image config to get the command
    println!("Parsing OCI image config...");
    let oci_image = bencher_oci::OciImage::parse(&oci_image_path)?;
    let command = oci_image.command();
    let workdir = oci_image.working_dir().unwrap_or("/");
    let env = oci_image.env();

    if command.is_empty() {
        // Clean up on error
        let _ = fs::remove_dir_all(&jail_root);
        return Err(RunnerError::Config(
            "OCI image has no CMD or ENTRYPOINT set".to_owned(),
        ));
    }

    println!("  Command: {}", command.join(" "));
    println!("  WorkDir: {workdir}");
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
        workdir,
        &env,
        config.output_file.as_deref(),
    )?;

    // Step 5: Copy bencher-init binary to /init
    println!("Installing init binary...");
    install_init_binary(&unpack_dir)?;

    // Step 6: Create squashfs rootfs
    println!("Creating squashfs at {host_rootfs_path}...");
    bencher_rootfs::create_squashfs(&unpack_dir, &host_rootfs_path)?;

    // Clean up the unpack directory - we only need the squashfs now
    let _ = fs::remove_dir_all(&unpack_dir);

    // Step 7: Write bundled kernel to jail root
    println!("Writing bundled kernel to {host_kernel_path}...");
    bencher_vmm::write_kernel_to_file(host_kernel_path.as_std_path())?;

    // Step 8: Get the path to ourselves for exec
    let exe_path = std::env::current_exe().map_err(|e| {
        let _ = fs::remove_dir_all(&jail_root);
        RunnerError::Io(e)
    })?;

    // Step 9: Build arguments for vmm subcommand
    let vmm_args = vec![
        "vmm".to_owned(),
        "--jail-root".to_owned(),
        jail_root.to_string(),
        "--kernel".to_owned(),
        kernel_path.to_string(),
        "--rootfs".to_owned(),
        rootfs_path.to_string(),
        "--vsock".to_owned(),
        vsock_path.to_string(),
        "--vcpus".to_owned(),
        config.vcpus.to_string(),
        "--memory".to_owned(),
        config.memory_mib.to_string(),
        "--timeout".to_owned(),
        config.timeout_secs.to_string(),
    ];

    println!("Exec'ing to vmm subcommand...");
    println!("  Jail root: {jail_root}");

    // Exec to ourselves with vmm subcommand
    // This replaces our process, so we don't return
    let err = Command::new(&exe_path).args(&vmm_args).exec();

    // If we get here, exec failed
    let _ = fs::remove_dir_all(&jail_root);
    Err(RunnerError::Io(err))
}

/// Non-Linux stub for `run_with_args`.
#[cfg(not(target_os = "linux"))]
pub async fn run_with_args(_args: &RunArgs) -> Result<(), RunnerError> {
    Err(RunnerError::Config(
        "bencher-runner requires Linux".to_owned(),
    ))
}

/// Run the benchmark runner.
///
/// This is the main entry point for the runner binary.
/// For now, this is a placeholder that will be expanded
/// to handle the full benchmark execution pipeline:
///
/// 1. Parse configuration
/// 2. Resolve OCI image (local or pull from registry)
/// 3. Unpack OCI image to directory
/// 4. Create squashfs rootfs from directory
/// 5. Boot VM with kernel and rootfs
/// 6. Collect benchmark results via serial output
/// 7. Return results
pub async fn run() -> Result<(), RunnerError> {
    println!("Bencher Runner starting...");
    println!("Pipeline: OCI (local or registry) -> Rootfs -> VMM -> Results");
    println!();
    println!("This runner requires Linux with KVM support.");
    println!("Use `bencher_runner::execute()` with a Config to run benchmarks.");

    Ok(())
}

/// Resolve an OCI image source to a local path.
///
/// If the source is a local path that exists, returns it directly.
/// If the source looks like a registry reference, pulls from the registry.
///
/// # Arguments
///
/// * `oci_image` - Local path or registry reference
/// * `token` - Optional JWT token for registry authentication
/// * `cache_dir` - Directory to cache pulled images
///
/// # Returns
///
/// Path to the local OCI image directory.
pub async fn resolve_oci_image(
    oci_image: &str,
    token: Option<&str>,
    cache_dir: &Utf8Path,
) -> Result<Utf8PathBuf, RunnerError> {
    let path = Utf8Path::new(oci_image);

    // If it's a local path that exists, use it directly
    if path.exists() {
        println!("Using local OCI image: {oci_image}");
        return Ok(path.to_owned());
    }

    // Otherwise, treat as a registry reference
    println!("Parsing registry reference: {oci_image}");
    let image_ref = bencher_oci::ImageReference::parse(oci_image)?;

    // Create a cache path based on the image reference
    // Replace characters that aren't filesystem-safe
    let cache_name = image_ref.full_name().replace(['/', ':', '@'], "_");
    let image_cache = cache_dir.join(&cache_name);

    // Check if already cached
    if image_cache.exists() {
        println!("Using cached image: {image_cache}");
        return Ok(image_cache);
    }

    // Pull from registry
    println!("Pulling from registry: {}", image_ref.full_name());

    // Create cache directory if it doesn't exist
    std::fs::create_dir_all(cache_dir)?;

    let mut client = if let Some(t) = token {
        println!("  Using authenticated client");
        bencher_oci::RegistryClient::with_token(t.to_owned())?
    } else {
        println!("  Using anonymous client");
        bencher_oci::RegistryClient::new()?
    };

    client.pull(&image_ref, &image_cache).await?;
    println!("Image pulled to: {image_cache}");

    Ok(image_cache)
}

/// Execute a single benchmark run with the given configuration.
///
/// # Arguments
///
/// * `config` - The benchmark run configuration
///
/// # Returns
///
/// The benchmark results as a string (from the VM via vsock or serial).
#[cfg(target_os = "linux")]
pub async fn execute(config: &crate::Config) -> Result<String, RunnerError> {
    use std::fs;

    println!("Executing benchmark run:");
    println!("  OCI image: {}", config.oci_image);
    println!(
        "  Kernel: {}",
        config
            .kernel
            .as_ref()
            .map_or("(bundled)", |p| p.as_str())
    );
    println!("  vCPUs: {}", config.vcpus);
    println!("  Memory: {} MiB", config.memory_mib);
    println!("  Timeout: {} seconds", config.timeout_secs);

    // Create a temporary work directory
    let temp_dir = tempfile::tempdir().map_err(|e| {
        RunnerError::Config(format!("Failed to create temp directory: {e}"))
    })?;
    let work_dir = Utf8Path::from_path(temp_dir.path())
        .ok_or_else(|| RunnerError::Config("Temp directory path is not UTF-8".to_owned()))?;

    let unpack_dir = work_dir.join("rootfs");
    let rootfs_path = work_dir.join("rootfs.squashfs");
    let vsock_path = work_dir.join("vsock.sock");

    // Get kernel path - use provided path or write bundled kernel to temp dir
    let kernel_path = if let Some(ref kernel) = config.kernel {
        kernel.clone()
    } else {
        let bundled_kernel_path = work_dir.join("vmlinux");
        println!("Writing bundled kernel to {bundled_kernel_path}...");
        bencher_vmm::write_kernel_to_file(bundled_kernel_path.as_std_path())?;
        bundled_kernel_path
    };

    // Step 1: Resolve OCI image (local path or pull from registry)
    let cache_dir = config.cache_dir();

    let oci_image_path = resolve_oci_image(
        &config.oci_image,
        config.token.as_deref(),
        &cache_dir,
    )
    .await?;

    // Step 2: Parse OCI image config to get the command
    println!("Parsing OCI image config...");
    let oci_image = bencher_oci::OciImage::parse(&oci_image_path)?;
    let command = oci_image.command();
    let workdir = oci_image.working_dir().unwrap_or("/");
    let env = oci_image.env();

    if command.is_empty() {
        return Err(RunnerError::Config(
            "OCI image has no CMD or ENTRYPOINT set".to_owned(),
        ));
    }

    println!("  Command: {}", command.join(" "));
    println!("  WorkDir: {workdir}");
    if !env.is_empty() {
        println!("  Env: {} variables", env.len());
    }

    // Step 3: Unpack OCI image
    println!("Unpacking OCI image to {unpack_dir}...");
    bencher_oci::unpack(&oci_image_path, &unpack_dir)?;

    // Step 4: Write command config and init script for the VM
    println!("Writing init script...");
    write_init_config(&unpack_dir, &command, workdir, &env, config.output_file.as_deref())?;

    // Step 5: Create squashfs rootfs
    println!("Creating squashfs at {rootfs_path}...");
    bencher_rootfs::create_squashfs(&unpack_dir, &rootfs_path)?;

    // Step 6: Boot VM and run benchmark
    println!("Booting VM with vsock at {vsock_path}...");
    let vm_config = bencher_vmm::VmConfig {
        kernel_path,
        rootfs_path,
        vcpus: config.vcpus,
        memory_mib: config.memory_mib,
        kernel_cmdline: config.kernel_cmdline.clone(),
        vsock_path: Some(vsock_path.clone()),
        timeout_secs: config.timeout_secs,
    };

    let results = bencher_vmm::run_vm(&vm_config)?;

    // temp_dir is automatically cleaned up when dropped
    drop(temp_dir);

    Ok(results)
}

/// Write the init config for the VM.
///
/// This creates `/etc/bencher/config.json` which is read by `bencher-init`.
#[cfg(target_os = "linux")]
fn write_init_config(
    rootfs: &camino::Utf8Path,
    command: &[String],
    workdir: &str,
    env: &[(String, String)],
    output_file: Option<&str>,
) -> Result<(), RunnerError> {
    use std::fs;

    let config_dir = rootfs.join("etc/bencher");
    fs::create_dir_all(&config_dir)?;

    // Build the config JSON
    let config = serde_json::json!({
        "command": command,
        "workdir": workdir,
        "env": env,
        "output_file": output_file,
    });

    let config_path = config_dir.join("config.json");
    let config_str = serde_json::to_string_pretty(&config)
        .map_err(|e| RunnerError::Config(format!("failed to serialize config: {e}")))?;
    fs::write(&config_path, config_str)?;

    Ok(())
}

/// Install the bencher-init binary into the rootfs at /init.
///
/// Uses the bundled init binary if available, otherwise falls back to searching on disk.
#[cfg(target_os = "linux")]
fn install_init_binary(rootfs: &camino::Utf8Path) -> Result<(), RunnerError> {
    use crate::init;

    let dest_path = rootfs.join("init");

    if init::INIT_BUNDLED {
        // Use the bundled init binary
        init::write_init_to_file(dest_path.as_std_path())?;
    } else {
        // Fall back to searching for the binary on disk
        let init_binary = find_init_binary()?;

        std::fs::copy(&init_binary, &dest_path).map_err(|e| {
            RunnerError::Config(format!(
                "failed to copy init binary from {} to {}: {e}",
                init_binary.display(),
                dest_path
            ))
        })?;

        // Make it executable
        use std::os::unix::fs::PermissionsExt as _;
        let mut perms = std::fs::metadata(&dest_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&dest_path, perms)?;
    }

    Ok(())
}

/// Find the bencher-init binary on disk (fallback when not bundled).
#[cfg(target_os = "linux")]
fn find_init_binary() -> Result<std::path::PathBuf, RunnerError> {
    // Look in these locations in order
    let candidates = [
        // Next to the current executable
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("bencher-init"))),
        // Common installation paths
        Some(std::path::PathBuf::from("/usr/local/bin/bencher-init")),
        Some(std::path::PathBuf::from("/usr/bin/bencher-init")),
    ];

    for candidate in candidates.into_iter().flatten() {
        if candidate.exists() {
            return Ok(candidate);
        }
    }

    Err(RunnerError::Config(
        "bencher-init binary not found. Build with: cargo build -p bencher_init".to_owned(),
    ))
}

/// Base directory for jail roots.
#[cfg(target_os = "linux")]
const JAIL_BASE_DIR: &str = "/tmp/bencher-runner";

/// Clean up stale jail directories from previous runs.
///
/// This removes any leftover directories in `/tmp/bencher-runner/` that were
/// not cleaned up due to the pivot_root architecture (we can't clean up after
/// ourselves once we've pivoted into the jail).
#[cfg(target_os = "linux")]
fn cleanup_stale_jails() {
    let base_dir = std::path::Path::new(JAIL_BASE_DIR);

    if !base_dir.exists() {
        return;
    }

    let entries = match std::fs::read_dir(base_dir) {
        Ok(entries) => entries,
        Err(e) => {
            eprintln!("Warning: failed to read {JAIL_BASE_DIR}: {e}");
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();

        // Only clean up directories (each run creates a UUID-named directory)
        if !path.is_dir() {
            continue;
        }

        // Try to remove the directory and its contents
        if let Err(e) = std::fs::remove_dir_all(&path) {
            eprintln!(
                "Warning: failed to clean up stale jail {}: {e}",
                path.display()
            );
        } else {
            println!("Cleaned up stale jail: {}", path.display());
        }
    }
}

/// Execute a single benchmark run (non-Linux stub).
#[cfg(not(target_os = "linux"))]
pub async fn execute(_config: &crate::Config) -> Result<String, RunnerError> {
    Err(RunnerError::Config(
        "Benchmark execution requires Linux with KVM support".to_owned(),
    ))
}
