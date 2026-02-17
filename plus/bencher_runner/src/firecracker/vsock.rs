//! Host-side vsock listener for Firecracker.
//!
//! Firecracker's vsock implementation uses Unix domain sockets on the host side.
//! When the guest connects to CID 2 (host) on port N, Firecracker connects to
//! `{uds_path}_{N}` on the host. The host must have Unix listeners at those
//! paths before VM boot.

use std::io::Read as _;
use std::os::fd::AsFd as _;
use std::os::unix::net::UnixListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use nix::poll::{PollFd, PollFlags, PollTimeout, poll};

use crate::firecracker::error::FirecrackerError;

/// Poll timeout for vsock listeners (50ms).
///
/// Using `LazyLock` because `PollTimeout::try_from` is not const.
#[expect(clippy::expect_used)]
static POLL_TIMEOUT: std::sync::LazyLock<PollTimeout> =
    std::sync::LazyLock::new(|| PollTimeout::try_from(50).expect("50ms fits in PollTimeout"));

/// Vsock port constants matching bencher-init.
mod ports {
    pub const STDOUT: u32 = 5000;
    pub const STDERR: u32 = 5001;
    pub const EXIT_CODE: u32 = 5002;
    pub const OUTPUT_FILES: u32 = 5005;
}

/// Results collected from the guest via vsock.
#[derive(Debug)]
pub struct VsockResults {
    /// Stdout output from the benchmark.
    pub stdout: String,
    /// Stderr output from the benchmark.
    pub stderr: String,
    /// Exit code as a string.
    pub exit_code: String,
    /// Optional output files (length-prefixed binary protocol).
    pub output_files: Option<Vec<u8>>,
}

/// Host-side vsock listener that accepts connections from Firecracker.
pub struct VsockListener {
    /// Base path for the vsock UDS.
    vsock_uds_path: String,
    /// Listeners for each port.
    stdout_listener: UnixListener,
    stderr_listener: UnixListener,
    exit_code_listener: UnixListener,
    output_files_listener: UnixListener,
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
        let output_files_path = format!("{vsock_uds_path}_{}", ports::OUTPUT_FILES);

        // Remove stale socket files
        for path in [
            &stdout_path,
            &stderr_path,
            &exit_code_path,
            &output_files_path,
        ] {
            drop(std::fs::remove_file(path));
        }

        let stdout_listener = UnixListener::bind(&stdout_path)
            .map_err(|e| FirecrackerError::VsockCollection(format!("bind stdout: {e}")))?;
        let stderr_listener = UnixListener::bind(&stderr_path)
            .map_err(|e| FirecrackerError::VsockCollection(format!("bind stderr: {e}")))?;
        let exit_code_listener = UnixListener::bind(&exit_code_path)
            .map_err(|e| FirecrackerError::VsockCollection(format!("bind exit_code: {e}")))?;
        let output_files_listener = UnixListener::bind(&output_files_path)
            .map_err(|e| FirecrackerError::VsockCollection(format!("bind output_files: {e}")))?;

        // Set non-blocking so we can poll
        drop(stdout_listener.set_nonblocking(true));
        drop(stderr_listener.set_nonblocking(true));
        drop(exit_code_listener.set_nonblocking(true));
        drop(output_files_listener.set_nonblocking(true));

        Ok(Self {
            vsock_uds_path: vsock_uds_path.to_owned(),
            stdout_listener,
            stderr_listener,
            exit_code_listener,
            output_files_listener,
        })
    }

    /// Collect results from the guest via vsock connections.
    ///
    /// Waits up to `timeout` for the guest to send results on all ports.
    /// The exit code port is mandatory; stdout, stderr, and output file are optional.
    ///
    /// `max_data_size` limits how many bytes are read per port, matching the
    /// guest-side `max_output_size` so both sides enforce the same cap.
    ///
    /// If `cancel_flag` is provided and set to `true`, collection stops early
    /// and returns a cancellation error.
    #[expect(clippy::too_many_lines)]
    pub fn collect_results(
        &self,
        timeout: Duration,
        max_data_size: usize,
        cancel_flag: Option<&Arc<AtomicBool>>,
        grace_period: Duration,
    ) -> Result<VsockResults, FirecrackerError> {
        let start = std::time::Instant::now();
        let poll_timeout = *POLL_TIMEOUT;

        let mut stdout_data: Option<Vec<u8>> = None;
        let mut stderr_data: Option<Vec<u8>> = None;
        let mut exit_code_data: Option<Vec<u8>> = None;
        let mut output_files_data: Option<Vec<u8>> = None;

        // Poll until we have the exit code (required) or timeout
        while start.elapsed() < timeout {
            // Check for cancellation
            if let Some(flag) = cancel_flag
                && flag.load(Ordering::SeqCst)
            {
                return Err(FirecrackerError::Cancelled);
            }

            // Build poll fds for listeners we still need data from.
            // Use empty flags for already-collected ports so the kernel skips them.
            let mut fds = [
                PollFd::new(
                    self.stdout_listener.as_fd(),
                    if stdout_data.is_none() {
                        PollFlags::POLLIN
                    } else {
                        PollFlags::empty()
                    },
                ),
                PollFd::new(
                    self.stderr_listener.as_fd(),
                    if stderr_data.is_none() {
                        PollFlags::POLLIN
                    } else {
                        PollFlags::empty()
                    },
                ),
                PollFd::new(
                    self.exit_code_listener.as_fd(),
                    if exit_code_data.is_none() {
                        PollFlags::POLLIN
                    } else {
                        PollFlags::empty()
                    },
                ),
                PollFd::new(
                    self.output_files_listener.as_fd(),
                    if output_files_data.is_none() {
                        PollFlags::POLLIN
                    } else {
                        PollFlags::empty()
                    },
                ),
            ];

            match poll(&mut fds, poll_timeout) {
                Ok(_) => {},
                Err(nix::errno::Errno::EINTR) => continue,
                Err(e) => {
                    return Err(FirecrackerError::VsockCollection(format!("poll: {e}")));
                },
            }

            // Try to accept and read from each listener that has activity
            if stdout_data.is_none()
                && fds[0]
                    .revents()
                    .is_some_and(|r| r.intersects(PollFlags::POLLIN))
            {
                stdout_data = try_accept_and_read(&self.stdout_listener, max_data_size);
            }
            if stderr_data.is_none()
                && fds[1]
                    .revents()
                    .is_some_and(|r| r.intersects(PollFlags::POLLIN))
            {
                stderr_data = try_accept_and_read(&self.stderr_listener, max_data_size);
            }
            if exit_code_data.is_none()
                && fds[2]
                    .revents()
                    .is_some_and(|r| r.intersects(PollFlags::POLLIN))
            {
                exit_code_data = try_accept_and_read(&self.exit_code_listener, max_data_size);
            }
            if output_files_data.is_none()
                && fds[3]
                    .revents()
                    .is_some_and(|r| r.intersects(PollFlags::POLLIN))
            {
                output_files_data = try_accept_and_read(&self.output_files_listener, max_data_size);
            }

            // Exit code is the signal that results are complete
            if exit_code_data.is_some() {
                // Give a brief window for remaining data to arrive.
                // The grace period balances latency vs reliability for stdout/stderr
                // that may still be in flight when the exit code lands.
                std::thread::sleep(grace_period);
                // Final collection pass
                if stdout_data.is_none() {
                    stdout_data = try_accept_and_read(&self.stdout_listener, max_data_size);
                }
                if stderr_data.is_none() {
                    stderr_data = try_accept_and_read(&self.stderr_listener, max_data_size);
                }
                if output_files_data.is_none() {
                    output_files_data =
                        try_accept_and_read(&self.output_files_listener, max_data_size);
                }
                break;
            }
        }

        let exit_code = String::from_utf8_lossy(&exit_code_data.unwrap_or_default())
            .trim()
            .to_owned();

        if exit_code.is_empty() {
            return Err(FirecrackerError::Timeout(format!(
                "VM execution timed out after {timeout:?}"
            )));
        }

        Ok(VsockResults {
            stdout: String::from_utf8_lossy(&stdout_data.unwrap_or_default()).into_owned(),
            stderr: String::from_utf8_lossy(&stderr_data.unwrap_or_default()).into_owned(),
            exit_code,
            output_files: output_files_data,
        })
    }

    /// Remove all socket files created by this listener.
    pub fn cleanup(&self) {
        for port in [
            ports::STDOUT,
            ports::STDERR,
            ports::EXIT_CODE,
            ports::OUTPUT_FILES,
        ] {
            let path = format!("{}_{port}", self.vsock_uds_path);
            drop(std::fs::remove_file(path));
        }
    }
}

impl Drop for VsockListener {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// Try to accept a connection on a non-blocking listener and read all data.
///
/// Reading stops once `max_data_size` bytes have been accumulated.
#[expect(clippy::indexing_slicing)]
fn try_accept_and_read(listener: &UnixListener, max_data_size: usize) -> Option<Vec<u8>> {
    let (mut stream, _) = listener.accept().ok()?;

    // Set blocking with a read timeout for the data stream
    drop(stream.set_nonblocking(false));
    drop(stream.set_read_timeout(Some(Duration::from_secs(5))));

    let mut data = Vec::new();
    let mut buf = [0u8; 8192];
    loop {
        match stream.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                data.extend_from_slice(&buf[..n]);
                if data.len() >= max_data_size {
                    break;
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => break,
            Err(e) if e.kind() == std::io::ErrorKind::TimedOut => break,
            Err(_) => break,
        }
    }

    Some(data)
}

#[cfg(test)]
#[expect(clippy::little_endian_bytes, clippy::cast_possible_truncation)]
mod tests {
    use super::*;

    use std::io::Write as _;
    use std::os::unix::net::UnixStream;

    /// 10 MiB — matches the default `max_output_size`.
    const TEST_MAX_DATA_SIZE: usize = 10 * 1024 * 1024;
    /// Short grace period for tests to avoid slowing down the test suite.
    const TEST_GRACE_PERIOD: Duration = Duration::from_millis(50);

    /// Helper: create a `VsockListener` in a temp directory.
    fn listener_in_tmpdir() -> (tempfile::TempDir, VsockListener) {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("vsock").to_str().unwrap().to_owned();
        let listener = VsockListener::new(&base).unwrap();
        (dir, listener)
    }

    /// Helper: connect to a vsock port and write data.
    fn send_to_port(base: &str, port: u32, data: &[u8]) {
        let path = format!("{base}_{port}");
        let mut stream = UnixStream::connect(path).unwrap();
        stream.write_all(data).unwrap();
        // drop closes the connection, signaling EOF
    }

    #[test]
    fn vsock_listener_creates_socket_files() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("vsock").to_str().unwrap().to_owned();
        let _listener = VsockListener::new(&base).unwrap();

        for port in [5000, 5001, 5002, 5005] {
            let path = format!("{base}_{port}");
            assert!(
                std::path::Path::new(&path).exists(),
                "socket file for port {port} should exist"
            );
        }
    }

    #[test]
    fn vsock_listener_cleanup_removes_files() {
        let dir = tempfile::tempdir().unwrap();
        let base = dir.path().join("vsock").to_str().unwrap().to_owned();

        {
            let _listener = VsockListener::new(&base).unwrap();
            // listener drops here
        }

        for port in [5000, 5001, 5002, 5005] {
            let path = format!("{base}_{port}");
            assert!(
                !std::path::Path::new(&path).exists(),
                "socket file for port {port} should be cleaned up"
            );
        }
    }

    #[test]
    fn collect_all_ports() {
        let (dir, listener) = listener_in_tmpdir();
        let base = dir.path().join("vsock").to_str().unwrap().to_owned();

        // Build protocol-encoded data: 1 file, path="out.bin", content=\x00\x01\x02
        let mut encoded = Vec::new();
        encoded.extend_from_slice(&1u32.to_le_bytes()); // file_count
        let path = b"out.bin";
        encoded.extend_from_slice(&(path.len() as u32).to_le_bytes());
        encoded.extend_from_slice(path);
        let content = b"\x00\x01\x02";
        encoded.extend_from_slice(&(content.len() as u64).to_le_bytes());
        encoded.extend_from_slice(content);

        // Send data on all ports from a separate thread
        let base_clone = base.clone();
        let sender = std::thread::spawn(move || {
            // Small delay to let collect_results start polling
            std::thread::sleep(Duration::from_millis(50));
            send_to_port(&base_clone, ports::STDOUT, b"benchmark output");
            send_to_port(&base_clone, ports::STDERR, b"some warnings");
            send_to_port(&base_clone, ports::OUTPUT_FILES, &encoded);
            send_to_port(&base_clone, ports::EXIT_CODE, b"0");
        });

        let results = listener
            .collect_results(
                Duration::from_secs(5),
                TEST_MAX_DATA_SIZE,
                None,
                TEST_GRACE_PERIOD,
            )
            .unwrap();
        sender.join().unwrap();

        assert_eq!(results.stdout, "benchmark output");
        assert_eq!(results.stderr, "some warnings");
        assert_eq!(results.exit_code, "0");
        assert!(results.output_files.is_some());
    }

    #[test]
    fn collect_exit_code_only() {
        let (dir, listener) = listener_in_tmpdir();
        let base = dir.path().join("vsock").to_str().unwrap().to_owned();

        let base_clone = base.clone();
        let sender = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            send_to_port(&base_clone, ports::EXIT_CODE, b"1");
        });

        let results = listener
            .collect_results(
                Duration::from_secs(5),
                TEST_MAX_DATA_SIZE,
                None,
                TEST_GRACE_PERIOD,
            )
            .unwrap();
        sender.join().unwrap();

        assert_eq!(results.exit_code, "1");
        assert_eq!(results.stdout, "");
        assert_eq!(results.stderr, "");
        assert_eq!(results.output_files, None);
    }

    #[test]
    fn collect_timeout_returns_error() {
        let (_dir, listener) = listener_in_tmpdir();

        // No data sent — should timeout with an error
        let result = listener.collect_results(
            Duration::from_millis(200),
            TEST_MAX_DATA_SIZE,
            None,
            TEST_GRACE_PERIOD,
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("timed out"),
            "error should mention timeout, got: {err}"
        );
    }

    #[test]
    fn collect_non_utf8_stdout() {
        let (dir, listener) = listener_in_tmpdir();
        let base = dir.path().join("vsock").to_str().unwrap().to_owned();

        let base_clone = base.clone();
        let sender = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            // Invalid UTF-8 bytes
            send_to_port(&base_clone, ports::STDOUT, b"hello \xff\xfe world");
            send_to_port(&base_clone, ports::EXIT_CODE, b"0");
        });

        let results = listener
            .collect_results(
                Duration::from_secs(5),
                TEST_MAX_DATA_SIZE,
                None,
                TEST_GRACE_PERIOD,
            )
            .unwrap();
        sender.join().unwrap();

        // Should use lossy conversion, not panic
        assert!(results.stdout.contains("hello"));
        assert!(results.stdout.contains("world"));
        assert_eq!(results.exit_code, "0");
    }

    #[test]
    fn collect_exit_code_triggers_final_pass() {
        let (dir, listener) = listener_in_tmpdir();
        let base = dir.path().join("vsock").to_str().unwrap().to_owned();

        let base_clone = base.clone();
        let sender = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(50));
            // Send exit code first
            send_to_port(&base_clone, ports::EXIT_CODE, b"0");
            // Then stdout arrives during the grace window
            std::thread::sleep(Duration::from_millis(20));
            send_to_port(&base_clone, ports::STDOUT, b"late output");
        });

        let results = listener
            .collect_results(
                Duration::from_secs(5),
                TEST_MAX_DATA_SIZE,
                None,
                TEST_GRACE_PERIOD,
            )
            .unwrap();
        sender.join().unwrap();

        assert_eq!(results.exit_code, "0");
        assert_eq!(results.stdout, "late output");
    }

    #[test]
    fn try_accept_and_read_no_connection() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.sock");
        let listener = UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();

        // No connection pending
        assert!(try_accept_and_read(&listener, TEST_MAX_DATA_SIZE).is_none());
    }

    #[test]
    fn try_accept_and_read_with_data() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.sock");
        let listener = UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();

        // Connect and send data
        let mut stream = UnixStream::connect(&path).unwrap();
        stream.write_all(b"hello").unwrap();
        drop(stream); // close to send EOF

        // Brief delay to ensure the connection is ready
        std::thread::sleep(Duration::from_millis(10));

        let data = try_accept_and_read(&listener, TEST_MAX_DATA_SIZE).unwrap();
        assert_eq!(data, b"hello");
    }

    #[test]
    fn try_accept_and_read_empty_connection() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.sock");
        let listener = UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();

        // Connect but send nothing
        let stream = UnixStream::connect(&path).unwrap();
        drop(stream); // close immediately

        std::thread::sleep(Duration::from_millis(10));

        let data = try_accept_and_read(&listener, TEST_MAX_DATA_SIZE).unwrap();
        assert!(data.is_empty());
    }

    #[test]
    fn collect_cancelled_returns_error() {
        let (_dir, listener) = listener_in_tmpdir();

        // Set the cancel flag before collecting
        let cancel_flag = Arc::new(AtomicBool::new(true));

        let result = listener.collect_results(
            Duration::from_secs(5),
            TEST_MAX_DATA_SIZE,
            Some(&cancel_flag),
            Duration::from_secs(1),
        );

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(
            matches!(err, FirecrackerError::Cancelled),
            "error should be Cancelled, got: {err}"
        );
    }
}
