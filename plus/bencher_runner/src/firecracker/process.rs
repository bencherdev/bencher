//! Firecracker process management.

use std::process::{Child, Command};
use std::time::Duration;

use crate::firecracker::client::FirecrackerClient;
use crate::firecracker::config::{Action, ActionType};
use crate::firecracker::error::FirecrackerError;

/// A running Firecracker process.
pub struct FirecrackerProcess {
    child: Child,
    api_socket_path: String,
}

impl FirecrackerProcess {
    /// Start a new Firecracker process.
    ///
    /// Spawns `firecracker --api-sock <path> --id <id> --level Error`
    /// and waits for the API socket to become ready.
    pub fn start(
        firecracker_bin: &str,
        api_socket_path: &str,
        vm_id: &str,
    ) -> Result<Self, FirecrackerError> {
        // Remove stale socket if it exists
        let _ = std::fs::remove_file(api_socket_path);

        let child = Command::new(firecracker_bin)
            .arg("--api-sock")
            .arg(api_socket_path)
            .arg("--id")
            .arg(vm_id)
            .arg("--level")
            .arg("Error")
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| {
                FirecrackerError::ProcessStart(format!(
                    "failed to spawn {firecracker_bin}: {e}"
                ))
            })?;

        let process = Self {
            child,
            api_socket_path: api_socket_path.to_owned(),
        };

        // Wait for the API socket to become ready
        process
            .client()
            .wait_for_ready(Duration::from_secs(5))?;

        Ok(process)
    }

    /// Get a client for the Firecracker REST API.
    pub fn client(&self) -> FirecrackerClient {
        FirecrackerClient::new(&self.api_socket_path)
    }

    /// Send Ctrl+Alt+Del and wait for graceful shutdown, then SIGKILL.
    pub fn kill_after_grace_period(&mut self, grace: Duration) {
        // Try graceful shutdown via API
        let action = Action {
            action_type: ActionType::SendCtrlAltDel,
        };
        let _ = self.client().put_action(&action);

        // Wait for the process to exit gracefully
        let start = std::time::Instant::now();
        let poll_interval = Duration::from_millis(100);
        while start.elapsed() < grace {
            if let Ok(Some(_)) = self.child.try_wait() {
                return;
            }
            std::thread::sleep(poll_interval);
        }

        // Force kill if still running
        self.kill();
    }

    /// Force-kill the Firecracker process.
    pub fn kill(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    /// Clean up socket files.
    pub fn cleanup(&self) {
        let _ = std::fs::remove_file(&self.api_socket_path);
    }
}

impl Drop for FirecrackerProcess {
    fn drop(&mut self) {
        self.kill();
        self.cleanup();
    }
}
