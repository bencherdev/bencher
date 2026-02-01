#![expect(clippy::print_stdout)]

use camino::{Utf8Path, Utf8PathBuf};

use crate::error::RunnerError;

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
    write_init_script(&unpack_dir, &command, workdir, &env, config.output_file.as_deref())?;

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

/// Write the init script and supporting files for the VM.
///
/// This creates:
/// - `/etc/bencher/init` - The main init script that runs the benchmark
/// - The init script buffers stdout/stderr, captures exit code, and sends all via vsock
#[cfg(target_os = "linux")]
fn write_init_script(
    rootfs: &camino::Utf8Path,
    command: &[String],
    workdir: &str,
    env: &[(String, String)],
    output_file: Option<&str>,
) -> Result<(), RunnerError> {
    use std::fs;
    use std::os::unix::fs::PermissionsExt as _;

    let config_dir = rootfs.join("etc/bencher");
    fs::create_dir_all(&config_dir)?;

    // Build the command as a shell-escaped string
    let command_str = command
        .iter()
        .map(|arg| shell_escape(arg))
        .collect::<Vec<_>>()
        .join(" ");

    // Build environment exports
    let mut env_exports = String::new();
    for (k, v) in env {
        use std::fmt::Write as _;
        writeln!(env_exports, "export {k}={}", shell_escape(v))
            .expect("writing to String cannot fail");
    }

    // Add BENCHER_OUTPUT_FILE if specified
    if let Some(path) = output_file {
        use std::fmt::Write as _;
        writeln!(env_exports, "export BENCHER_OUTPUT_FILE={}", shell_escape(path))
            .expect("writing to String cannot fail");
    }

    // Generate the init script
    // This script:
    // 1. Sets up /run/bencher for temporary files
    // 2. Changes to the working directory
    // 3. Exports environment variables
    // 4. Runs the command, buffering stdout/stderr to files
    // 5. Captures the exit code
    // 6. Sends all results via vsock after the command completes
    let init_script = generate_init_script(&command_str, workdir, &env_exports, output_file);

    let init_path = config_dir.join("init");
    fs::write(&init_path, init_script)?;

    // Make the script executable
    let mut perms = fs::metadata(&init_path)?.permissions();
    perms.set_mode(0o755);
    fs::set_permissions(&init_path, perms)?;

    Ok(())
}

/// Generate the init script content.
#[cfg(target_os = "linux")]
fn generate_init_script(
    command: &str,
    workdir: &str,
    env_exports: &str,
    output_file: Option<&str>,
) -> String {
    // Vsock ports (matching bencher_vmm::ports)
    const PORT_STDOUT: u32 = 5000;
    const PORT_STDERR: u32 = 5001;
    const PORT_EXIT_CODE: u32 = 5002;
    const PORT_OUTPUT_FILE: u32 = 5005;

    let workdir_cmd = if workdir.is_empty() || workdir == "/" {
        String::new()
    } else {
        format!("cd {}\n", shell_escape(workdir))
    };

    let output_file_send = if output_file.is_some() {
        format!(
            r#"
# Send output file if it exists
if [ -n "$BENCHER_OUTPUT_FILE" ] && [ -f "$BENCHER_OUTPUT_FILE" ]; then
    /usr/bin/vsock-send --port {PORT_OUTPUT_FILE} "$BENCHER_OUTPUT_FILE"
fi
"#
        )
    } else {
        String::new()
    };

    format!(
        r#"#!/bin/sh
# Bencher VM init script
# This script runs the benchmark and sends results to the host via vsock

set -e

# Create temporary directory for buffering output
mkdir -p /run/bencher

# Set up environment
{env_exports}
# Change to working directory
{workdir_cmd}
# Run the benchmark, buffering stdout and stderr
# Use a subshell to capture the exit code properly
set +e
({command}) >/run/bencher/stdout 2>/run/bencher/stderr
EXIT_CODE=$?
set -e

# Write exit code to file
echo "$EXIT_CODE" >/run/bencher/exit_code

# Send results to host via vsock
# stdout on port {PORT_STDOUT}
/usr/bin/vsock-send --port {PORT_STDOUT} /run/bencher/stdout

# stderr on port {PORT_STDERR}
/usr/bin/vsock-send --port {PORT_STDERR} /run/bencher/stderr

# exit code on port {PORT_EXIT_CODE}
/usr/bin/vsock-send --port {PORT_EXIT_CODE} /run/bencher/exit_code
{output_file_send}
# Shutdown the VM
poweroff -f
"#
    )
}

/// Simple shell escaping for arguments.
#[cfg(target_os = "linux")]
fn shell_escape(s: &str) -> String {
    if s.chars()
        .all(|c| c.is_alphanumeric() || c == '_' || c == '-' || c == '/' || c == '.')
    {
        s.to_owned()
    } else {
        format!("'{}'", s.replace('\'', "'\"'\"'"))
    }
}

/// Execute a single benchmark run (non-Linux stub).
#[cfg(not(target_os = "linux"))]
pub async fn execute(_config: &crate::Config) -> Result<String, RunnerError> {
    Err(RunnerError::Config(
        "Benchmark execution requires Linux with KVM support".to_owned(),
    ))
}
