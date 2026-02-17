#![expect(clippy::print_stdout)]

use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use camino::{Utf8Path, Utf8PathBuf};

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
#[cfg(target_os = "linux")]
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
    /// Whether to enable network access in the VM.
    pub network: bool,
    /// Host tuning configuration.
    pub tuning: TuningConfig,
    /// Firecracker process log level.
    #[cfg(target_os = "linux")]
    pub firecracker_log_level: crate::firecracker::FirecrackerLogLevel,
}

/// Run the `run` subcommand with parsed arguments.
///
/// Prepares the rootfs and launches a Firecracker microVM.
#[cfg(target_os = "linux")]
pub fn run_with_args(args: &RunArgs) -> Result<(), RunnerError> {
    // Apply host tuning — guard restores settings on drop
    let _tuning_guard = crate::tuning::apply(&args.tuning);

    // Build config from args
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
    let config = if let Some(ref token) = args.token {
        config.with_token(token.clone())
    } else {
        config
    };
    let config = if let Some(ref file_paths) = args.file_paths {
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
    config.firecracker_log_level = args.firecracker_log_level;

    let output = execute(&config, None)?;
    println!("{}", output.stdout);
    if !output.stderr.is_empty() {
        eprintln!("{}", output.stderr);
    }
    Ok(())
}

/// Non-Linux stub for `run_with_args`.
#[cfg(not(target_os = "linux"))]
pub fn run_with_args(_args: &RunArgs) -> Result<(), RunnerError> {
    Err(RunnerError::Config(
        "bencher-runner requires Linux".to_owned(),
    ))
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
    let image_ref = bencher_oci::ImageReference::parse(oci_image)?;

    // Pull into the provided directory
    let image_dir = pull_dir.join("oci-image");

    // Pull from registry
    println!("Pulling from registry: {}", image_ref.full_name());

    // Create pull directory if it doesn't exist
    std::fs::create_dir_all(pull_dir)?;

    let mut client = if let Some(t) = token {
        println!("  Using authenticated client");
        bencher_oci::RegistryClient::with_token(t)?
    } else {
        println!("  Using anonymous client");
        bencher_oci::RegistryClient::new()?
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
    use crate::firecracker::{FirecrackerJobConfig, run_firecracker};

    println!("Executing benchmark run:");
    println!("  OCI image: {}", config.oci_image);
    println!(
        "  Kernel: {}",
        config.kernel.as_ref().map_or("(system)", |p| p.as_str())
    );
    println!("  vCPUs: {}", config.vcpus);
    println!("  Memory: {} MiB", config.memory.to_mib());
    println!("  Timeout: {} seconds", config.timeout_secs);

    // Create a temporary work directory
    let temp_dir = tempfile::tempdir()
        .map_err(|e| RunnerError::Config(format!("Failed to create temp directory: {e}")))?;
    let work_dir = Utf8Path::from_path(temp_dir.path())
        .ok_or_else(|| RunnerError::Config("Temp directory path is not UTF-8".to_owned()))?;

    let unpack_dir = work_dir.join("rootfs");
    let rootfs_path = work_dir.join("rootfs.ext4");

    // Get kernel path - use bundled, provided, or find system kernel
    let kernel_path = if let Some(ref kernel) = config.kernel {
        kernel.clone()
    } else if crate::kernel::KERNEL_BUNDLED {
        let kernel_dest = work_dir.join("vmlinux");
        crate::kernel::write_kernel_to_file(kernel_dest.as_std_path())?;
        println!("  Extracted bundled kernel to {kernel_dest}");
        kernel_dest
    } else {
        find_kernel()?
    };

    // Step 1: Resolve OCI image (local path or pull from registry)
    let oci_image_path = resolve_oci_image(&config.oci_image, config.token.as_deref(), work_dir)?;

    // Step 2: Parse OCI image config to get the command
    println!("Parsing OCI image config...");
    let oci_image = bencher_oci::OciImage::parse(&oci_image_path)?;
    let command = oci_image.command();
    let workdir = oci_image
        .working_dir()
        .filter(|w| !w.is_empty())
        .unwrap_or("/");
    // Sanitize environment variables to remove dangerous ones like LD_PRELOAD
    let env = sanitize_env(&oci_image.env());

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

    // Step 4: Write command config for the VM
    println!("Writing init config...");
    write_init_config(
        &unpack_dir,
        &command,
        workdir,
        &env,
        config.file_paths.as_deref(),
        config.max_output_size,
    )?;

    // Step 5: Install init binary
    println!("Installing init binary...");
    install_init_binary(&unpack_dir)?;

    // Step 6: Create ext4 rootfs
    println!(
        "Creating ext4 at {rootfs_path} ({} MiB)...",
        config.disk.to_mib()
    );
    bencher_rootfs::create_ext4_with_size(&unpack_dir, &rootfs_path, config.disk.to_mib())?;

    // Step 7: Find Firecracker binary - use bundled or find on system
    let firecracker_bin = if crate::firecracker_bin::FIRECRACKER_BUNDLED {
        let fc_dest = work_dir.join("firecracker");
        crate::firecracker_bin::write_firecracker_to_file(fc_dest.as_std_path())?;
        println!("  Extracted bundled firecracker to {fc_dest}");
        fc_dest
    } else {
        find_firecracker_binary()?
    };

    // Step 8: Run benchmark in Firecracker microVM
    println!("Launching Firecracker microVM...");
    #[expect(
        clippy::cast_possible_truncation,
        reason = "CPU count fits in u8 for Firecracker"
    )]
    let vcpus = u32::from(config.vcpus) as u8;
    #[expect(
        clippy::cast_possible_truncation,
        reason = "Practical memory fits in u32 MiB for Firecracker"
    )]
    let memory_mib = config.memory.to_mib() as u32;
    let fc_config = FirecrackerJobConfig {
        firecracker_bin,
        kernel_path,
        rootfs_path,
        vcpus,
        memory_mib,
        boot_args: config.kernel_cmdline.clone(),
        timeout_secs: config.timeout_secs,
        work_dir: work_dir.to_owned(),
        cpu_layout: config.cpu_layout.clone(),
        log_level: config.firecracker_log_level,
        max_file_count: config.max_file_count,
        max_output_size: config.max_output_size,
    };

    let run_output = run_firecracker(&fc_config, cancel_flag)?;

    Ok(run_output)
}

/// Write the init config for the VM.
///
/// This creates `/etc/bencher/config.json` which is read by `bencher-init`.
#[cfg(target_os = "linux")]
fn write_init_config(
    rootfs: &camino::Utf8Path,
    command: &[String],
    workdir: &str,
    env: &[(String, String)],
    file_paths: Option<&[Utf8PathBuf]>,
    max_output_size: usize,
) -> Result<(), RunnerError> {
    use std::fs;

    let config_dir = rootfs.join("etc/bencher");
    fs::create_dir_all(&config_dir)?;

    // Build the config JSON
    let config = serde_json::json!({
        "command": command,
        "workdir": workdir,
        "env": env,
        "file_paths": file_paths,
        "max_output_size": max_output_size,
    });

    let config_path = config_dir.join("config.json");
    let config_str = serde_json::to_string_pretty(&config)
        .map_err(|e| RunnerError::Config(format!("failed to serialize config: {e}")))?;
    fs::write(&config_path, config_str)?;

    Ok(())
}

/// Install the bencher-init binary into the rootfs at /init.
///
/// Uses the bundled init binary if available, otherwise falls back to searching on disk.
#[cfg(target_os = "linux")]
fn install_init_binary(rootfs: &camino::Utf8Path) -> Result<(), RunnerError> {
    use crate::init;

    let dest_path = rootfs.join("init");

    if init::INIT_BUNDLED {
        // Use the bundled init binary
        init::write_init_to_file(dest_path.as_std_path())?;
    } else {
        // Fall back to searching for the binary on disk
        let init_binary = find_init_binary()?;

        std::fs::copy(&init_binary, &dest_path).map_err(|e| {
            RunnerError::Config(format!(
                "failed to copy init binary from {init_binary} to {dest_path}: {e}",
            ))
        })?;

        // Make it executable
        use std::os::unix::fs::PermissionsExt as _;
        let mut perms = std::fs::metadata(&dest_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&dest_path, perms)?;
    }

    Ok(())
}

/// Find the bencher-init binary on disk (fallback when not bundled).
#[cfg(target_os = "linux")]
fn find_init_binary() -> Result<Utf8PathBuf, RunnerError> {
    // Look in these locations in order
    let candidates = [
        // Next to the current executable
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("bencher-init")))
            .and_then(|p| Utf8PathBuf::try_from(p).ok()),
        // Common installation paths
        Some(Utf8PathBuf::from("/usr/local/bin/bencher-init")),
        Some(Utf8PathBuf::from("/usr/bin/bencher-init")),
    ];

    for candidate in candidates.into_iter().flatten() {
        if candidate.as_std_path().exists() {
            return Ok(candidate);
        }
    }

    Err(RunnerError::Config(
        "bencher-init binary not found. Build with: cargo build -p bencher_init".to_owned(),
    ))
}

/// Find the Firecracker binary on the system.
#[cfg(target_os = "linux")]
fn find_firecracker_binary() -> Result<Utf8PathBuf, RunnerError> {
    let candidates = [
        // Next to the current executable
        std::env::current_exe()
            .ok()
            .and_then(|p| p.parent().map(|d| d.join("firecracker")))
            .and_then(|p| Utf8PathBuf::try_from(p).ok()),
        // Common installation paths
        Some(Utf8PathBuf::from("/usr/local/bin/firecracker")),
        Some(Utf8PathBuf::from("/usr/bin/firecracker")),
    ];

    for candidate in candidates.into_iter().flatten() {
        if candidate.as_std_path().exists() {
            return Ok(candidate);
        }
    }

    Err(RunnerError::Config(
        "firecracker binary not found. Install from: https://github.com/firecracker-microvm/firecracker/releases".to_owned(),
    ))
}

/// Find the kernel image on the system.
#[cfg(target_os = "linux")]
fn find_kernel() -> Result<Utf8PathBuf, RunnerError> {
    let candidates = [
        // Bencher's shared location
        "/usr/local/share/bencher/vmlinux",
        // Next to the current executable
    ];

    for candidate in candidates {
        if std::path::Path::new(candidate).exists() {
            return Ok(Utf8PathBuf::from(candidate));
        }
    }

    // Try next to the current executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(parent) = exe.parent() {
            let kernel = parent.join("vmlinux");
            if kernel.exists() {
                if let Some(path) = kernel.to_str() {
                    return Ok(Utf8PathBuf::from(path));
                }
            }
        }
    }

    Err(RunnerError::Config(
        "kernel image (vmlinux) not found. Place at /usr/local/share/bencher/vmlinux".to_owned(),
    ))
}

/// Sanitize environment variables by removing dangerous ones.
///
/// This filters out environment variables that could be used to inject
/// malicious code into the guest process, such as LD_PRELOAD.
#[cfg(target_os = "linux")]
fn sanitize_env(env: &[(String, String)]) -> Vec<(String, String)> {
    let mut sanitized = Vec::with_capacity(env.len());
    let mut blocked_count = 0;

    for (key, value) in env {
        let key_upper = key.to_uppercase();
        let is_blocked = BLOCKED_ENV_VARS
            .iter()
            .any(|blocked| key_upper == *blocked || key_upper.starts_with(&format!("{blocked}_")));

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

/// Execute a single benchmark run (non-Linux stub).
#[cfg(not(target_os = "linux"))]
pub fn execute(
    _config: &crate::Config,
    _cancel_flag: Option<&Arc<AtomicBool>>,
) -> Result<RunOutput, RunnerError> {
    Err(RunnerError::Config(
        "Benchmark execution requires Linux with KVM support".to_owned(),
    ))
}

#[cfg(test)]
#[cfg(target_os = "linux")]
mod tests {
    use super::*;

    fn env(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
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
