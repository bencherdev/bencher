use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use camino::{Utf8Path, Utf8PathBuf};
use slog::{Logger, info, warn};

use bencher_json::Iteration;

use crate::JobDeadline;
use crate::error::RunnerError;
use crate::tuning::{TuningConfig, preflight};

/// Output from a benchmark run.
#[derive(Debug)]
pub struct RunOutput {
    /// Exit code from the guest process.
    pub exit_code: i32,
    /// Stdout output from the benchmark.
    pub stdout: String,
    /// Stderr output from the benchmark.
    pub stderr: String,
    /// Optional output files as (path, contents), in the Job's declared order.
    pub output_files: Option<Vec<(Utf8PathBuf, Vec<u8>)>>,
}

/// Arguments for the `run` subcommand.
#[derive(Debug, Clone)]
pub struct RunArgs {
    /// OCI image (local path or registry reference).
    pub image: String,
    /// JWT token for registry authentication.
    pub token: Option<String>,
    /// Optional vCPU count override.
    pub vcpus: Option<bencher_json::Cpu>,
    /// Optional memory override (in bytes).
    pub memory: Option<bencher_json::Memory>,
    /// Optional disk size override (in bytes).
    pub disk: Option<bencher_json::Disk>,
    /// Execution timeout in seconds, shared by every iteration from the start of the run.
    pub timeout_secs: u64,
    /// Output file paths inside guest.
    pub file_paths: Option<Vec<Utf8PathBuf>>,
    /// Maximum size in bytes for collected stdout/stderr.
    pub max_output_size: Option<usize>,
    /// Maximum number of output files to decode.
    pub max_file_count: Option<u32>,
    /// Maximum number of symlinks to follow during path resolution.
    pub max_symlinks: Option<u32>,
    /// Optional entrypoint override for the container.
    pub entrypoint: Option<Vec<String>>,
    /// Optional command override for the container.
    pub cmd: Option<Vec<String>>,
    /// Optional environment variables for the container.
    pub env: Option<HashMap<String, String>>,
    /// Whether to enable network access in the VM.
    pub network: bool,
    /// Number of benchmark iterations.
    pub iter: Iteration,
    /// Allow benchmark failure without short-circuiting iterations. Running out
    /// of time still ends the run as timed out.
    pub allow_failure: bool,
    /// Host tuning configuration.
    pub tuning: TuningConfig,
    /// Sandbox process log level.
    pub sandbox_log_level: crate::SandboxLogLevel,
    /// Sandbox mode for benchmark execution.
    pub sandbox: Option<bencher_json::Sandbox>,
    pub state_dir: Utf8PathBuf,
    pub jail_user: crate::jail::JailUser,
}

/// Build a `Config` from CLI `RunArgs`.
///
/// Shared between the Linux and non-Linux debug `run_with_args` paths.
fn build_config_from_run_args(args: &RunArgs) -> Result<crate::Config, crate::error::ConfigError> {
    let mut config = crate::Config::new(args.image.clone())
        .with_timeout_secs(args.timeout_secs)
        .with_network(args.network);
    if let Some(vcpus) = args.vcpus {
        config = config.with_vcpus(vcpus);
    }
    if let Some(memory) = args.memory {
        config = config.with_memory(memory);
    }
    if let Some(disk) = args.disk {
        config = config.with_disk(disk);
    }
    let config = if let Some(token) = &args.token {
        config.with_token(token.clone())?
    } else {
        config
    };
    let config = if let Some(file_paths) = &args.file_paths {
        config.with_file_paths(file_paths.clone())
    } else {
        config
    };
    let config = if let Some(max_output_size) = args.max_output_size {
        config.with_max_output_size(max_output_size)
    } else {
        config
    };
    let config = if let Some(max_file_count) = args.max_file_count {
        config.with_max_file_count(max_file_count)
    } else {
        config
    };
    let mut config = if let Some(max_symlinks) = args.max_symlinks {
        config.with_max_symlinks(max_symlinks)
    } else {
        config
    };
    config = config
        .with_entrypoint_opt(args.entrypoint.clone())
        .with_cmd_opt(args.cmd.clone())
        .with_env_opt(args.env.clone());
    config.sandbox_log_level = args.sandbox_log_level;
    config = config.with_sandbox(args.sandbox);
    config = config.with_state_dir(args.state_dir.clone());
    config = config.with_jail_user(args.jail_user);
    Ok(config)
}

/// Run the `run` subcommand with parsed arguments.
///
/// Dispatches to Firecracker VM or local host execution based on the sandbox
/// setting in the configuration.
#[cfg_attr(
    not(target_os = "linux"),
    expect(unused_mut, reason = "config only mutated on Linux for CPU isolation")
)]
pub fn run_with_args(log: &Logger, args: &RunArgs) -> Result<(), RunnerError> {
    // A signal cancels the job through its teardown rather than killing the
    // runner and stranding the VMM.
    crate::signal::install_cancel_handlers();

    #[cfg(target_os = "linux")]
    let runner_lock = crate::runner_lock::RunnerLock::acquire(log)?;
    #[cfg(target_os = "linux")]
    crate::jail::prepare_at_startup(log, &runner_lock, args.sandbox.is_some())?;

    // Warn about host conditions that limit benchmark accuracy (Linux only)
    preflight::log_host_warnings(log);

    let tuning = &args.tuning;

    // Apply host tuning, which persists until the host reboots (no-op on non-Linux)
    let _tuning_guard = crate::tuning::apply(log, tuning);

    let mut config = build_config_from_run_args(args)?;

    // Detect the CPU layout after tuning (disabling SMT changes the core
    // count), steer kernel work off the benchmark cores, and pin the run
    // to them. Mirrors the `runner up` path. Core pinning is core runner
    // behavior, deliberately independent of --no-tuning: the tuning config
    // only gates the tuning knobs (including the partition and steering).
    #[cfg(target_os = "linux")]
    {
        let cpu_layout = crate::cpu::CpuLayout::detect(log);
        crate::tuning::apply_cpu_scoped(log, tuning, &cpu_layout);
        cpu_layout.log_isolation(log);
        if cpu_layout.has_isolation() {
            config = config.with_cpu_layout(cpu_layout);
        }
    }

    let mut host = crate::jail::HostPreparation::new();

    // Like a Job's, the timeout spans every iteration.
    let deadline = JobDeadline::start(Duration::from_secs(args.timeout_secs));
    let iter_count = args.iter.as_usize();
    for iteration in 0..iter_count {
        if crate::signal::stop_requested() {
            return Err(
                crate::error::ExecutionError::Canceled("run was canceled".to_owned()).into(),
            );
        }
        if deadline.remaining().is_zero() {
            return Err(crate::error::ExecutionError::Timeout(format!(
                "the run timed out after {}s, before iteration {}/{iter_count}",
                deadline.timeout().as_secs(),
                iteration + 1
            ))
            .into());
        }
        match execute_with_deadline(
            log,
            &config,
            &mut host,
            Some(crate::signal::stop_flag()),
            deadline,
        ) {
            Ok(output) => {
                print_product(&output);
                if output.exit_code != 0 && !args.allow_failure {
                    return Err(RunnerError::NonZeroExitCode(output.exit_code));
                }
            },
            Err(e) => {
                // A failure with no time left ends the run, since no later
                // iteration could run either.
                if args.allow_failure
                    && !crate::signal::stop_requested()
                    && !deadline.remaining().is_zero()
                {
                    warn!(log, "Iteration failed, skipped under allow_failure";
                        "iteration" => iteration + 1,
                        "iterations" => iter_count,
                        "error" => bencher_logger::capped(&e),
                    );
                    continue;
                }
                return Err(e);
            },
        }
    }
    Ok(())
}

/// The benchmark's output is the product of `runner run`, so it goes out raw.
#[expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "the benchmark's output is the product of `runner run`"
)]
fn print_product(output: &RunOutput) {
    println!("{}", output.stdout);
    if !output.stderr.is_empty() {
        eprintln!("{}", output.stderr);
    }
}

/// Workspace created by [`prepare_oci_workspace`]: temp directory, work/unpack
/// paths, and the resolved OCI configuration.
pub(crate) struct OciWorkspace {
    /// Held for RAII — dropping this removes the temporary directory.
    #[expect(dead_code, reason = "kept alive for RAII cleanup of temp directory")]
    pub temp_dir: tempfile::TempDir,
    /// Base working directory inside the temp dir.
    #[cfg_attr(
        not(target_os = "linux"),
        expect(dead_code, reason = "only read by vm_execute on Linux")
    )]
    pub work_dir: Utf8PathBuf,
    pub unpack_dir: Utf8PathBuf,
    pub oci_config: ResolvedOciConfig,
}

/// Prepare a temporary OCI workspace: create temp dir, resolve the OCI image,
/// parse its config, and unpack the layers.
///
/// Shared between `local_execute` and `vm_execute` to avoid duplicating this
/// setup sequence.
pub(crate) fn prepare_oci_workspace(
    log: &Logger,
    config: &crate::Config,
) -> Result<OciWorkspace, RunnerError> {
    // Create a temporary work directory
    let temp_dir = tempfile::tempdir().map_err(crate::error::ConfigError::TempDir)?;
    let work_dir =
        Utf8Path::from_path(temp_dir.path()).ok_or(crate::error::ConfigError::NonUtf8TempDir)?;
    let work_dir = work_dir.to_path_buf();

    let unpack_dir = work_dir.join("rootfs");

    // Resolve OCI image (local path or pull from registry)
    let oci_image_path = resolve_oci_image(
        log,
        &config.oci_image,
        config.token.as_ref().map(AsRef::as_ref),
        config.registry_scheme,
        &work_dir,
    )?;

    // Parse OCI image config to get the command
    info!(log, "Parsing OCI image config");
    let oci_image = bencher_oci::OciImage::parse(&oci_image_path)?;
    let oci_config = resolve_oci_config(&oci_image, config)?;

    info!(log, "OCI image config";
        "command" => bencher_logger::capped(oci_config.command.join(" ")),
        "work_dir" => bencher_logger::capped(&oci_config.working_dir),
        "env_vars" => oci_config.env.len(),
    );

    // Unpack OCI image layers into the rootfs directory
    info!(log, "Unpacking OCI image"; "unpack_dir" => unpack_dir.as_str());
    bencher_oci::unpack(&oci_image_path, &unpack_dir)?;

    Ok(OciWorkspace {
        temp_dir,
        work_dir,
        unpack_dir,
        oci_config,
    })
}

/// Resolve an OCI image source to a local path.
///
/// If the source is a local path that exists, returns it directly.
/// If the source looks like a registry reference, pulls from the registry
/// into the provided `pull_dir`. Image data is not cached between runs —
/// the caller is expected to pass a temporary directory that is cleaned up
/// after each job.
///
/// # Arguments
///
/// * `oci_image` - Local path or registry reference
/// * `token` - Optional JWT token for registry authentication
/// * `pull_dir` - Directory to pull images into (temporary, not cached)
///
/// # Returns
///
/// Path to the local OCI image directory.
pub fn resolve_oci_image(
    log: &Logger,
    oci_image: &str,
    token: Option<&str>,
    scheme: bencher_oci::RegistryScheme,
    pull_dir: &Utf8Path,
) -> Result<Utf8PathBuf, RunnerError> {
    let path = Utf8Path::new(oci_image);

    // If it's a local path that exists, use it directly
    if path.exists() {
        info!(log, "Using a local OCI image"; "image" => bencher_logger::capped(oci_image));
        return Ok(path.to_owned());
    }

    // Otherwise, treat as a registry reference
    info!(log, "Parsing registry reference"; "image" => bencher_logger::capped(oci_image));
    let image_ref = bencher_oci::ImageReference::parse(oci_image)
        .map_err(|e| bencher_oci::OciError::InvalidReference(e.to_string()))?;

    // Pull into the provided directory
    let image_dir = pull_dir.join("oci-image");

    // Pull from registry
    info!(log, "Pulling from registry"; "image" => bencher_logger::capped(image_ref.full_name()));

    // Create pull directory if it doesn't exist
    std::fs::create_dir_all(pull_dir)?;

    info!(log, "Registry client"; "authenticated" => token.is_some());
    let mut client = if let Some(t) = token {
        bencher_oci::RegistryClient::with_token(t)?.with_scheme(scheme)
    } else {
        bencher_oci::RegistryClient::new()?.with_scheme(scheme)
    };

    client.pull(&image_ref, &image_dir)?;
    info!(log, "Image pulled"; "image_dir" => image_dir.as_str());

    Ok(image_dir)
}

/// Resolved OCI image configuration (entrypoint, cmd, env, working directory).
pub struct ResolvedOciConfig {
    /// The full command to execute (entrypoint + cmd merged).
    pub command: Vec<String>,
    /// The working directory from the OCI image config.
    pub working_dir: String,
    /// Environment variables (OCI image defaults merged with config overrides).
    pub env: Vec<(String, String)>,
}

/// Parse an OCI image config and resolve entrypoint, cmd, env, and working dir,
/// applying overrides from the runner `Config`.
///
/// Shared between `local.rs` (non-sandboxed) and `vm.rs` (Firecracker) to
/// avoid duplicating the Docker-style entrypoint/cmd merge semantics.
pub fn resolve_oci_config(
    oci_image: &bencher_oci::OciImage,
    config: &crate::Config,
) -> Result<ResolvedOciConfig, RunnerError> {
    // Apply entrypoint/cmd overrides (Config takes precedence over OCI image)
    let entrypoint = config
        .entrypoint
        .clone()
        .unwrap_or_else(|| oci_image.entrypoint());
    // Docker semantics: overriding entrypoint clears image CMD
    let cmd = if config.entrypoint.is_some() {
        config.cmd.clone().unwrap_or_default()
    } else {
        config.cmd.clone().unwrap_or_else(|| oci_image.cmd())
    };
    let command = if entrypoint.is_empty() {
        cmd
    } else {
        let mut c = entrypoint;
        c.extend(cmd);
        c
    };

    let working_dir = oci_image
        .working_dir()
        .filter(|w| !w.is_empty())
        .unwrap_or("/")
        .to_owned();

    // Apply env overrides (Config env merged on top of OCI env)
    let mut env = oci_image.env();
    if let Some(config_env) = &config.env {
        for (key, value) in config_env {
            env.retain(|(k, _)| k != key);
            env.push((key.clone(), value.clone()));
        }
    }
    if command.is_empty() {
        return Err(crate::error::ConfigError::MissingCommand.into());
    }

    Ok(ResolvedOciConfig {
        command,
        working_dir,
        env,
    })
}

/// Execute a single benchmark run with the given configuration.
///
/// Dispatches based on `config.sandbox`:
/// - `Some(Sandbox::Firecracker)` → Firecracker microVM (Linux-only)
/// - `None` → local host execution (any platform)
///
/// # Arguments
///
/// * `config` - The benchmark run configuration
/// * `cancel_flag` - Optional cancellation flag; if set to `true`, the run
///   will be aborted at its next stage or while it waits for the VM.
///
/// # Returns
///
/// The benchmark output including exit code and stdout.
pub fn execute(
    log: &Logger,
    config: &crate::Config,
    host: &mut crate::jail::HostPreparation,
    cancel_flag: Option<&AtomicBool>,
) -> Result<RunOutput, RunnerError> {
    let deadline = JobDeadline::start(Duration::from_secs(config.timeout_secs));
    execute_with_deadline(log, config, host, cancel_flag, deadline)
}

/// Like [`execute`], but the run has only what is left of `deadline`, which
/// can have started before it.
#[cfg_attr(
    not(target_os = "linux"),
    expect(
        unused_variables,
        reason = "host preparation is Linux-only, as is the VM executor it prepares for"
    )
)]
pub fn execute_with_deadline(
    log: &Logger,
    config: &crate::Config,
    host: &mut crate::jail::HostPreparation,
    cancel_flag: Option<&AtomicBool>,
    deadline: JobDeadline,
) -> Result<RunOutput, RunnerError> {
    match config.sandbox {
        Some(bencher_json::Sandbox::Firecracker) => {
            #[cfg(target_os = "linux")]
            {
                crate::vm::vm_execute(log, config, host, cancel_flag, deadline)
            }
            #[cfg(not(target_os = "linux"))]
            {
                Err(crate::error::ConfigError::UnsupportedPlatform(
                    "Firecracker sandbox requires Linux with KVM support".to_owned(),
                )
                .into())
            }
        },
        None => crate::local::local_execute(log, config, cancel_flag, deadline),
    }
}
