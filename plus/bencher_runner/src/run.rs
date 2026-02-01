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
    println!("  Kernel: {}", config.kernel);
    println!("  vCPUs: {}", config.vcpus);
    println!("  Memory: {} MiB", config.memory_mib);

    // Create a temporary work directory
    let work_dir = Utf8Path::new("/tmp/bencher-runner");
    fs::create_dir_all(work_dir)?;

    let unpack_dir = work_dir.join("rootfs");
    let rootfs_path = work_dir.join("rootfs.squashfs");
    let vsock_path = work_dir.join("vsock.sock");

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

    // Step 4: Write command config for init to read
    println!("Writing command config...");
    write_command_config(&unpack_dir, &command, workdir, &env)?;

    // Step 5: Create squashfs rootfs
    println!("Creating squashfs at {rootfs_path}...");
    bencher_rootfs::create_squashfs(&unpack_dir, &rootfs_path)?;

    // Step 6: Boot VM and run benchmark
    println!("Booting VM with vsock at {vsock_path}...");
    let vm_config = bencher_vmm::VmConfig {
        kernel_path: config.kernel.clone(),
        rootfs_path,
        vcpus: config.vcpus,
        memory_mib: config.memory_mib,
        kernel_cmdline: config.kernel_cmdline.clone(),
        vsock_path: Some(vsock_path.clone()),
    };

    let results = bencher_vmm::run_vm(&vm_config)?;

    // Clean up
    drop(fs::remove_dir_all(work_dir));

    Ok(results)
}

/// Write the command configuration files for the init script.
#[cfg(target_os = "linux")]
fn write_command_config(
    rootfs: &camino::Utf8Path,
    command: &[String],
    workdir: &str,
    env: &[(String, String)],
) -> Result<(), RunnerError> {
    use std::fs;

    let config_dir = rootfs.join("etc/bencher");
    fs::create_dir_all(&config_dir)?;

    // Write the command as a shell-escaped string
    let command_str = command
        .iter()
        .map(|arg| shell_escape(arg))
        .collect::<Vec<_>>()
        .join(" ");
    fs::write(config_dir.join("command"), command_str)?;

    // Write working directory
    if !workdir.is_empty() && workdir != "/" {
        fs::write(config_dir.join("workdir"), workdir)?;
    }

    // Write environment as shell exports
    if !env.is_empty() {
        use std::fmt::Write as _;
        let mut env_script = String::new();
        for (k, v) in env {
            writeln!(env_script, "export {k}={}", shell_escape(v))
                .expect("writing to String cannot fail");
        }
        fs::write(config_dir.join("env"), env_script)?;
    }

    Ok(())
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
