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

#![expect(clippy::print_stdout, clippy::print_stderr)]

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

    // Step 4: Execute command from the unpacked rootfs
    println!("Running command on host...");

    let Some(program) = oci_config.command.first() else {
        return Err(crate::error::ConfigError::MissingCommand.into());
    };
    let args = oci_config.command.get(1..).unwrap_or_default();

    // Resolve program path relative to the unpacked rootfs and validate it
    // stays within the rootfs to prevent path traversal (e.g. `../../bin/sh`).
    let program_path = unpack_dir.join(program.trim_start_matches('/'));
    let canonical_program = program_path.canonicalize_utf8().map_err(|e| {
        crate::error::ConfigError::BinaryNotFound {
            name: program.clone(),
            hint: format!("Failed to resolve program path: {e}"),
        }
    })?;
    if !canonical_program.starts_with(&unpack_dir) {
        return Err(crate::error::ConfigError::BinaryNotFound {
            name: program.clone(),
            hint: "program path escapes the unpacked rootfs".to_owned(),
        }
        .into());
    }

    let mut cmd = Command::new(canonical_program.as_str());
    cmd.args(args);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    // Clear parent env and set only OCI-derived variables
    cmd.env_clear();
    for (key, value) in &oci_config.env {
        cmd.env(key, value);
    }

    // Set working directory inside the unpacked rootfs
    let cwd = unpack_dir.join(oci_config.working_dir.trim_start_matches('/'));
    if cwd.exists() {
        cmd.current_dir(cwd.as_std_path());
    } else {
        cmd.current_dir(unpack_dir.as_std_path());
    }

    let child = cmd
        .spawn()
        .map_err(|e| crate::error::ConfigError::BinaryNotFound {
            name: program.clone(),
            hint: format!("Failed to spawn process: {e}"),
        })?;

    let output = wait_with_timeout(child, config.timeout_secs, cancel_flag)?;

    // Collect output files from the unpacked rootfs
    let output_files = collect_output_files(config.file_paths.as_deref(), &unpack_dir);

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
/// Saves the child PID before spawning the wait thread so that timeout/cancel
/// can reliably signal the process via `libc::kill` even after `wait_with_output`
/// has consumed the `Child` handle.
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
            return Err(crate::error::ConfigError::Runtime {
                kind: "timeout",
                message: format!("process did not complete within {timeout_secs}s"),
            }
            .into());
        }

        if let Some(flag) = cancel_flag
            && flag.load(Ordering::SeqCst)
        {
            kill_by_pid(pid);
            return Err(crate::error::ConfigError::Runtime {
                kind: "canceled",
                message: "job was canceled".to_owned(),
            }
            .into());
        }

        std::thread::sleep(Duration::from_millis(100));
    }

    let output = handle
        .join()
        .map_err(|_panic| std::io::Error::other("child thread panicked"))??;

    let exit_code = output.status.code().unwrap_or(-1);
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
fn kill_by_pid(pid: u32) {
    // SAFETY: We send SIGKILL to a known child PID. If the process has already
    // exited the call is harmless (returns ESRCH).
    #[expect(unsafe_code, clippy::cast_possible_wrap)]
    unsafe {
        libc::kill(pid as i32, libc::SIGKILL);
    }
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
