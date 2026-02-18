//! Firecracker microVM integration.
//!
//! This module manages Firecracker microVMs for running benchmarks in isolation.
//! Instead of a custom VMM, we use Firecracker as an external process controlled
//! via its REST API over a Unix domain socket.

#![expect(clippy::print_stdout, clippy::print_stderr, clippy::use_debug)]

mod client;
pub mod config;
pub mod error;
mod process;
mod vsock;

use std::collections::HashMap;

pub use crate::log_level::FirecrackerLogLevel;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::{Duration, Instant};

use camino::Utf8PathBuf;

use crate::cpu::CpuLayout;
use crate::metrics::{self, RunMetrics};

pub use error::FirecrackerError;

/// Guest CID (Context ID) for the Firecracker VM.
///
/// In the vsock address space:
/// - CID 0 is reserved (hypervisor)
/// - CID 1 is reserved (host in some implementations)
/// - CID 2 is the host
/// - CID 3+ are guests
///
/// Firecracker assigns CID 3 to the single guest VM by convention.
const GUEST_CID: u32 = 3;

use crate::run::RunOutput;

use config::{Action, ActionType, BootSource, Drive, MachineConfig, VsockConfig};
use process::FirecrackerProcess;
use vsock::VsockListener;

/// Configuration for a Firecracker-based benchmark run.
#[derive(Debug)]
pub struct FirecrackerJobConfig {
    /// Path to the Firecracker binary.
    pub firecracker_bin: Utf8PathBuf,
    /// Path to the kernel image.
    pub kernel_path: Utf8PathBuf,
    /// Path to the ext4 rootfs image.
    pub rootfs_path: Utf8PathBuf,
    /// Number of vCPUs.
    pub vcpus: u8,
    /// Memory size in MiB.
    pub memory_mib: u32,
    /// Kernel boot arguments.
    pub boot_args: String,
    /// Execution timeout in seconds.
    pub timeout_secs: u64,
    /// Working directory for temporary files (API socket, vsock UDS).
    pub work_dir: Utf8PathBuf,
    /// Optional CPU layout for core isolation via cpuset.
    pub cpu_layout: Option<CpuLayout>,
    /// Firecracker process log level.
    pub log_level: FirecrackerLogLevel,
    /// Maximum number of output files to decode.
    pub max_file_count: u32,
    /// Maximum content size in bytes for a single output file.
    pub max_content_size: u64,
    /// Maximum data size in bytes per vsock port.
    pub max_output_size: usize,
    /// Grace period after exit code before final collection.
    pub grace_period: bencher_json::GracePeriod,
}

/// Run a benchmark inside a Firecracker microVM.
///
/// This function:
/// 1. Optionally creates a cgroup with cpuset for CPU isolation
/// 2. Starts a Firecracker process (and moves it into the cgroup)
/// 3. Configures the VM via REST API
/// 4. Creates vsock listeners for result collection
/// 5. Boots the VM
/// 6. Collects results via vsock
/// 7. Cleans up (including cgroup)
///
/// Returns the benchmark output including exit code and stdout.
#[expect(clippy::too_many_lines)]
pub fn run_firecracker(
    config: &FirecrackerJobConfig,
    cancel_flag: Option<&Arc<AtomicBool>>,
) -> Result<RunOutput, FirecrackerError> {
    let vm_id = uuid::Uuid::new_v4().to_string();
    let api_socket_path = format!("{}/firecracker-{vm_id}.sock", config.work_dir);
    let vsock_uds_path = format!("{}/vsock-{vm_id}.sock", config.work_dir);

    let start_time = Instant::now();

    // Step 0: Create cgroup with cpuset if CPU layout is provided
    let cgroup = if let Some(layout) = &config.cpu_layout {
        if layout.has_isolation() {
            match crate::jail::CgroupManager::new(&vm_id) {
                Ok(cg) => {
                    // Apply cpuset to pin Firecracker to benchmark cores
                    if let Err(e) = cg.apply_cpuset(layout) {
                        eprintln!("Warning: failed to apply cpuset: {e}");
                    } else {
                        println!(
                            "CPU isolation: Firecracker pinned to cores {}",
                            layout.benchmark_cpuset()
                        );
                    }
                    Some(cg)
                },
                Err(e) => {
                    eprintln!("Warning: failed to create cgroup for CPU isolation: {e}");
                    None
                },
            }
        } else {
            None
        }
    } else {
        None
    };

    // Step 1: Start Firecracker process
    println!("Starting Firecracker process...");
    let housekeeping_cores = config
        .cpu_layout
        .as_ref()
        .map(|l| l.housekeeping.clone())
        .unwrap_or_default();
    let mut fc_process = FirecrackerProcess::start(
        config.firecracker_bin.as_str(),
        &api_socket_path,
        &vm_id,
        config.log_level.as_str(),
        housekeeping_cores,
    )?;

    // Move Firecracker process into cgroup for CPU isolation
    if let Some(cg) = &cgroup
        && let Err(e) = cg.add_pid(fc_process.pid())
    {
        eprintln!("Warning: failed to add Firecracker to cgroup: {e}");
    }

    let client = fc_process.client();

    // Step 2: Configure VM via REST API
    println!("Configuring VM...");

    client.put_machine_config(&MachineConfig {
        vcpu_count: config.vcpus,
        mem_size_mib: config.memory_mib,
        smt: false,
    })?;

    client.put_boot_source(&BootSource {
        kernel_image_path: config.kernel_path.to_string(),
        boot_args: config.boot_args.clone(),
    })?;

    client.put_drive(&Drive {
        drive_id: "rootfs".to_owned(),
        path_on_host: config.rootfs_path.to_string(),
        is_root_device: true,
        is_read_only: false,
    })?;

    client.put_vsock(&VsockConfig {
        guest_cid: GUEST_CID,
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

    let grace_period = Duration::from_secs(u64::from(u32::from(config.grace_period)));
    println!("Waiting for benchmark results (timeout: {timeout:?})...");
    let results = match vsock_listener.collect_results(
        timeout,
        config.max_output_size,
        cancel_flag,
        grace_period,
    ) {
        Ok(results) => results,
        Err(e) => {
            let elapsed = start_time.elapsed();
            // Output metrics on timeout or cancellation
            let run_metrics = RunMetrics {
                wall_clock_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
                timed_out: matches!(e, FirecrackerError::Timeout(_)),
                transport: "vsock".to_owned(),
                cgroup: None,
            };
            if let Some(line) = metrics::format_metrics(&run_metrics) {
                eprintln!("{line}");
            }
            fc_process.kill_after_grace_period(Duration::from_secs(2));
            return Err(e);
        },
    };
    let elapsed = start_time.elapsed();

    // Step 6: Output metrics to stderr
    let run_metrics = RunMetrics {
        wall_clock_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
        timed_out: false,
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

    // Parse exit code from string, defaulting to 1 on parse failure
    let exit_code = parse_exit_code(&results.exit_code);

    // Decode output files from the length-prefixed binary protocol
    let output_files = match results.output_files {
        Some(data) if !data.is_empty() => Some(decode_output_files(
            &data,
            config.max_file_count,
            config.max_content_size,
        )?),
        _ => None,
    };

    Ok(RunOutput {
        exit_code,
        stdout: results.stdout,
        stderr: results.stderr,
        output_files,
    })
}

/// Decode the length-prefixed binary protocol for multiple output files.
fn decode_output_files(
    data: &[u8],
    max_file_count: u32,
    max_content_size: u64,
) -> Result<HashMap<Utf8PathBuf, Vec<u8>>, FirecrackerError> {
    let files = bencher_output_protocol::decode(data, max_file_count, max_content_size)
        .map_err(|e| FirecrackerError::VsockCollection(format!("output files: {e}")))?;
    Ok(files.into_iter().collect())
}

/// Parse an exit code string to i32, defaulting to 1 on failure.
fn parse_exit_code(s: &str) -> i32 {
    s.parse::<i32>().unwrap_or(1)
}

#[cfg(test)]
#[expect(clippy::get_unwrap)]
mod tests {
    use super::*;

    #[test]
    fn parse_exit_code_zero() {
        assert_eq!(parse_exit_code("0"), 0);
    }

    #[test]
    fn parse_exit_code_nonzero() {
        assert_eq!(parse_exit_code("1"), 1);
        assert_eq!(parse_exit_code("137"), 137);
    }

    #[test]
    fn parse_exit_code_invalid() {
        assert_eq!(parse_exit_code("not_a_number"), 1);
    }

    #[test]
    fn parse_exit_code_empty() {
        assert_eq!(parse_exit_code(""), 1);
    }

    #[test]
    fn log_level_default() {
        let level = FirecrackerLogLevel::default();
        assert_eq!(level.as_str(), "Warning");
    }

    #[test]
    fn log_level_from_str() {
        assert_eq!(
            "error".parse::<FirecrackerLogLevel>().unwrap().as_str(),
            "Error"
        );
        assert_eq!(
            "WARNING".parse::<FirecrackerLogLevel>().unwrap().as_str(),
            "Warning"
        );
        assert_eq!(
            "Info".parse::<FirecrackerLogLevel>().unwrap().as_str(),
            "Info"
        );
        assert_eq!(
            "debug".parse::<FirecrackerLogLevel>().unwrap().as_str(),
            "Debug"
        );
        assert_eq!(
            "trace".parse::<FirecrackerLogLevel>().unwrap().as_str(),
            "Trace"
        );
        assert_eq!(
            "off".parse::<FirecrackerLogLevel>().unwrap().as_str(),
            "Off"
        );
    }

    #[test]
    fn log_level_from_str_invalid() {
        assert!("invalid".parse::<FirecrackerLogLevel>().is_err());
    }

    #[test]
    fn log_level_display() {
        assert_eq!(FirecrackerLogLevel::Error.to_string(), "Error");
        assert_eq!(FirecrackerLogLevel::Warning.to_string(), "Warning");
    }

    #[test]
    fn run_output_fields() {
        let mut files = HashMap::new();
        files.insert(Utf8PathBuf::from("out.json"), vec![1, 2, 3]);
        let output = RunOutput {
            exit_code: 42,
            stdout: "hello".to_owned(),
            stderr: "warnings".to_owned(),
            output_files: Some(files),
        };
        assert_eq!(output.exit_code, 42);
        assert_eq!(output.stdout, "hello");
        assert_eq!(output.stderr, "warnings");
        assert_eq!(
            output
                .output_files
                .unwrap()
                .get(Utf8PathBuf::from("out.json").as_path())
                .unwrap(),
            &vec![1, 2, 3]
        );
    }
}
