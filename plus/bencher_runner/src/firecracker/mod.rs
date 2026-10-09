//! Firecracker microVM integration.
//!
//! This module manages Firecracker microVMs for running benchmarks in isolation.
//! Instead of a custom VMM, we use Firecracker as an external process, which
//! boots the VM from a configuration file in its jail and runs no API server.
//!
//! The VMM always runs under the Firecracker jailer, chrooted as an unprivileged
//! user with no host network; see [`crate::jail`].

pub mod config;
pub mod error;
mod pin;
mod process;
mod results;

pub use crate::log_level::SandboxLogLevel;
use std::process::ExitStatus;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use camino::Utf8PathBuf;
use slog::{Logger, info, warn};

use crate::JobDeadline;
use crate::cpu::CpuLayout;
use crate::jail::{CgroupManager, Cpuset, JailPaths, JailUser, VmId};
use crate::metrics::{self, RunMetrics};

pub use error::FirecrackerError;
pub use results::ResultsDrive;

/// How long past the Job's timeout a guest may take to write its results and
/// power off, well inside the server's grace for a Job past its timeout.
const SHUTDOWN_ALLOWANCE: Duration = Duration::from_secs(5);

/// How long after the first pin the VMM's threads are pinned again: long
/// enough for vCPU 0's first run to start KVM's NX recovery worker, and far
/// short of the guest's init.
const REPIN_AFTER: Duration = Duration::from_millis(50);

use crate::run::RunOutput;

use config::{BootSource, Drive, MachineConfig, VmConfig};
use pin::Task;
use process::{FirecrackerProcess, JailedSpawn, Started};

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
    /// Made in the jail before the VMM starts, and read once it is gone.
    pub results: ResultsDrive,
}

/// Run a benchmark inside a jailed Firecracker microVM.
///
/// This function:
/// 1. Creates the VM's cgroup, with a cpuset when the layout isolates cores
/// 2. Writes the VM's configuration into the jail, with the results drive
///    after the rootfs
/// 3. Starts Firecracker under the jailer, placed in the cgroup before exec,
///    which boots the VM
/// 4. Verifies the placement landed
/// 5. Waits for the vCPU threads and pins them
/// 6. Waits for the guest to power off, which it does once its results are on
///    the drive
/// 7. Kills whatever is left in the cgroup, then reads the results
///
/// On a timeout or a cancel it kills the cgroup instead, and the results are
/// discarded.
///
/// The VM boots only with time left on `deadline`, and has until
/// [`SHUTDOWN_ALLOWANCE`] after it runs out to power off.
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

    // Step 0: Create the cgroup, which every jailed process is born in
    let cgroup = cgroup_for_run(log, config.cpu_layout.as_ref(), || {
        CgroupManager::new(log, vm_id)
    })?;

    // Step 1: The configuration Firecracker boots the VM from
    VmConfig {
        boot_source: BootSource {
            kernel_image_path: jail.kernel().chroot().clone(),
            boot_args: config.boot_args.clone(),
        },
        drives: vec![
            Drive {
                drive_id: "rootfs".to_owned(),
                path_on_host: jail.rootfs().chroot().clone(),
                is_root_device: true,
                is_read_only: false,
            },
            // The guest's second drive, `/dev/vdb`.
            Drive {
                drive_id: "results".to_owned(),
                path_on_host: jail.results().chroot().clone(),
                is_root_device: false,
                is_read_only: false,
            },
        ],
        machine_config: MachineConfig {
            vcpu_count: config.vcpus,
            mem_size_mib: config.memory_mib,
            smt: false,
        },
    }
    .write(jail.vm_config().host().as_path())?;

    // Step 2: Start the jailed Firecracker process, which boots the VM, so
    // this is the last point a Job out of time or canceled stops before it.
    let housekeeping_cores = config
        .cpu_layout
        .as_ref()
        .map(|l| l.housekeeping.clone())
        .unwrap_or_default();
    let cgroup_procs = placement_target(&cgroup)?;
    if deadline.remaining().is_zero() {
        return Err(FirecrackerError::Timeout(format!(
            "the timeout ran out before the VM booted (timeout {:?})",
            deadline.timeout()
        )));
    }
    refuse_cancelled(cancel_flag)?;
    info!(log, "Booting VM");
    let mut fc_process = FirecrackerProcess::start(JailedSpawn {
        log: log.clone(),
        jailer_bin: &config.jailer_bin,
        exec_file: &config.firecracker_bin,
        vm_id,
        jail_user: config.jail_user,
        chroot_base_dir: &config.chroot_base_dir,
        netns: &config.netns,
        vm_config: jail.vm_config(),
        log_level: config.log_level.as_str(),
        housekeeping_cores,
        cgroup_procs: Some(cgroup_procs),
    })?;

    // Step 2b: Verify the placement landed, which is race free because `spawn`
    // returns only after the exec, and catches a write to the wrong cgroup; a
    // VMM that already exited is found by where it ran.
    verify_placement(&cgroup, fc_process.pid())?;

    // Step 3: Wait for the vCPU threads, which start only once Firecracker has
    // accepted its configuration, and pin them to dedicated benchmark cores
    // before the guest's init runs: a required companion to the isolated
    // cpuset partition, which has no load balancing between cores.
    let until = deadline.extended_by(SHUTDOWN_ALLOWANCE);
    let ended = match fc_process.wait_for_vcpus(config.vcpus, until, cancel_flag) {
        Ok(Some(Started::Vcpus(threads))) => {
            let pinning = config
                .cpu_layout
                .as_ref()
                .filter(|layout| layout.has_isolation());
            if let Some(layout) = pinning {
                pin::pin_vcpu_threads(log, &threads, layout, config.vcpus);
            }

            // Step 4: Wait for the guest to power off
            info!(log, "Waiting for the VM to power off";
                "timeout_secs" => deadline.timeout().as_secs(),
                "remaining_ms" => u64::try_from(deadline.remaining().as_millis()).unwrap_or(u64::MAX),
                "shutdown_allowance_secs" => SHUTDOWN_ALLOWANCE.as_secs(),
            );
            match pinning {
                Some(layout) => {
                    let task_dir = fc_process.task_dir();
                    repin_then_wait(
                        until,
                        |deadline| fc_process.wait_for_exit(deadline, cancel_flag),
                        || pin::read_tasks(&task_dir),
                        |threads| pin::pin_threads_again(log, threads, layout),
                    )
                },
                None => fc_process.wait_for_exit(until, cancel_flag),
            }
        },
        Ok(Some(Started::Exited(status))) => {
            warn!(
                log,
                "The VM ran and powered off before its vCPU threads were seen, so none was pinned"
            );
            Ok(Some(status))
        },
        Ok(None) => Ok(None),
        Err(e) => Err(e),
    };
    let ended = match ended {
        Ok(Some(status)) => Ok(status),
        Ok(None) => Err(FirecrackerError::Timeout(format!(
            "VM execution timed out after {:?}",
            deadline.timeout()
        ))),
        Err(e) => Err(e),
    };
    let elapsed = start_time.elapsed();
    let run_metrics = RunMetrics {
        wall_clock_ms: u64::try_from(elapsed.as_millis()).unwrap_or(u64::MAX),
        timed_out: matches!(ended, Err(FirecrackerError::Timeout(_))),
        transport: "drive".to_owned(),
        cgroup: metrics::read_cgroup_metrics(cgroup.path()),
    };
    info!(log, "Run metrics"; &run_metrics);
    let status = match ended {
        Ok(status) => status,
        Err(e) => {
            // One write kills the VMM and anything it started.
            if let Err(kill) = cgroup.empty() {
                warn!(log, "VM cgroup not emptied, left to the teardown"; "error" => %kill);
            }
            return Err(e);
        },
    };
    // A guest stops itself however it likes, so a failed stop after a whole
    // record still reports that record.
    info!(log, "VMM exited"; "status" => %status);

    // Step 5: Read the results the guest left
    config
        .results
        .read_from_emptied_jail(&cgroup, config.max_file_count, config.max_content_size)
}

/// Pin the VMM's threads again after [`REPIN_AFTER`], read anew so any started
/// since the first pass is among them, then wait for the VMM to exit by
/// `until`. The wait, the thread list, and the pin are parameters, so a test
/// can stand in for each.
fn repin_then_wait<W, R, P>(
    until: JobDeadline,
    mut wait: W,
    mut threads: R,
    mut pin_again: P,
) -> Result<Option<ExitStatus>, FirecrackerError>
where
    W: FnMut(JobDeadline) -> Result<Option<ExitStatus>, FirecrackerError>,
    R: FnMut() -> Vec<Task>,
    P: FnMut(&[Task]),
{
    if let Some(status) = wait(JobDeadline::start(REPIN_AFTER.min(until.remaining())))? {
        return Ok(Some(status));
    }
    pin_again(&threads());
    wait(until)
}

/// Every run gets a cgroup or fails the job, since placement and the kill need
/// no controller, and a layout with isolation confines it too, per the failure
/// policy table in [`crate::jail`].
fn cgroup_for_run<F>(
    log: &Logger,
    layout: Option<&CpuLayout>,
    create: F,
) -> Result<CgroupManager, FirecrackerError>
where
    F: FnOnce() -> Result<CgroupManager, crate::RunnerError>,
{
    let cgroup = create().map_err(|e| FirecrackerError::Cgroup(Box::new(e)))?;
    match layout.filter(|layout| layout.has_isolation()) {
        Some(layout) => confine(log, cgroup, layout),
        None => Ok(cgroup),
    }
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

fn placement_target(cgroup: &CgroupManager) -> Result<std::fs::File, FirecrackerError> {
    cgroup
        .open_procs()
        .map_err(FirecrackerError::CgroupPlacement)
}

fn verify_placement(cgroup: &CgroupManager, pid: u32) -> Result<(), FirecrackerError> {
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

#[cfg(test)]
mod tests {
    use crate::error::JailError;
    use crate::log::discard;

    use super::*;

    #[test]
    fn a_vm_cgroup_that_cannot_be_created_fails_the_job() {
        // A VMM outside its cgroup runs unconfined, unmetered, and beyond the
        // kill that reaps it, so no reason to lack one degrades.
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

        assert_eq!(cgroup.path(), root, "a cgroup that was created is kept");
    }

    #[test]
    fn a_run_whose_layout_isolates_nothing_still_gets_its_cgroup() {
        // Kills a run with no cgroup on a one-core layout, or on `runner run`'s
        // missing one, whose VMM the kill could then never reach.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();

        for layout in [Some(CpuLayout::with_core_count(1)), None] {
            let cgroup = cgroup_for_run(&discard(), layout.as_ref(), || {
                Ok(CgroupManager::detached(root.clone()))
            })
            .unwrap();

            assert_eq!(cgroup.path(), root, "{layout:?}");
            assert!(
                !root.join("memory.swap.max").exists(),
                "a layout with no isolation is not confined: {layout:?}"
            );
        }
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
    fn the_second_pass_pins_a_thread_started_after_the_first() {
        // Kills a second pass that pins a thread list read before its wait, or
        // none, which leaves a thread started since, as KVM's NX recovery
        // worker, on whatever cores it inherited.
        use std::os::unix::process::ExitStatusExt as _;

        let started_since = std::cell::Cell::new(false);
        let mut waits = 0;
        let mut pinned = Vec::new();

        let status = repin_then_wait(
            JobDeadline::start(Duration::from_secs(30)),
            |_deadline| {
                waits += 1;
                if waits == 1 {
                    // The worker starts while the second pass waits.
                    started_since.set(true);
                    Ok(None)
                } else {
                    Ok(Some(ExitStatus::from_raw(0)))
                }
            },
            || {
                let mut threads = vec![
                    (100, "firecracker".to_owned()),
                    (101, "fc_vcpu 0".to_owned()),
                ];
                if started_since.get() {
                    threads.push((102, "kvm-nx-lpage-re".to_owned()));
                }
                threads
            },
            |threads| pinned.push(threads.to_vec()),
        )
        .unwrap();

        assert!(status.is_some_and(|status| status.success()), "{status:?}");
        assert_eq!(
            pinned,
            [vec![
                (100, "firecracker".to_owned()),
                (101, "fc_vcpu 0".to_owned()),
                (102, "kvm-nx-lpage-re".to_owned()),
            ]]
        );
    }

    #[test]
    fn a_cgroup_without_the_pid_aborts() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8PathBuf::try_from(dir.path().to_path_buf()).unwrap();
        std::fs::write(root.join("cgroup.procs"), "999\n").unwrap();
        let cgroup = CgroupManager::detached(root);

        let err = verify_placement(&cgroup, 123).unwrap_err();

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

        verify_placement(&cgroup, 123).unwrap();
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
