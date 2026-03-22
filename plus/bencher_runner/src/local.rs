//! Non-sandboxed host execution — runs commands directly on the host system.
//!
//! # Trust Model
//!
//! Non-sandboxed mode is intended for **trusted workloads only**. The benchmark
//! process runs with full host privileges — there is no filesystem sandboxing
//! and no network restrictions. The OCI image layers are unpacked to a temporary
//! directory and the command executes from there. The host environment is cleared
//! and only OCI-derived environment variables are set.
//!
//! The `--danger-allow-no-sandbox` flag on `runner up` (or omitting `--sandbox`
//! on `runner run`) gates this mode to prevent accidental use.

#![expect(
    clippy::print_stdout,
    clippy::print_stderr,
    reason = "local executor prints progress and diagnostic output"
)]

use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use camino::{Utf8Path, Utf8PathBuf};

use crate::error::RunnerError;
use crate::run::{RunOutput, resolve_oci_config, resolve_oci_image};

/// Execute a single benchmark run locally on the host system.
///
/// Pulls and unpacks the OCI image, then runs the command directly via
/// `std::process::Command` from the unpacked rootfs. No sandboxing is applied.
pub fn local_execute(
    config: &crate::Config,
    cancel_flag: Option<&Arc<AtomicBool>>,
) -> Result<RunOutput, RunnerError> {
    println!("Executing benchmark run (non-sandboxed mode):");
    println!("  OCI image: {}", config.oci_image);
    println!("  Timeout: {} seconds", config.timeout_secs);

    // Create a temporary work directory
    let temp_dir = tempfile::tempdir().map_err(crate::error::ConfigError::TempDir)?;
    let work_dir =
        Utf8Path::from_path(temp_dir.path()).ok_or(crate::error::ConfigError::NonUtf8TempDir)?;

    let unpack_dir = work_dir.join("rootfs");

    // Step 1: Resolve OCI image (local path or pull from registry)
    let oci_image_path = resolve_oci_image(
        &config.oci_image,
        config.token.as_ref().map(AsRef::as_ref),
        config.registry_scheme,
        work_dir,
    )?;

    // Step 2: Parse OCI image config to get the command
    println!("Parsing OCI image config...");
    let oci_image = bencher_oci::OciImage::parse(&oci_image_path)?;
    let oci_config = resolve_oci_config(&oci_image, config)?;

    println!("  Command: {}", oci_config.command.join(" "));
    println!("  WorkDir: {}", oci_config.working_dir);
    if !oci_config.env.is_empty() {
        println!("  Env: {} variables", oci_config.env.len());
    }

    // Step 3: Unpack OCI image layers into the rootfs directory
    println!("Unpacking OCI image to {unpack_dir}...");
    bencher_oci::unpack(&oci_image_path, &unpack_dir)?;

    // Canonicalize unpack_dir *after* unpacking so all symlinks within the
    // rootfs are materialized and we have a stable base for containment checks.
    let canonical_unpack_dir = unpack_dir.canonicalize_utf8().map_err(|e| {
        crate::error::ConfigError::Setup(format!("Failed to canonicalize unpack dir: {e}"))
    })?;

    // Step 4: Execute command from the unpacked rootfs
    println!("Running command on host...");

    let Some(program) = oci_config.command.first() else {
        return Err(crate::error::ConfigError::MissingCommand.into());
    };
    let args = oci_config.command.get(1..).unwrap_or_default();

    // Resolve program path within the unpacked rootfs.
    // Absolute paths are resolved directly; relative/bare names are searched
    // in PATH directories (mirroring how a container runtime resolves commands).
    let canonical_program = resolve_program(program, &oci_config.env, &canonical_unpack_dir)?;

    let mut cmd = Command::new(canonical_program.as_str());
    cmd.args(args);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    // Clear parent env and set only OCI-derived variables
    cmd.env_clear();
    for (key, value) in &oci_config.env {
        cmd.env(key, value);
    }

    // Set working directory inside the unpacked rootfs.
    // Canonicalize and validate it stays within the rootfs.
    let cwd = canonical_unpack_dir.join(oci_config.working_dir.trim_start_matches('/'));
    if cwd.exists() {
        let canonical_cwd = cwd.canonicalize_utf8().map_err(|e| {
            crate::error::ConfigError::Setup(format!("Failed to canonicalize working dir: {e}"))
        })?;
        if canonical_cwd.starts_with(&canonical_unpack_dir) {
            cmd.current_dir(canonical_cwd.as_std_path());
        } else {
            eprintln!("Warning: working directory escapes rootfs, falling back to rootfs root");
            cmd.current_dir(canonical_unpack_dir.as_std_path());
        }
    } else {
        cmd.current_dir(canonical_unpack_dir.as_std_path());
    }

    let child = cmd
        .spawn()
        .map_err(|e| crate::error::ConfigError::BinaryNotFound {
            name: program.clone(),
            hint: format!("Failed to spawn process: {e}"),
        })?;

    let output = wait_with_timeout(child, config.timeout_secs, cancel_flag)?;

    // Collect output files from the unpacked rootfs
    let output_files = collect_output_files(config.file_paths.as_deref(), &canonical_unpack_dir);

    Ok(RunOutput {
        exit_code: output.exit_code,
        stdout: output.stdout,
        stderr: output.stderr,
        output_files,
    })
}

/// Output from waiting on a child process.
struct WaitOutput {
    exit_code: i32,
    stdout: String,
    stderr: String,
}

/// Wait for a child process with timeout and cancellation support.
///
/// Spawns a background thread to call `wait_with_output`. The main thread
/// polls for timeout or cancellation, killing the child process if either
/// triggers. On timeout/cancel the thread is joined to ensure cleanup.
fn wait_with_timeout(
    child: std::process::Child,
    timeout_secs: u64,
    cancel_flag: Option<&Arc<AtomicBool>>,
) -> Result<WaitOutput, RunnerError> {
    let timeout = Duration::from_secs(timeout_secs);
    let start = Instant::now();

    // Save PID before the thread consumes the child, so we can reliably kill
    // the process even after `wait_with_output` takes ownership.
    let pid = child.id();

    let handle = std::thread::spawn(move || child.wait_with_output());

    // Poll for completion, timeout, or cancellation
    loop {
        if handle.is_finished() {
            break;
        }

        if start.elapsed() > timeout {
            kill_by_pid(pid);
            drop(handle.join());
            return Err(crate::error::ConfigError::Timeout(format!(
                "process did not complete within {timeout_secs}s"
            ))
            .into());
        }

        if let Some(flag) = cancel_flag
            && flag.load(Ordering::SeqCst)
        {
            kill_by_pid(pid);
            drop(handle.join());
            return Err(crate::error::ConfigError::Canceled("job was canceled".to_owned()).into());
        }

        std::thread::sleep(Duration::from_millis(100));
    }

    let output = handle.join().map_err(|panic| {
        let msg = panic
            .downcast_ref::<&str>()
            .copied()
            .or_else(|| panic.downcast_ref::<String>().map(String::as_str))
            .unwrap_or("unknown panic");
        std::io::Error::other(format!("child thread panicked: {msg}"))
    })??;

    let exit_code = output.status.code().unwrap_or_else(|| {
        #[cfg(unix)]
        {
            use std::os::unix::process::ExitStatusExt as _;
            output.status.signal().map_or(-1, |sig| 128 + sig)
        }
        #[cfg(not(unix))]
        {
            -1
        }
    });
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();

    Ok(WaitOutput {
        exit_code,
        stdout,
        stderr,
    })
}

/// Send SIGKILL to a process by PID. Best-effort; errors are silently ignored
/// because the process may have already exited.
#[cfg(unix)]
fn kill_by_pid(pid: u32) {
    // SAFETY: We send SIGKILL to a known child PID. If the process has already
    // exited the call is harmless (returns ESRCH).
    #[expect(unsafe_code, clippy::cast_possible_wrap)]
    unsafe {
        libc::kill(pid as i32, libc::SIGKILL);
    }
}

/// Non-Unix fallback: uses `std::process::Command` to kill by PID.
#[cfg(not(unix))]
fn kill_by_pid(pid: u32) {
    // On Windows, `taskkill /F /PID <pid>` forcefully terminates the process.
    drop(
        std::process::Command::new("taskkill")
            .args(["/F", "/PID", &pid.to_string()])
            .output(),
    );
}

/// Default PATH used when the OCI image does not specify one.
const DEFAULT_PATH: &str = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin";

/// Resolve a program name to a canonical path within the unpacked rootfs.
///
/// - Absolute paths (e.g. `/bin/sh`) are resolved directly within the rootfs.
/// - Bare names (e.g. `echo`) are searched in each PATH directory within the
///   rootfs, mirroring how a container runtime resolves commands.
///
/// The resolved path is canonicalized and validated to stay within `unpack_dir`
/// to prevent path traversal.
fn resolve_program(
    program: &str,
    env: &[(String, String)],
    unpack_dir: &Utf8Path,
) -> Result<Utf8PathBuf, RunnerError> {
    if program.starts_with('/') || program.contains('/') {
        // Absolute or relative path — resolve directly
        let program_path = unpack_dir.join(program.trim_start_matches('/'));
        return canonicalize_and_check(program, &program_path, unpack_dir);
    }

    // Bare command name — search PATH directories within the rootfs
    let path_val = env
        .iter()
        .find(|(k, _)| k == "PATH")
        .map_or(DEFAULT_PATH, |(_, v)| v.as_str());

    for dir in path_val.split(':') {
        let candidate = unpack_dir.join(dir.trim_start_matches('/')).join(program);
        if candidate.exists() {
            return canonicalize_and_check(program, &candidate, unpack_dir);
        }
    }

    Err(crate::error::ConfigError::BinaryNotFound {
        name: program.to_owned(),
        hint: format!("not found in PATH directories within rootfs (PATH={path_val})"),
    }
    .into())
}

/// Canonicalize a candidate path and verify it stays within `unpack_dir`.
fn canonicalize_and_check(
    program: &str,
    candidate: &Utf8Path,
    unpack_dir: &Utf8Path,
) -> Result<Utf8PathBuf, RunnerError> {
    let canonical =
        candidate
            .canonicalize_utf8()
            .map_err(|e| crate::error::ConfigError::BinaryNotFound {
                name: program.to_owned(),
                hint: format!("Failed to resolve program path: {e}"),
            })?;
    if !canonical.starts_with(unpack_dir) {
        return Err(crate::error::ConfigError::BinaryNotFound {
            name: program.to_owned(),
            hint: "program path escapes the unpacked rootfs".to_owned(),
        }
        .into());
    }
    Ok(canonical)
}

/// Collect output files from the unpacked rootfs.
///
/// OCI file paths are specified relative to the container root. We resolve them
/// relative to `unpack_dir` and validate they don't escape the rootfs.
fn collect_output_files(
    file_paths: Option<&[Utf8PathBuf]>,
    unpack_dir: &Utf8Path,
) -> Option<HashMap<Utf8PathBuf, Vec<u8>>> {
    let paths = file_paths?;

    let mut files = HashMap::with_capacity(paths.len());
    for path in paths {
        // Resolve the OCI-relative path within the unpacked rootfs
        let host_path = unpack_dir.join(path.as_str().trim_start_matches('/'));
        // Validate the resolved path stays within unpack_dir
        let Ok(canonical) = host_path.canonicalize_utf8() else {
            eprintln!("Warning: output file does not exist: {path}");
            continue;
        };
        if !canonical.starts_with(unpack_dir) {
            eprintln!("Warning: output file path escapes rootfs: {path}");
            continue;
        }
        match std::fs::read(canonical.as_std_path()) {
            Ok(contents) => {
                files.insert(path.clone(), contents);
            },
            Err(e) => {
                eprintln!("Warning: failed to read output file {path}: {e}");
            },
        }
    }

    if files.is_empty() { None } else { Some(files) }
}
