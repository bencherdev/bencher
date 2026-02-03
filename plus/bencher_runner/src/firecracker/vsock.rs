//! Host-side vsock listener for Firecracker.
//!
//! Firecracker's vsock implementation uses Unix domain sockets on the host side.
//! When the guest connects to CID 2 (host) on port N, Firecracker connects to
//! `{uds_path}_{N}` on the host. The host must have Unix listeners at those
//! paths before VM boot.

use std::io::Read;
use std::os::unix::net::UnixListener;
use std::time::Duration;

use crate::firecracker::error::FirecrackerError;

/// Vsock port constants matching bencher-init.
mod ports {
    pub const STDOUT: u32 = 5000;
    pub const STDERR: u32 = 5001;
    pub const EXIT_CODE: u32 = 5002;
    pub const OUTPUT_FILE: u32 = 5005;
}

/// Maximum data size per port (10 MiB).
const MAX_DATA_SIZE: usize = 10 * 1024 * 1024;

/// Results collected from the guest via vsock.
#[derive(Debug)]
pub struct VsockResults {
    /// Stdout output from the benchmark.
    pub stdout: String,
    /// Stderr output from the benchmark.
    pub stderr: String,
    /// Exit code as a string.
    pub exit_code: String,
    /// Optional output file contents.
    pub output_file: Option<Vec<u8>>,
}

/// Host-side vsock listener that accepts connections from Firecracker.
pub struct VsockListener {
    /// Base path for the vsock UDS.
    vsock_uds_path: String,
    /// Listeners for each port.
    stdout_listener: UnixListener,
    stderr_listener: UnixListener,
    exit_code_listener: UnixListener,
    output_file_listener: UnixListener,
}

impl VsockListener {
    /// Create vsock listeners for all expected ports.
    ///
    /// Creates Unix listeners at `{vsock_uds_path}_{port}` for each port.
    /// These must be created before the VM boots.
    pub fn new(vsock_uds_path: &str) -> Result<Self, FirecrackerError> {
        let stdout_path = format!("{vsock_uds_path}_{}", ports::STDOUT);
        let stderr_path = format!("{vsock_uds_path}_{}", ports::STDERR);
        let exit_code_path = format!("{vsock_uds_path}_{}", ports::EXIT_CODE);
        let output_file_path = format!("{vsock_uds_path}_{}", ports::OUTPUT_FILE);

        // Remove stale socket files
        for path in [&stdout_path, &stderr_path, &exit_code_path, &output_file_path] {
            let _ = std::fs::remove_file(path);
        }

        let stdout_listener = UnixListener::bind(&stdout_path)
            .map_err(|e| FirecrackerError::VsockCollection(format!("bind stdout: {e}")))?;
        let stderr_listener = UnixListener::bind(&stderr_path)
            .map_err(|e| FirecrackerError::VsockCollection(format!("bind stderr: {e}")))?;
        let exit_code_listener = UnixListener::bind(&exit_code_path)
            .map_err(|e| FirecrackerError::VsockCollection(format!("bind exit_code: {e}")))?;
        let output_file_listener = UnixListener::bind(&output_file_path)
            .map_err(|e| FirecrackerError::VsockCollection(format!("bind output_file: {e}")))?;

        // Set non-blocking so we can poll
        stdout_listener.set_nonblocking(true).ok();
        stderr_listener.set_nonblocking(true).ok();
        exit_code_listener.set_nonblocking(true).ok();
        output_file_listener.set_nonblocking(true).ok();

        Ok(Self {
            vsock_uds_path: vsock_uds_path.to_owned(),
            stdout_listener,
            stderr_listener,
            exit_code_listener,
            output_file_listener,
        })
    }

    /// Collect results from the guest via vsock connections.
    ///
    /// Waits up to `timeout` for the guest to send results on all ports.
    /// The exit code port is mandatory; stdout, stderr, and output file are optional.
    pub fn collect_results(&self, timeout: Duration) -> Result<VsockResults, FirecrackerError> {
        let start = std::time::Instant::now();
        let poll_interval = Duration::from_millis(50);

        let mut stdout_data: Option<Vec<u8>> = None;
        let mut stderr_data: Option<Vec<u8>> = None;
        let mut exit_code_data: Option<Vec<u8>> = None;
        let mut output_file_data: Option<Vec<u8>> = None;

        // Poll until we have the exit code (required) or timeout
        while start.elapsed() < timeout {
            // Try to accept and read from each listener
            if stdout_data.is_none() {
                stdout_data = try_accept_and_read(&self.stdout_listener);
            }
            if stderr_data.is_none() {
                stderr_data = try_accept_and_read(&self.stderr_listener);
            }
            if exit_code_data.is_none() {
                exit_code_data = try_accept_and_read(&self.exit_code_listener);
            }
            if output_file_data.is_none() {
                output_file_data = try_accept_and_read(&self.output_file_listener);
            }

            // Exit code is the signal that results are complete
            if exit_code_data.is_some() {
                // Give a brief window for remaining data
                std::thread::sleep(Duration::from_millis(100));
                // Final collection pass
                if stdout_data.is_none() {
                    stdout_data = try_accept_and_read(&self.stdout_listener);
                }
                if stderr_data.is_none() {
                    stderr_data = try_accept_and_read(&self.stderr_listener);
                }
                if output_file_data.is_none() {
                    output_file_data = try_accept_and_read(&self.output_file_listener);
                }
                break;
            }

            std::thread::sleep(poll_interval);
        }

        Ok(VsockResults {
            stdout: String::from_utf8_lossy(&stdout_data.unwrap_or_default()).into_owned(),
            stderr: String::from_utf8_lossy(&stderr_data.unwrap_or_default()).into_owned(),
            exit_code: String::from_utf8_lossy(&exit_code_data.unwrap_or_default())
                .trim()
                .to_owned(),
            output_file: output_file_data,
        })
    }

    /// Remove all socket files created by this listener.
    pub fn cleanup(&self) {
        for port in [ports::STDOUT, ports::STDERR, ports::EXIT_CODE, ports::OUTPUT_FILE] {
            let path = format!("{}_{port}", self.vsock_uds_path);
            let _ = std::fs::remove_file(path);
        }
    }
}

impl Drop for VsockListener {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// Try to accept a connection on a non-blocking listener and read all data.
fn try_accept_and_read(listener: &UnixListener) -> Option<Vec<u8>> {
    let (mut stream, _) = listener.accept().ok()?;

    // Set blocking with a read timeout for the data stream
    stream.set_nonblocking(false).ok();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .ok();

    let mut data = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                data.extend_from_slice(&buf[..n]);
                if data.len() >= MAX_DATA_SIZE {
                    break;
                }
            }
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(_) => break,
        }
    }

    Some(data)
}
