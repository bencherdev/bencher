//! Firecracker microVM integration.
//!
//! This module manages Firecracker microVMs for running benchmarks in isolation.
//! Instead of a custom VMM, we use Firecracker as an external process controlled
//! via its REST API over a Unix domain socket.
//!
//! The VMM always runs under the Firecracker jailer, chrooted as an unprivileged
//! user with no host network; see [`crate::jail`].

mod client;
pub mod config;
pub mod error;
mod pin;
mod process;
#[cfg(test)]
mod test_util;
mod vsock;

pub use crate::log_level::SandboxLogLevel;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use camino::Utf8PathBuf;
use slog::{Logger, info, warn};

use crate::JobDeadline;
use crate::cpu::CpuLayout;
use crate::jail::{CgroupManager, CgroupSurvived, Cpuset, JailPaths, JailUser, VmId};
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
use process::{FirecrackerProcess, JailedSpawn};
use vsock::VsockListener;

/// Configuration for a Firecracker-based benchmark run.
#[derive(Debug)]
pub struct FirecrackerJobConfig {
    /// Path to the staged Firecracker binary, outside the jail.
    pub firecracker_bin: Utf8PathBuf,
    pub jailer_bin: Utf8PathBuf,
    /// The jailer id, chroot name, and cgroup name, minted before the job's
    /// artifacts because the jail root is derived from it.
    pub vm_id: VmId,
    pub jail: JailPaths,
    pub jail_user: JailUser,
    pub chroot_base_dir: Utf8PathBuf,
    /// Handle of the empty network namespace the VMM joins.
    pub netns: Utf8PathBuf,
    /// Shared with the chroot guard of the same id, so a cgroup this job cannot
    /// remove keeps the chroot a later sweep finds it by.
    pub cgroup_survived: CgroupSurvived,
    /// Number of vCPUs.
    pub vcpus: u8,
    /// Memory size in MiB.
    pub memory_mib: u32,
    /// Kernel boot arguments.
    pub boot_args: String,
    /// Optional CPU layout for core isolation via cpuset.
    pub cpu_layout: Option<CpuLayout>,
    /// Firecracker process log level.
    pub log_level: SandboxLogLevel,
    /// Maximum number of output files to decode.
    pub max_file_count: u32,
    /// Maximum content size in bytes for a single output file.
    pub max_content_size: u64,
    /// Maximum data size in bytes per vsock port.
    pub max_output_size: usize,
    /// Grace period after exit code before final collection.
    pub grace_period: bencher_json::GracePeriod,
}

/// Run a benchmark inside a jailed Firecracker microVM.
///
/// This function:
/// 1. Optionally creates a cgroup with cpuset for CPU isolation
/// 2. Starts Firecracker under the jailer, placed in the cgroup before exec
/// 3. Verifies the placement landed
/// 4. Configures the VM via REST API
/// 5. Creates vsock listeners for result collection and hands them to the jail
/// 6. Boots the VM
/// 7. Collects results via vsock
/// 8. Cleans up (including cgroup)
///
/// Returns the benchmark output including exit code and stdout.
///
/// The VM boots only with time left on `deadline`, and its results must arrive
/// before it runs out.
#[expect(
    clippy::too_many_lines,
    reason = "VM lifecycle steps are sequential and clearer inline"
)]
pub fn run_firecracker(
    log: &Logger,
    config: &FirecrackerJobConfig,
    cancel_flag: Option<&AtomicBool>,
    deadline: JobDeadline,
) -> Result<RunOutput, FirecrackerError> {
    let vm_id = &config.vm_id;
    let jail = &config.jail;

    let start_time = Instant::now();

    // Step 0: Create cgroup with cpuset if CPU layout is provided
    let cgroup = cgroup_for_run(log, config.cpu_layout.as_ref(), || {
        CgroupManager::new(log, vm_id, config.cgroup_survived.clone())
    })?;

    // Step 1: Start the jailed Firecracker process.
    info!(log, "Starting the jailed Firecracker process");
    let housekeeping_cores = config
        .cpu_layout
        .as_ref()
        .map(|l| l.housekeeping.clone())
        .unwrap_or_default();
    let cgroup_procs = placement_target(cgroup.as_ref())?;
    let mut fc_process = FirecrackerProcess::start(JailedSpawn {
        log: log.clone(),
        jailer_bin: &config.jailer_bin,
        exec_file: &config.firecracker_bin,
        vm_id,
        jail_user: config.jail_user,
        chroot_base_dir: &config.chroot_base_dir,
        netns: &config.netns,
        api_socket: jail.api_socket(),
        log_level: config.log_level.as_str(),
        housekeeping_cores,
        cgroup_procs,
    })?;

    // Step 1b: Verify the placement landed, which is race free because `spawn`
    // returns only after the exec, and catches a write to the wrong cgroup.
    verify_placement(cgroup.as_ref(), fc_process.pid())?;
    // Placed now, so a process that joined another Bencher cgroup while the
    // jail was built is seen before the guest runs.
    crate::jail::refuse_occupied_cgroups(Some(vm_id)).map_err(FirecrackerError::CoresOccupied)?;
    refuse_cancelled(cancel_flag)?;

    let client = fc_process.client();

    // Step 2: Configure VM via REST API
    info!(log, "Configuring VM");

    client.put_machine_config(&MachineConfig {
        vcpu_count: config.vcpus,
        mem_size_mib: config.memory_mib,
        smt: false,
    })?;

    client.put_boot_source(&BootSource {
        kernel_image_path: jail.kernel().chroot().clone(),
        boot_args: config.boot_args.clone(),
    })?;

    client.put_drive(&Drive {
        drive_id: "rootfs".to_owned(),
        path_on_host: jail.rootfs().chroot().clone(),
        is_root_device: true,
        is_read_only: false,
    })?;

    client.put_vsock(&VsockConfig {
        guest_cid: GUEST_CID,
        uds_path: jail.vsock().chroot().clone(),
    })?;

    // Step 3: Create vsock listeners (must be before boot)
    info!(log, "Setting up vsock listeners");
    let vsock_listener = VsockListener::new(jail.vsock())?;
    vsock_listener
        .chown_to_jail(config.jail_user)
        .map_err(FirecrackerError::Chown)?;

    // Step 4: Boot the VM
    if deadline.remaining().is_zero() {
        return Err(FirecrackerError::Timeout(format!(
            "the timeout ran out before the VM booted (timeout {:?})",
            deadline.timeout()
        )));
    }
    info!(log, "Booting VM");
    client.put_action(&Action {
        action_type: ActionType::InstanceStart,
    })?;

    // Pin vCPU threads (spawned during InstanceStart) to dedicated
    // benchmark cores. Required companion to the isolated cpuset
    // partition, which has no load balancing between cores.
    if let Some(layout) = &config.cpu_layout
        && layout.has_isolation()
    {
        pin::pin_vcpu_threads(log, fc_process.pid(), layout, config.vcpus);
    }

    // Step 5: Collect results via vsock
    let grace_period = Duration::from_secs(u64::from(u32::from(config.grace_period)));
    info!(log, "Waiting for benchmark results";
        "timeout_secs" => deadline.timeout().as_secs(),
        "remaining_ms" => u64::try_from(deadline.remaining().as_millis()).unwrap_or(u64::MAX),
    );
    let results = match vsock_listener.collect_results(
        deadline,
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
                cgroup: cgroup
                    .as_ref()
                    .and_then(|cg| metrics::read_cgroup_metrics(cg.path())),
            };
            info!(log, "Run metrics"; &run_metrics);
            fc_process.kill_after_grace_period(Duration::from_secs(2));
            return Err(e);
        },
    };
    let elapsed = start_time.elapsed();

    // Step 6: Log the metrics
    let run_metrics = RunMetrics {
        wall_clock_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
        timed_out: false,
        transport: "vsock".to_owned(),
        cgroup: cgroup
            .as_ref()
            .and_then(|cg| metrics::read_cgroup_metrics(cg.path())),
    };
    info!(log, "Run metrics"; &run_metrics);

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

/// A layout with isolation gets the VMM a confined cgroup or fails the job,
/// per the failure policy table in [`crate::jail`].
fn cgroup_for_run<F>(
    log: &Logger,
    layout: Option<&CpuLayout>,
    create: F,
) -> Result<Option<CgroupManager>, FirecrackerError>
where
    F: FnOnce() -> Result<CgroupManager, crate::RunnerError>,
{
    let Some(layout) = layout.filter(|layout| layout.has_isolation()) else {
        return Ok(None);
    };
    let cgroup = create().map_err(|e| FirecrackerError::Cgroup(Box::new(e)))?;
    confine(log, cgroup, layout).map(Some)
}

/// Stop at a stage boundary once the job is cancelled, before anything later is
/// built or booted.
pub(crate) fn refuse_cancelled(cancel_flag: Option<&AtomicBool>) -> Result<(), FirecrackerError> {
    if cancel_flag.is_some_and(|flag| flag.load(Ordering::SeqCst)) {
        return Err(FirecrackerError::Cancelled);
    }
    Ok(())
}

/// A rejected cpuset is fatal, but an undelegated one only warns and keeps the
/// cgroup for placement, swap, and metrics.
fn confine(
    log: &Logger,
    cgroup: CgroupManager,
    layout: &CpuLayout,
) -> Result<CgroupManager, FirecrackerError> {
    match cgroup
        .apply_cpuset(layout)
        .map_err(|e| FirecrackerError::CpusetFailed(Box::new(e)))?
    {
        Cpuset::Applied => {
            info!(log, "CPU isolation applied"; "cores" => layout.benchmark_cpuset());
        },
        // The vCPU threads are still pinned further down, which is gated on
        // the layout rather than on the cgroup.
        Cpuset::Unavailable(reason) => {
            warn!(log, "No cgroup cpuset, so other work can share the benchmark cores";
                "reason" => reason,
            );
        },
    }
    // Keep VM memory resident: swap adds run-to-run variance
    if let Err(e) = cgroup.disable_swap() {
        warn!(log, "No swap limit, so guest memory can be swapped out"; "error" => %e);
    }
    Ok(cgroup)
}

fn placement_target(
    cgroup: Option<&CgroupManager>,
) -> Result<Option<std::fs::File>, FirecrackerError> {
    cgroup
        .map(CgroupManager::open_procs)
        .transpose()
        .map_err(FirecrackerError::CgroupPlacement)
}

fn verify_placement(cgroup: Option<&CgroupManager>, pid: u32) -> Result<(), FirecrackerError> {
    let Some(cgroup) = cgroup else {
        return Ok(());
    };
    let placed = cgroup
        .contains_pid(pid)
        .map_err(FirecrackerError::CgroupPlacement)?;
    if placed {
        Ok(())
    } else {
        Err(FirecrackerError::CgroupMissingPid {
            pid,
            cgroup: cgroup.path().to_owned(),
        })
    }
}

/// Decode the length-prefixed binary protocol for multiple output files.
fn decode_output_files(
    data: &[u8],
    max_file_count: u32,
    max_content_size: u64,
) -> Result<Vec<(Utf8PathBuf, Vec<u8>)>, FirecrackerError> {
    bencher_output_protocol::decode(data, max_file_count, max_content_size)
        .map_err(|source| FirecrackerError::DecodeOutputFiles { source })
}

/// Parse an exit code string to i32, defaulting to 1 on failure.
fn parse_exit_code(s: &str) -> i32 {
    s.parse::<i32>().unwrap_or(1)
}

#[cfg(test)]
mod tests {
    use crate::error::JailError;
    use crate::log::discard;

    use super::*;

    #[test]
    fn a_vm_cgroup_that_cannot_be_created_fails_the_job() {
        // A VMM outside its cgroup runs unconfined, unmetered, and unseen by the
        // occupancy check, so no reason to lack one degrades.
        let unwritable = || {
            Err(JailError::CreateCgroup {
                path: Utf8PathBuf::from("/sys/fs/cgroup/bencher/vm-1"),
                source: std::io::Error::from_raw_os_error(libc::EROFS),
            }
            .into())
        };

        let Err(err) = cgroup_for_run(&discard(), Some(&CpuLayout::with_core_count(8)), unwritable)
        else {
            panic!("a job with no VM cgroup must not run");
        };

        assert!(
            matches!(err, FirecrackerError::Cgroup(_)),
            "the job must fail naming the cgroup, got: {err}"
        );
    }

    #[test]
    fn a_cgroup_that_was_created_is_the_one_the_run_uses() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

        let cgroup = cgroup_for_run(&discard(), Some(&CpuLayout::with_core_count(8)), || {
            Ok(CgroupManager::detached(root.clone()))
        })
        .unwrap();

        assert_eq!(
            cgroup.map(|cg| cg.path().to_owned()),
            Some(root),
            "a cgroup that was created is kept"
        );
    }

    #[test]
    fn a_cgroup_without_a_cpuset_still_keeps_memory_resident() {
        // Prevents dropping the cgroup over a missing `cpuset`, which lets guest memory swap.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

        let cgroup = confine(
            &discard(),
            CgroupManager::detached(root.clone()),
            &CpuLayout::with_core_count(8),
        )
        .unwrap();

        assert_eq!(cgroup.path(), root, "the cgroup is kept");
        assert_eq!(
            std::fs::read_to_string(root.join("memory.swap.max")).unwrap(),
            "0",
            "swap is off without a cpuset too"
        );
    }

    #[test]
    fn no_cgroup_skips_placement() {
        // Prevents a host with no cgroup from failing the job at placement.
        assert!(
            placement_target(None).unwrap().is_none(),
            "no cgroup means nothing to place through"
        );
    }

    #[test]
    fn no_cgroup_skips_verification() {
        verify_placement(None, 1).unwrap();
    }

    #[test]
    fn a_cgroup_without_the_pid_aborts() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        std::fs::write(root.join("cgroup.procs"), "999\n").unwrap();
        let cgroup = CgroupManager::detached(root);

        let err = verify_placement(Some(&cgroup), 123).unwrap_err();

        assert!(
            matches!(err, FirecrackerError::CgroupMissingPid { pid: 123, .. }),
            "a cgroup that does not contain the VMM must abort, got: {err}"
        );
    }

    #[test]
    fn a_cgroup_holding_the_pid_verifies() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        std::fs::write(root.join("cgroup.procs"), "123\n456\n").unwrap();
        let cgroup = CgroupManager::detached(root);

        verify_placement(Some(&cgroup), 123).unwrap();
    }

    #[test]
    fn output_files_decode_in_the_order_the_guest_sent_them() {
        // Descending, and enough files that neither a sorted nor a hashed order matches.
        let paths: Vec<Utf8PathBuf> = (0..32)
            .rev()
            .map(|n| Utf8PathBuf::from(format!("/{n}.out")))
            .collect();
        let files: Vec<(&camino::Utf8Path, &[u8])> = paths
            .iter()
            .map(|path| (path.as_path(), b"x".as_slice()))
            .collect();
        let data = bencher_output_protocol::encode(&files).unwrap();

        let decoded = decode_output_files(&data, 32, 1).unwrap();

        let decoded_paths: Vec<Utf8PathBuf> = decoded.into_iter().map(|(path, _)| path).collect();
        assert_eq!(decoded_paths, paths);
    }

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
        let level = SandboxLogLevel::default();
        assert_eq!(level.as_str(), "Warning");
    }

    #[test]
    fn log_level_from_str() {
        assert_eq!(
            "error".parse::<SandboxLogLevel>().unwrap().as_str(),
            "Error"
        );
        assert_eq!(
            "WARNING".parse::<SandboxLogLevel>().unwrap().as_str(),
            "Warning"
        );
        assert_eq!("Info".parse::<SandboxLogLevel>().unwrap().as_str(), "Info");
        assert_eq!(
            "debug".parse::<SandboxLogLevel>().unwrap().as_str(),
            "Debug"
        );
        assert_eq!(
            "trace".parse::<SandboxLogLevel>().unwrap().as_str(),
            "Trace"
        );
        assert_eq!("off".parse::<SandboxLogLevel>().unwrap().as_str(), "Off");
    }

    #[test]
    fn log_level_from_str_invalid() {
        "invalid".parse::<SandboxLogLevel>().unwrap_err();
    }

    #[test]
    fn log_level_display() {
        assert_eq!(SandboxLogLevel::Error.to_string(), "Error");
        assert_eq!(SandboxLogLevel::Warning.to_string(), "Warning");
    }

    #[test]
    fn run_output_fields() {
        let files = vec![(Utf8PathBuf::from("out.json"), vec![1, 2, 3])];
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
            output.output_files.unwrap(),
            vec![(Utf8PathBuf::from("out.json"), vec![1, 2, 3])]
        );
    }
}
