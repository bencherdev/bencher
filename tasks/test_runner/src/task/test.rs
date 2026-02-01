//! Test runner orchestration.
//!
//! This module coordinates the full test:
//! 1. Ensures the kernel is available
//! 2. Creates the OCI image
//! 3. Runs the VMM (directly on Linux, via Docker on macOS)

use anyhow::Context as _;
use camino::Utf8PathBuf;

use super::kernel;
use super::oci;
use crate::docker;
use crate::parser::TaskTest;

#[derive(Debug)]
pub struct Test {}

impl TryFrom<TaskTest> for Test {
    type Error = anyhow::Error;

    fn try_from(_test: TaskTest) -> Result<Self, Self::Error> {
        Ok(Self {})
    }
}

impl Test {
    #[expect(clippy::unused_self)]
    pub fn exec(&self) -> anyhow::Result<()> {
        run_test()
    }
}

/// Run the full test.
fn run_test() -> anyhow::Result<()> {
    println!("=== Bencher Runner Integration Test ===");
    println!();

    // Detect platform
    let is_linux = docker::is_linux();
    let has_kvm = is_linux && std::path::Path::new("/dev/kvm").exists();

    println!("Platform: {}", if is_linux { "Linux" } else { "macOS" });
    println!("KVM available: {}", if has_kvm { "Yes" } else { "No" });
    println!();

    if is_linux && has_kvm {
        // Run directly on Linux
        run_test_native()
    } else {
        print!("Checking Docker availability... ");
        std::io::Write::flush(&mut std::io::stdout())?;
        if docker::docker_available() {
            println!("available");
            // Run via Docker on macOS (or Linux without KVM)
            run_test_docker()
        } else {
            println!("not available");
            println!();
            println!("Warning: Neither KVM nor Docker is available.");
            println!("Running in mock mode (no actual VM execution).");
            run_test_mock()
        }
    }
}

/// Run the test natively on Linux with KVM.
fn run_test_native() -> anyhow::Result<()> {
    println!("Running test natively with KVM...");
    println!();

    // Step 1: Ensure kernel is available
    kernel::ensure_kernel()?;
    println!();

    // Step 2: Create OCI image
    oci::create_test_image()?;
    println!();

    // Step 3: Run the benchmark
    run_benchmark()
}

/// Run the test via Docker.
fn run_test_docker() -> anyhow::Result<()> {
    println!("Running test via Docker...");
    println!();

    // Check Docker KVM support
    if docker::docker_kvm_available() {
        println!("Docker has KVM support, running full test...");

        let workspace_root = get_workspace_root();
        let output = docker::run_in_docker(&workspace_root)?;
        println!("{output}");
        Ok(())
    } else {
        println!("Docker does not have KVM support.");
        println!("Running mock test instead...");
        run_test_mock()
    }
}

/// Run a mock test without actual VM execution.
fn run_test_mock() -> anyhow::Result<()> {
    println!("Running mock test...");
    println!();

    // Step 1: Ensure kernel is available
    kernel::ensure_kernel()?;
    println!();

    // Step 2: Create OCI image
    oci::create_test_image()?;
    println!();

    // Step 3: Mock benchmark output (matches `bencher mock` format)
    println!("=== Mock Benchmark Output ===");
    println!(
        r#"{{
  "bencher::mock_0": {{
    "latency": {{
      "value": 4.5535649932187034,
      "lower_value": 4.098208493896833,
      "upper_value": 5.008921492540574
    }}
  }},
  "bencher::mock_1": {{
    "latency": {{
      "value": 16.537506086518523,
      "lower_value": 14.88375547786667,
      "upper_value": 18.191256695170374
    }}
  }},
  "bencher::mock_2": {{
    "latency": {{
      "value": 20.221420814607537,
      "lower_value": 18.199278733146784,
      "upper_value": 22.24356289606829
    }}
  }},
  "bencher::mock_3": {{
    "latency": {{
      "value": 34.92859461603261,
      "lower_value": 31.435735154429352,
      "upper_value": 38.42145407763587
    }}
  }},
  "bencher::mock_4": {{
    "latency": {{
      "value": 42.40432493036204,
      "lower_value": 38.163892437325835,
      "upper_value": 46.64475742339824
    }}
  }}
}}"#
    );
    println!();

    println!("=== Mock Test Complete ===");
    println!();
    println!("Note: This was a mock test. To run the full test:");
    println!("  - On Linux: Ensure /dev/kvm is available");
    println!("  - On macOS: Install Docker Desktop with VirtioFS");
    println!();

    Ok(())
}

/// Run the actual benchmark using `bencher_runner`.
#[tokio::main]
async fn run_benchmark() -> anyhow::Result<()> {
    println!("Starting benchmark VM...");
    println!();

    let kernel = kernel::kernel_path();
    let oci_image = oci::oci_image_path();

    // Unpack OCI image
    let work_dir = super::work_dir();
    let unpack_dir = work_dir.join("unpack");
    let rootfs_squashfs = work_dir.join("rootfs.squashfs");

    if unpack_dir.exists() {
        std::fs::remove_dir_all(&unpack_dir)?;
    }

    println!("Unpacking OCI image...");
    bencher_oci::unpack(&oci_image, &unpack_dir).context("Failed to unpack OCI image")?;

    println!("Creating squashfs...");
    bencher_rootfs::create_squashfs(&unpack_dir, &rootfs_squashfs)
        .context("Failed to create squashfs")?;

    // Create runner config
    let config = bencher_runner::Config::new(oci_image.clone(), kernel.clone())
        .with_vcpus(1)
        .with_memory_mib(256)
        .with_kernel_cmdline("console=ttyS0 reboot=k panic=1 root=/dev/vda ro init=/init");

    println!("VM Configuration:");
    println!("  Kernel: {}", config.kernel);
    println!("  OCI Image: {}", config.oci_image);
    println!("  vCPUs: {}", config.vcpus);
    println!("  Memory: {} MiB", config.memory_mib);
    println!();

    println!("Running benchmark...");
    let result = bencher_runner::execute(&config).await?;

    println!();
    println!("=== Benchmark Output ===");
    println!("{result}");
    println!("========================");
    println!();

    // Verify the output contains expected benchmark results
    if result.contains("bencher") || result.contains("mock") || result.contains("latency") {
        println!("Test PASSED: Benchmark output looks valid");
    } else if result.contains("error") || result.contains("Error") {
        println!("Test FAILED: Errors in output");
        anyhow::bail!("Benchmark execution failed");
    } else {
        println!("Test PASSED: VM ran successfully");
    }

    // Cleanup
    drop(std::fs::remove_dir_all(&unpack_dir));
    drop(std::fs::remove_file(&rootfs_squashfs));

    Ok(())
}

/// Get the workspace root directory.
fn get_workspace_root() -> Utf8PathBuf {
    Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("Failed to get parent directory")
        .parent()
        .expect("Failed to get workspace root")
        .to_owned()
}
