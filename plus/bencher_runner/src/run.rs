#![expect(clippy::print_stdout)]
#![cfg_attr(
    any(target_os = "linux", debug_assertions),
    expect(clippy::print_stderr)
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

/// Environment variables that are blocked for security reasons.
///
/// These variables could be used to inject malicious code or libraries
/// into the guest process if passed through from the OCI image.
const BLOCKED_ENV_VARS: &[&str] = &[
    // Dynamic linker variables - could load malicious libraries
    "LD_PRELOAD",
    "LD_LIBRARY_PATH",
    "LD_AUDIT",
    "LD_DEBUG",
    "LD_DEBUG_OUTPUT",
    "LD_DYNAMIC_WEAK",
    "LD_HWCAP_MASK",
    "LD_ORIGIN_PATH",
    "LD_POINTER_GUARD",
    "LD_PROFILE",
    "LD_PROFILE_OUTPUT",
    "LD_SHOW_AUXV",
    "LD_USE_LOAD_BIAS",
    "LD_BIND_NOW",
    "LD_BIND_NOT",
    // glibc malloc hooks
    "MALLOC_CHECK_",
    "MALLOC_TRACE",
    // Other potentially dangerous variables
    "BASH_ENV",
    "ENV",
    "CDPATH",
    "GLOBIGNORE",
    "IFS",
];

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
    /// Firecracker process log level.
    pub firecracker_log_level: crate::FirecrackerLogLevel,
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
    config.firecracker_log_level = args.firecracker_log_level;
    config
}

/// Run the `run` subcommand with parsed arguments.
///
/// Prepares the rootfs and launches a Firecracker microVM.
#[cfg(target_os = "linux")]
pub fn run_with_args(args: &RunArgs) -> Result<(), RunnerError> {
    // Apply host tuning — guard restores settings on drop
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

/// Non-Linux debug stub for `run_with_args` — local host execution.
#[cfg(all(debug_assertions, not(target_os = "linux")))]
pub fn run_with_args(args: &RunArgs) -> Result<(), RunnerError> {
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

/// Non-Linux release stub for `run_with_args`.
#[cfg(all(not(debug_assertions), not(target_os = "linux")))]
pub fn run_with_args(_args: &RunArgs) -> Result<(), RunnerError> {
    Err(
        crate::error::ConfigError::UnsupportedPlatform("bencher-runner requires Linux".to_owned())
            .into(),
    )
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

/// Execute a single benchmark run with the given configuration.
///
/// Prepares the rootfs and launches a Firecracker microVM.
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
#[cfg(target_os = "linux")]
pub fn execute(
    config: &crate::Config,
    cancel_flag: Option<&Arc<AtomicBool>>,
) -> Result<RunOutput, RunnerError> {
    crate::vm::vm_execute(config, cancel_flag)
}

/// Execute a single benchmark run (non-Linux debug — local host execution).
#[cfg(all(debug_assertions, not(target_os = "linux")))]
pub fn execute(
    config: &crate::Config,
    cancel_flag: Option<&Arc<AtomicBool>>,
) -> Result<RunOutput, RunnerError> {
    crate::local::local_execute(config, cancel_flag)
}

/// Sanitize environment variables by removing dangerous ones.
///
/// This filters out environment variables that could be used to inject
/// malicious code into the guest process, such as `LD_PRELOAD`.
pub(crate) fn sanitize_env(env: &[(String, String)]) -> Vec<(String, String)> {
    let mut sanitized = Vec::with_capacity(env.len());
    let mut blocked_count = 0;

    for (key, value) in env {
        let key_upper = key.to_uppercase();
        let is_blocked = BLOCKED_ENV_VARS.iter().any(|blocked| {
            key_upper == *blocked
                || (key_upper.starts_with(blocked)
                    && key_upper.as_bytes().get(blocked.len()) == Some(&b'_'))
        });

        if is_blocked {
            blocked_count += 1;
        } else {
            sanitized.push((key.clone(), value.clone()));
        }
    }

    if blocked_count > 0 {
        println!("  Blocked {blocked_count} dangerous environment variable(s)");
    }

    sanitized
}

/// Execute a single benchmark run (non-Linux release stub).
#[cfg(all(not(debug_assertions), not(target_os = "linux")))]
pub fn execute(
    _config: &crate::Config,
    _cancel_flag: Option<&Arc<AtomicBool>>,
) -> Result<RunOutput, RunnerError> {
    Err(crate::error::ConfigError::UnsupportedPlatform(
        "Benchmark execution requires Linux with KVM support".to_owned(),
    )
    .into())
}

#[cfg(test)]
#[expect(clippy::indexing_slicing, clippy::str_to_string)]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| ((*k).to_string(), (*v).to_string()))
            .collect()
    }

    #[test]
    fn sanitize_env_passes_safe_vars() {
        let input = env(&[("PATH", "/usr/bin"), ("HOME", "/root"), ("LANG", "C")]);
        let result = sanitize_env(&input);
        assert_eq!(result.len(), 3);
        assert_eq!(result[0].0, "PATH");
    }

    #[test]
    fn sanitize_env_blocks_ld_preload() {
        let input = env(&[("LD_PRELOAD", "/evil.so"), ("PATH", "/usr/bin")]);
        let result = sanitize_env(&input);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "PATH");
    }

    #[test]
    fn sanitize_env_blocks_ld_library_path() {
        let input = env(&[("LD_LIBRARY_PATH", "/tmp"), ("HOME", "/root")]);
        let result = sanitize_env(&input);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, "HOME");
    }

    #[test]
    fn sanitize_env_blocks_all_known_dangerous_vars() {
        let input = env(&[
            ("LD_PRELOAD", "x"),
            ("LD_LIBRARY_PATH", "x"),
            ("LD_AUDIT", "x"),
            ("LD_DEBUG", "x"),
            ("LD_DEBUG_OUTPUT", "x"),
            ("LD_DYNAMIC_WEAK", "x"),
            ("LD_HWCAP_MASK", "x"),
            ("LD_ORIGIN_PATH", "x"),
            ("LD_POINTER_GUARD", "x"),
            ("LD_PROFILE", "x"),
            ("LD_PROFILE_OUTPUT", "x"),
            ("LD_SHOW_AUXV", "x"),
            ("LD_USE_LOAD_BIAS", "x"),
            ("LD_BIND_NOW", "x"),
            ("LD_BIND_NOT", "x"),
            ("MALLOC_CHECK_", "x"),
            ("MALLOC_TRACE", "x"),
            ("BASH_ENV", "x"),
            ("ENV", "x"),
            ("CDPATH", "x"),
            ("GLOBIGNORE", "x"),
            ("IFS", "x"),
        ]);
        let result = sanitize_env(&input);
        assert!(
            result.is_empty(),
            "all dangerous vars should be blocked, got: {result:?}"
        );
    }

    #[test]
    fn sanitize_env_case_insensitive() {
        let input = env(&[("ld_preload", "/evil.so"), ("Ld_Library_Path", "/tmp")]);
        let result = sanitize_env(&input);
        assert!(
            result.is_empty(),
            "case-insensitive matching should block lowercase variants"
        );
    }

    #[test]
    fn sanitize_env_blocks_prefixed_variants() {
        let input = env(&[("LD_PRELOAD_32", "/evil.so"), ("MALLOC_CHECK__FOO", "1")]);
        let result = sanitize_env(&input);
        assert!(
            result.is_empty(),
            "prefix-suffixed variants should be blocked"
        );
    }

    #[test]
    fn sanitize_env_empty_input() {
        let result = sanitize_env(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn sanitize_env_preserves_order() {
        let input = env(&[("A", "1"), ("B", "2"), ("C", "3")]);
        let result = sanitize_env(&input);
        assert_eq!(result[0].0, "A");
        assert_eq!(result[1].0, "B");
        assert_eq!(result[2].0, "C");
    }
}
