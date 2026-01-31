use crate::error::RunnerError;

/// Run the benchmark runner.
///
/// This is the main entry point for the runner binary.
/// For now, this is a placeholder that will be expanded
/// to handle the full benchmark execution pipeline:
///
/// 1. Parse configuration
/// 2. Unpack OCI image to directory
/// 3. Create squashfs rootfs from directory
/// 4. Boot VM with kernel and rootfs
/// 5. Collect benchmark results via serial output
/// 6. Return results
pub async fn run() -> Result<(), RunnerError> {
    println!("Bencher Runner starting...");
    println!("Pipeline: OCI -> Rootfs -> VMM -> Results");
    println!();
    println!("This runner requires Linux with KVM support.");
    println!("Use `bencher_runner::execute()` with a Config to run benchmarks.");

    Ok(())
}

/// Execute a single benchmark run with the given configuration.
///
/// # Arguments
///
/// * `config` - The benchmark run configuration
///
/// # Returns
///
/// The benchmark results as a string (serial output from the VM).
#[cfg(target_os = "linux")]
pub async fn execute(config: &crate::Config) -> Result<String, RunnerError> {
    use camino::Utf8Path;

    println!("Executing benchmark run:");
    println!("  OCI image: {}", config.oci_image);
    println!("  Kernel: {}", config.kernel);
    println!("  vCPUs: {}", config.vcpus);
    println!("  Memory: {} MiB", config.memory_mib);

    // Create a temporary work directory
    let work_dir = Utf8Path::new("/tmp/bencher-runner");
    std::fs::create_dir_all(work_dir)?;

    let unpack_dir = work_dir.join("rootfs");
    let rootfs_path = work_dir.join("rootfs.squashfs");

    // Step 1: Unpack OCI image
    println!("Unpacking OCI image to {unpack_dir}...");
    bencher_oci::unpack(&config.oci_image, &unpack_dir)?;

    // Step 2: Create squashfs rootfs
    println!("Creating squashfs at {rootfs_path}...");
    bencher_rootfs::create_squashfs(&unpack_dir, &rootfs_path)?;

    // Step 3: Boot VM and run benchmark
    println!("Booting VM...");
    let vm_config = bencher_vmm::VmConfig {
        kernel_path: config.kernel.clone(),
        rootfs_path,
        vcpus: config.vcpus,
        memory_mib: config.memory_mib,
        kernel_cmdline: config.kernel_cmdline.clone(),
    };

    let results = bencher_vmm::run_vm(&vm_config)?;

    // Clean up
    let _ = std::fs::remove_dir_all(work_dir);

    Ok(results)
}

/// Execute a single benchmark run (non-Linux stub).
#[cfg(not(target_os = "linux"))]
pub async fn execute(_config: &crate::Config) -> Result<String, RunnerError> {
    Err(RunnerError::Config(
        "Benchmark execution requires Linux with KVM support".to_owned(),
    ))
}
