//! Non-sandboxed host execution — runs commands directly on the host system.
//!
//! Pulls the OCI image from the registry (exercising the real pull path),
//! parses its config for entrypoint/cmd/env, but does NOT unpack the image
//! layers. Instead, the command is executed directly on the host via
//! `std::process::Command`.

#![expect(clippy::print_stdout, clippy::print_stderr)]

use std::collections::HashMap;
use std::process::Command;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use camino::{Utf8Path, Utf8PathBuf};

use crate::error::RunnerError;
use crate::run::{RunOutput, resolve_oci_image, sanitize_env};

/// Execute a single benchmark run locally on the host system.
///
/// Pulls the OCI image, parses its config, and runs the command directly
/// via `std::process::Command` instead of booting a Firecracker microVM.
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

    // Apply env overrides (Config env merged on top of OCI env, then sanitize)
    let mut env = oci_image.env();
    if let Some(config_env) = &config.env {
        for (key, value) in config_env {
            env.retain(|(k, _)| k != key);
            env.push((key.clone(), value.clone()));
        }
    }
    let env = sanitize_env(&env);

    if command.is_empty() {
        return Err(crate::error::ConfigError::MissingCommand.into());
    }

    println!("  Command: {}", command.join(" "));
    if !env.is_empty() {
        println!("  Env: {} variables", env.len());
    }

    // Skip: unpacking, init config, init binary, ext4 rootfs, Firecracker
    // Execute command directly on the host system
    println!("Running command on host...");

    let Some(program) = command.first() else {
        return Err(crate::error::ConfigError::MissingCommand.into());
    };
    let args = command.get(1..).unwrap_or_default();

    let mut cmd = Command::new(program);
    cmd.args(args);
    cmd.stdout(std::process::Stdio::piped());
    cmd.stderr(std::process::Stdio::piped());

    // Clear host environment and set only the sanitized env vars
    cmd.env_clear();
    for (key, value) in &env {
        cmd.env(key, value);
    }

    // Run the benchmark in the temp directory for isolation
    cmd.current_dir(temp_dir.path());

    let child = cmd
        .spawn()
        .map_err(|e| crate::error::ConfigError::BinaryNotFound {
            name: program.clone(),
            hint: format!("Failed to spawn process: {e}"),
        })?;

    let output = wait_with_timeout(child, config.timeout_secs, cancel_flag)?;

    // Collect output files from the work directory if file_paths configured.
    // In non-sandboxed mode, paths are resolved relative to the temp work dir
    // to prevent reading arbitrary host files.
    let output_files = collect_output_files(config.file_paths.as_deref(), work_dir);

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

/// Collect output files, resolving paths relative to the work directory.
///
/// Absolute paths are stripped to relative (e.g., `/tmp/results.json` → `tmp/results.json`)
/// and then resolved under `work_dir`. Paths that would escape the work directory
/// via `..` segments are rejected to prevent reading arbitrary host files.
fn collect_output_files(
    file_paths: Option<&[Utf8PathBuf]>,
    work_dir: &Utf8Path,
) -> Option<HashMap<Utf8PathBuf, Vec<u8>>> {
    let paths = file_paths?;

    let mut files = HashMap::with_capacity(paths.len());
    for path in paths {
        // Strip leading `/` so absolute container paths become relative
        let relative = path.as_str().trim_start_matches('/');
        if relative.is_empty() {
            eprintln!("Warning: skipping empty output file path");
            continue;
        }
        let resolved = work_dir.join(relative);

        // Reject paths that escape the work directory via `..`
        if !resolved.as_str().starts_with(work_dir.as_str()) {
            eprintln!("Warning: rejecting output file path that escapes work directory: {path}");
            continue;
        }

        match std::fs::read(resolved.as_std_path()) {
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
