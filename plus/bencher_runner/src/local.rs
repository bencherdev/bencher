//! Non-sandboxed host execution — runs commands directly on the host system.
//!
//! # Trust Model
//!
//! Non-sandboxed mode is intended for **trusted workloads only**. The benchmark
//! process runs with full host privileges — there is no environment isolation,
//! no filesystem sandboxing, and no network restrictions. The OCI image layers
//! are unpacked to a temporary directory and the command executes from there
//! with the host's environment inherited.
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

    // Resolve program path relative to the unpacked rootfs
    let program_path = unpack_dir.join(program.trim_start_matches('/'));

    let mut cmd = Command::new(program_path.as_str());
    cmd.args(args);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    // Set env vars from OCI config + overrides (host env is inherited)
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

    // Collect output files directly from the host filesystem
    let output_files = collect_output_files(config.file_paths.as_deref());

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
fn wait_with_timeout(
    child: std::process::Child,
    timeout_secs: u64,
    cancel_flag: Option<&Arc<AtomicBool>>,
) -> Result<WaitOutput, RunnerError> {
    let timeout = Duration::from_secs(timeout_secs);
    let start = Instant::now();

    // We need to capture stdout/stderr, so use piped I/O via wait_with_output
    // in a separate thread with timeout checking.
    let child_arc = Arc::new(std::sync::Mutex::new(Some(child)));
    let child_thread = Arc::clone(&child_arc);

    let handle = std::thread::spawn(move || -> Result<std::process::Output, std::io::Error> {
        // These are safe to unwrap: the lock is never poisoned (only this thread
        // and the polling loop access it), and the child is always Some (only
        // taken once here).
        let Ok(mut guard) = child_thread.lock() else {
            return Err(std::io::Error::other("child lock poisoned"));
        };
        let Some(child) = guard.take() else {
            return Err(std::io::Error::other("child already taken"));
        };
        child.wait_with_output()
    });

    // Poll for completion, timeout, or cancellation
    loop {
        if handle.is_finished() {
            break;
        }

        if start.elapsed() > timeout {
            // Kill the child process on timeout
            if let Ok(mut guard) = child_arc.lock()
                && let Some(ref mut child) = *guard
            {
                drop(child.kill());
            }
            return Err(crate::error::ConfigError::Runtime {
                kind: "timeout",
                message: format!("process did not complete within {timeout_secs}s"),
            }
            .into());
        }

        if let Some(flag) = cancel_flag
            && flag.load(Ordering::SeqCst)
        {
            // Kill the child process on cancellation
            if let Ok(mut guard) = child_arc.lock()
                && let Some(ref mut child) = *guard
            {
                drop(child.kill());
            }
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

/// Collect output files directly from the host filesystem.
fn collect_output_files(
    file_paths: Option<&[Utf8PathBuf]>,
) -> Option<HashMap<Utf8PathBuf, Vec<u8>>> {
    let paths = file_paths?;

    let mut files = HashMap::with_capacity(paths.len());
    for path in paths {
        match std::fs::read(path.as_std_path()) {
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
