#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "runner prints progress and diagnostic output"
)]

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use camino::{Utf8Path, Utf8PathBuf};

use bencher_json::Iteration;

use crate::error::RunnerError;
use crate::tuning::TuningConfig;

/// Output from a benchmark run.
#[derive(Debug)]
pub struct RunOutput {
    /// Exit code from the guest process.
    pub exit_code: i32,
    /// Stdout output from the benchmark.
    pub stdout: String,
    /// Stderr output from the benchmark.
    pub stderr: String,
    /// Optional output files: path → contents.
    pub output_files: Option<HashMap<Utf8PathBuf, Vec<u8>>>,
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
    /// Execution timeout in seconds.
    pub timeout_secs: u64,
    /// Output file paths inside guest.
    pub file_paths: Option<Vec<Utf8PathBuf>>,
    /// Maximum size in bytes for collected stdout/stderr.
    pub max_output_size: Option<usize>,
    /// Maximum number of output files to decode.
    pub max_file_count: Option<u32>,
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
    /// Allow benchmark failure without short-circuiting iterations.
    pub allow_failure: bool,
    /// Host tuning configuration.
    pub tuning: TuningConfig,
    /// Grace period in seconds after exit code before final collection.
    pub grace_period: bencher_json::GracePeriod,
    /// Sandbox process log level.
    pub sandbox_log_level: crate::SandboxLogLevel,
    /// Sandbox mode for benchmark execution.
    pub sandbox: Option<bencher_json::Sandbox>,
}

/// Build a `Config` from CLI `RunArgs`.
///
/// Shared between the Linux and non-Linux debug `run_with_args` paths.
fn build_config_from_run_args(args: &RunArgs) -> crate::Config {
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
        config.with_token(token.clone())
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
    let mut config = if let Some(max_file_count) = args.max_file_count {
        config.with_max_file_count(max_file_count)
    } else {
        config
    };
    config = config
        .with_entrypoint_opt(args.entrypoint.clone())
        .with_cmd_opt(args.cmd.clone())
        .with_env_opt(args.env.clone());
    config = config.with_grace_period(args.grace_period);
    config.sandbox_log_level = args.sandbox_log_level;
    config = config.with_sandbox(args.sandbox);
    config
}

/// Run the `run` subcommand with parsed arguments.
///
/// Dispatches to Firecracker VM or local host execution based on the sandbox
/// setting in the configuration.
pub fn run_with_args(args: &RunArgs) -> Result<(), RunnerError> {
    // Apply host tuning — guard restores settings on drop (no-op on non-Linux)
    let _tuning_guard = crate::tuning::apply(&args.tuning);

    let config = build_config_from_run_args(args);

    let iter_count = args.iter.as_usize();
    for iteration in 0..iter_count {
        match execute(&config, None) {
            Ok(output) => {
                println!("{}", output.stdout);
                if !output.stderr.is_empty() {
                    eprintln!("{}", output.stderr);
                }
                if output.exit_code != 0 && !args.allow_failure {
                    return Err(RunnerError::NonZeroExitCode(output.exit_code));
                }
            },
            Err(e) => {
                if args.allow_failure {
                    eprintln!(
                        "Iteration {}/{iter_count} failed (allow_failure=true, skipping): {e}",
                        iteration + 1
                    );
                    continue;
                }
                return Err(e);
            },
        }
    }
    Ok(())
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
    oci_image: &str,
    token: Option<&str>,
    scheme: bencher_oci::RegistryScheme,
    pull_dir: &Utf8Path,
) -> Result<Utf8PathBuf, RunnerError> {
    let path = Utf8Path::new(oci_image);

    // If it's a local path that exists, use it directly
    if path.exists() {
        println!("Using local OCI image: {oci_image}");
        return Ok(path.to_owned());
    }

    // Otherwise, treat as a registry reference
    println!("Parsing registry reference: {oci_image}");
    let image_ref = bencher_oci::ImageReference::parse(oci_image)
        .map_err(|e| bencher_oci::OciError::InvalidReference(e.to_string()))?;

    // Pull into the provided directory
    let image_dir = pull_dir.join("oci-image");

    // Pull from registry
    println!("Pulling from registry: {}", image_ref.full_name());

    // Create pull directory if it doesn't exist
    std::fs::create_dir_all(pull_dir)?;

    let mut client = if let Some(t) = token {
        println!("  Using authenticated client");
        bencher_oci::RegistryClient::with_token(t)?.with_scheme(scheme)
    } else {
        println!("  Using anonymous client");
        bencher_oci::RegistryClient::new()?.with_scheme(scheme)
    };

    client.pull(&image_ref, &image_dir)?;
    println!("Image pulled to: {image_dir}");

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
///   will be aborted as soon as the vsock polling loop detects it.
///
/// # Returns
///
/// The benchmark output including exit code and stdout.
pub fn execute(
    config: &crate::Config,
    cancel_flag: Option<&Arc<AtomicBool>>,
) -> Result<RunOutput, RunnerError> {
    match config.sandbox {
        Some(bencher_json::Sandbox::Firecracker) => {
            #[cfg(target_os = "linux")]
            {
                crate::vm::vm_execute(config, cancel_flag)
            }
            #[cfg(not(target_os = "linux"))]
            {
                Err(crate::error::ConfigError::UnsupportedPlatform(
                    "Firecracker sandbox requires Linux with KVM support".to_owned(),
                )
                .into())
            }
        },
        None => crate::local::local_execute(config, cancel_flag),
    }
}
