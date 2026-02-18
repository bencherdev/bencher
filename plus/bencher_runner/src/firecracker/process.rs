//! Firecracker process management.
#![expect(clippy::print_stderr)]

use std::process::{Child, Command};
use std::time::Duration;

use crate::firecracker::client::FirecrackerClient;
use crate::firecracker::config::{Action, ActionType};
use crate::firecracker::error::FirecrackerError;

/// A running Firecracker process.
pub struct FirecrackerProcess {
    child: Child,
    api_socket_path: String,
    stderr_thread: Option<std::thread::JoinHandle<()>>,
}

impl FirecrackerProcess {
    /// Start a new Firecracker process.
    ///
    /// Spawns `firecracker --api-sock <path> --id <id> --level <level>`
    /// and waits for the API socket to become ready.
    /// A background thread reads stderr and prints lines prefixed with `[firecracker]`.
    pub fn start(
        firecracker_bin: &str,
        api_socket_path: &str,
        vm_id: &str,
        log_level: &str,
        housekeeping_cores: Vec<usize>,
    ) -> Result<Self, FirecrackerError> {
        // Remove stale socket if it exists
        drop(std::fs::remove_file(api_socket_path));

        let mut child = Command::new(firecracker_bin)
            .arg("--api-sock")
            .arg(api_socket_path)
            .arg("--id")
            .arg(vm_id)
            .arg("--level")
            .arg(log_level)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                FirecrackerError::ProcessStart(format!("failed to spawn {firecracker_bin}: {e}"))
            })?;

        // Spawn a thread to read stderr line-by-line
        let stderr = child.stderr.take().ok_or_else(|| {
            FirecrackerError::ProcessStart("stderr was piped but not available".into())
        })?;
        let stderr_thread = std::thread::spawn(move || {
            use std::io::BufRead as _;

            // Pin to housekeeping cores to avoid benchmark interference
            if let Err(e) = crate::cpu::pin_current_thread(&housekeeping_cores) {
                eprintln!("Warning: failed to pin stderr reader thread: {e}");
            }
            let reader = std::io::BufReader::new(stderr);
            for line in reader.lines() {
                match line {
                    Ok(line) => eprintln!("[firecracker] {line}"),
                    Err(_) => break,
                }
            }
        });

        let process = Self {
            child,
            api_socket_path: api_socket_path.to_owned(),
            stderr_thread: Some(stderr_thread),
        };

        // Wait for the API socket to become ready
        process.client().wait_for_ready(Duration::from_secs(5))?;

        Ok(process)
    }

    /// Get a client for the Firecracker REST API.
    pub fn client(&self) -> FirecrackerClient {
        FirecrackerClient::new(&self.api_socket_path)
    }

    /// Get the PID of the Firecracker process.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Send Ctrl+Alt+Del and wait for graceful shutdown, then SIGKILL.
    pub fn kill_after_grace_period(&mut self, grace: Duration) {
        // Try graceful shutdown via API
        let action = Action {
            action_type: ActionType::SendCtrlAltDel,
        };
        drop(self.client().put_action(&action));

        // Wait for the process to exit gracefully
        let start = std::time::Instant::now();
        let poll_interval = Duration::from_millis(100);
        while start.elapsed() < grace {
            if let Ok(Some(_)) = self.child.try_wait() {
                self.join_stderr_thread();
                return;
            }
            std::thread::sleep(poll_interval);
        }

        // Force kill if still running
        self.kill();
    }

    /// Force-kill the Firecracker process.
    pub fn kill(&mut self) {
        drop(self.child.kill());
        drop(self.child.wait());
        self.join_stderr_thread();
    }

    /// Clean up socket files.
    pub fn cleanup(&self) {
        drop(std::fs::remove_file(&self.api_socket_path));
    }

    /// Join the stderr reader thread if it exists.
    fn join_stderr_thread(&mut self) {
        if let Some(handle) = self.stderr_thread.take() {
            drop(handle.join());
        }
    }
}

impl Drop for FirecrackerProcess {
    fn drop(&mut self) {
        self.kill();
        self.cleanup();
    }
}
