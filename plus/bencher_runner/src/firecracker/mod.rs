//! Firecracker microVM integration.
//!
//! This module manages Firecracker microVMs for running benchmarks in isolation.
//! Instead of a custom VMM, we use Firecracker as an external process controlled
//! via its REST API over a Unix domain socket.

#![expect(clippy::print_stdout)]

mod client;
pub mod config;
pub mod error;
mod process;
mod vsock;

use std::time::{Duration, Instant};

use crate::metrics::{self, RunMetrics};

pub use error::FirecrackerError;

use config::{Action, ActionType, BootSource, Drive, MachineConfig, VsockConfig};
use process::FirecrackerProcess;
use vsock::VsockListener;

/// Configuration for a Firecracker-based benchmark run.
#[derive(Debug)]
pub struct FirecrackerJobConfig {
    /// Path to the Firecracker binary.
    pub firecracker_bin: String,
    /// Path to the kernel image.
    pub kernel_path: String,
    /// Path to the ext4 rootfs image.
    pub rootfs_path: String,
    /// Number of vCPUs.
    pub vcpus: u8,
    /// Memory size in MiB.
    pub memory_mib: u32,
    /// Kernel boot arguments.
    pub boot_args: String,
    /// Execution timeout in seconds.
    pub timeout_secs: u64,
    /// Working directory for temporary files (API socket, vsock UDS).
    pub work_dir: String,
}

/// Run a benchmark inside a Firecracker microVM.
///
/// This function:
/// 1. Starts a Firecracker process
/// 2. Configures the VM via REST API
/// 3. Creates vsock listeners for result collection
/// 4. Boots the VM
/// 5. Collects results via vsock
/// 6. Cleans up
///
/// Returns the benchmark stdout output.
pub fn run_firecracker(config: &FirecrackerJobConfig) -> Result<String, FirecrackerError> {
    let vm_id = uuid::Uuid::new_v4().to_string();
    let api_socket_path = format!("{}/firecracker-{vm_id}.sock", config.work_dir);
    let vsock_uds_path = format!("{}/vsock-{vm_id}.sock", config.work_dir);

    let start_time = Instant::now();

    // Step 1: Start Firecracker process
    println!("Starting Firecracker process...");
    let mut fc_process =
        FirecrackerProcess::start(&config.firecracker_bin, &api_socket_path, &vm_id)?;

    let client = fc_process.client();

    // Step 2: Configure VM via REST API
    println!("Configuring VM...");

    client.put_machine_config(&MachineConfig {
        vcpu_count: config.vcpus,
        mem_size_mib: config.memory_mib,
        smt: false,
    })?;

    client.put_boot_source(&BootSource {
        kernel_image_path: config.kernel_path.clone(),
        boot_args: config.boot_args.clone(),
    })?;

    client.put_drive(&Drive {
        drive_id: "rootfs".to_owned(),
        path_on_host: config.rootfs_path.clone(),
        is_root_device: true,
        is_read_only: false,
    })?;

    client.put_vsock(&VsockConfig {
        guest_cid: 3,
        uds_path: vsock_uds_path.clone(),
    })?;

    // Step 3: Create vsock listeners (must be before boot)
    println!("Setting up vsock listeners...");
    let vsock_listener = VsockListener::new(&vsock_uds_path)?;

    // Step 4: Boot the VM
    println!("Booting VM...");
    client.put_action(&Action {
        action_type: ActionType::InstanceStart,
    })?;

    // Step 5: Collect results via vsock
    let timeout = if config.timeout_secs > 0 {
        Duration::from_secs(config.timeout_secs)
    } else {
        Duration::from_secs(300) // Default 5 min
    };

    println!("Waiting for benchmark results (timeout: {timeout:?})...");
    let results = vsock_listener.collect_results(timeout)?;
    let elapsed = start_time.elapsed();

    let timed_out = results.exit_code.is_empty() && elapsed >= timeout;

    // Step 6: Output metrics to stderr
    let run_metrics = RunMetrics {
        wall_clock_ms: elapsed.as_millis() as u64,
        timed_out,
        transport: "vsock".to_owned(),
        cgroup: None,
    };
    if let Some(line) = metrics::format_metrics(&run_metrics) {
        eprintln!("{line}");
    }

    if !results.stderr.is_empty() {
        eprint!("{}", results.stderr);
    }

    // Step 7: Kill Firecracker process
    fc_process.kill_after_grace_period(Duration::from_secs(2));

    if timed_out {
        return Err(FirecrackerError::Timeout(format!(
            "VM execution timed out after {} seconds",
            config.timeout_secs
        )));
    }

    Ok(results.stdout)
}
