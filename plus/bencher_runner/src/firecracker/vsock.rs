//! Host-side vsock listener for Firecracker.
//!
//! Firecracker's vsock implementation uses Unix domain sockets on the host side.
//! When the guest connects to CID 2 (host) on port N, Firecracker connects to
//! `{uds_path}_{N}`. The runner binds those sockets, from outside the chroot,
//! before VM boot; Firecracker reaches the same inodes at the chroot view of
//! the path and creates the base `uds_path` itself.
//!
//! Unix domain sockets are scoped by the filesystem, not by the network
//! namespace, so the empty namespace the VMM joins does not affect them.

use std::io::Read as _;
use std::os::fd::AsFd as _;
use std::os::unix::net::UnixListener;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use camino::Utf8Path;
use nix::poll::{PollFd, PollFlags, PollTimeout, poll};

use crate::firecracker::error::FirecrackerError;
use crate::jail::chroot::chown_to_jail;
use crate::jail::{JailFile, JailUser};

/// Poll timeout for vsock listeners (50ms).
///
/// Using `LazyLock` because `PollTimeout::try_from` is not const.
#[expect(
    clippy::expect_used,
    reason = "50ms is a valid PollTimeout; infallible in practice"
)]
static POLL_TIMEOUT: std::sync::LazyLock<PollTimeout> =
    std::sync::LazyLock::new(|| PollTimeout::try_from(50).expect("50ms fits in PollTimeout"));

/// Vsock port constants matching bencher-init.
mod ports {
    pub const STDOUT: u32 = 5000;
    pub const STDERR: u32 = 5001;
    pub const EXIT_CODE: u32 = 5002;
    pub const OUTPUT_FILES: u32 = 5005;

    /// Every port the runner listens on.
    pub const ALL: [u32; 4] = [STDOUT, STDERR, EXIT_CODE, OUTPUT_FILES];

    /// The stream a port carries, for errors that name what failed.
    ///
    /// A port number alone sends an operator to this table; the stream name is
    /// the handle they already have.
    pub const fn stream(port: u32) -> &'static str {
        match port {
            STDOUT => "stdout",
            STDERR => "stderr",
            EXIT_CODE => "exit_code",
            OUTPUT_FILES => "output_files",
            _ => "unknown",
        }
    }
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
    /// Both views of the vsock base path.
    ///
    /// Binding needs the socket view, because of the `sun_path` limit.
    /// Everything else uses the host view, which carries no dependency on a
    /// descriptor staying open.
    vsock: JailFile,
    /// Listeners for each port.
    stdout_listener: UnixListener,
    stderr_listener: UnixListener,
    exit_code_listener: UnixListener,
    output_files_listener: UnixListener,
}

impl VsockListener {
    /// Create vsock listeners for all expected ports.
    ///
    /// Creates Unix listeners at `{vsock}_{port}` for each port. Binding uses
    /// the socket view, which is the only view short enough for `sun_path`;
    /// unlinking and ownership use the host view, which does not depend on a
    /// descriptor staying open. These must be created before the VM boots.
    ///
    /// Nothing is unlinked first. Every job binds inside a chroot named by an
    /// id this runner just minted, so there is no stale socket of ours to
    /// clear, and a bind that finds one anyway is a surprise worth reporting
    /// rather than a file to delete. The removal that used to run here named
    /// the socket view, which is the one view that can go stale, so it was also
    /// the one step that contradicted the rule above.
    pub fn new(vsock: &JailFile) -> Result<Self, FirecrackerError> {
        let stdout_listener = bind_nonblocking(vsock, ports::STDOUT)?;
        let stderr_listener = bind_nonblocking(vsock, ports::STDERR)?;
        let exit_code_listener = bind_nonblocking(vsock, ports::EXIT_CODE)?;
        let output_files_listener = bind_nonblocking(vsock, ports::OUTPUT_FILES)?;

        Ok(Self {
            vsock: vsock.clone(),
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
    #[expect(
        clippy::too_many_lines,
        reason = "multi-port poll loop is clearer as one function"
    )]
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
                Err(source) => {
                    return Err(FirecrackerError::PollVsock { source });
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

    /// Hand the listener sockets to the jail uid and gid.
    ///
    /// Firecracker connects out to these sockets as the unprivileged jail
    /// user, so it needs write permission on the inodes. After `pivot_root`
    /// the only directory it traverses is `/`, which the jailer chowns itself,
    /// so the inodes are all that is left to hand over. Must run after bind
    /// and before `InstanceStart`.
    pub fn chown_to_jail(&self, jail_user: JailUser) -> Result<(), crate::error::JailError> {
        for port in ports::ALL {
            chown_to_jail(Utf8Path::new(&self.host_path(port)), jail_user)?;
        }
        Ok(())
    }

    /// Remove all socket files created by this listener.
    ///
    /// Unlinks through the host view, for the same reason as
    /// [`crate::firecracker::process::FirecrackerProcess::cleanup`]: unlinking
    /// has no `sun_path` limit, and this runs from `Drop`, where naming a
    /// descriptor that may already be closed would delete an unrelated file
    /// rather than fail.
    pub fn cleanup(&self) {
        for port in ports::ALL {
            drop(std::fs::remove_file(self.host_path(port)));
        }
    }

    /// The host path of the listener socket for a port.
    fn host_path(&self, port: u32) -> String {
        format!("{}_{port}", self.vsock.host())
    }
}

impl Drop for VsockListener {
    fn drop(&mut self) {
        self.cleanup();
    }
}

/// Bind one port's listener at the socket view and make it non-blocking.
///
/// The socket view is the only view short enough for `sun_path`; see
/// [`VsockListener::new`]. Non-blocking is set here rather than in a second
/// pass, so no listener ever exists in a state the collection loop cannot
/// poll.
fn bind_nonblocking(vsock: &JailFile, port: u32) -> Result<UnixListener, FirecrackerError> {
    let path = vsock.socket().with_port(port);
    let listener = UnixListener::bind(&path).map_err(|source| FirecrackerError::BindVsock {
        stream: ports::stream(port),
        port,
        source,
    })?;
    listener
        .set_nonblocking(true)
        .map_err(|source| FirecrackerError::VsockNonblocking {
            stream: ports::stream(port),
            port,
            source,
        })?;
    Ok(listener)
}

/// Try to accept a connection on a non-blocking listener and read all data.
///
/// Reading stops once `max_data_size` bytes have been accumulated.
#[expect(clippy::indexing_slicing, reason = "buf slice bounded by bytes read")]
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
                let remaining = max_data_size.saturating_sub(data.len());
                let to_copy = n.min(remaining);
                data.extend_from_slice(&buf[..to_copy]);
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
#[expect(
    clippy::little_endian_bytes,
    clippy::cast_possible_truncation,
    reason = "test protocol uses little-endian wire format"
)]
mod tests {
    use super::*;
    use crate::jail::JailPaths;

    use std::io::Write as _;
    use std::os::unix::net::UnixStream;

    /// 10 MiB — matches the default `max_output_size`.
    const TEST_MAX_DATA_SIZE: usize = 10 * 1024 * 1024;
    /// Short grace period for tests to avoid slowing down the test suite.
    const TEST_GRACE_PERIOD: Duration = Duration::from_millis(50);

    /// Helper: a jail whose descriptor stays open for the whole test.
    ///
    /// The socket view names an open descriptor by number, so the `JailPaths`
    /// has to outlive every path derived from it. Dropping it early leaves the
    /// paths addressing whatever the kernel hands that number to next, which
    /// is exactly the failure this binding prevents.
    fn jail_in_tmpdir() -> (tempfile::TempDir, JailPaths) {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(dir.path()).unwrap();
        let jail = JailPaths::new(root).unwrap();
        (dir, jail)
    }

    /// Helper: create a `VsockListener` on a jail that stays alive.
    fn listener_in_tmpdir() -> (tempfile::TempDir, JailPaths, VsockListener) {
        let (dir, jail) = jail_in_tmpdir();
        let listener = VsockListener::new(jail.vsock()).unwrap();
        (dir, jail, listener)
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
        let (_dir, jail, _listener) = listener_in_tmpdir();
        let base = jail.vsock().host().to_string();

        for port in [5000, 5001, 5002, 5005] {
            let path = format!("{base}_{port}");
            assert!(
                std::path::Path::new(&path).exists(),
                "socket file for port {port} should exist"
            );
        }
    }

    #[test]
    fn a_path_already_taken_is_reported_not_deleted() {
        // The failure this prevents: `new` used to unlink each socket path
        // before binding it, and it did so through the socket view, which names
        // an open descriptor by number. Every other unlink in this crate takes
        // the host view precisely because a stale number deletes whatever
        // inherited it, and that deletion looks like success. Nothing at these
        // paths is ever ours to remove anyway: the chroot is named by a freshly
        // minted id.
        let (_dir, jail) = jail_in_tmpdir();
        let occupied = format!("{}_{}", jail.vsock().host(), ports::STDOUT);
        std::fs::write(&occupied, b"not ours").unwrap();

        let Err(err) = VsockListener::new(jail.vsock()) else {
            panic!("binding over a path that is already taken must fail");
        };

        assert!(
            matches!(
                err,
                FirecrackerError::BindVsock {
                    stream: "stdout",
                    ..
                }
            ),
            "a taken path is a bind failure that names the stream, got: {err}"
        );
        assert_eq!(
            std::fs::read(&occupied).unwrap(),
            b"not ours",
            "the file that was there must still be there"
        );
    }

    #[test]
    fn vsock_listener_cleanup_removes_files() {
        let (_dir, jail) = jail_in_tmpdir();
        let base = jail.vsock().host().to_string();

        {
            let _listener = VsockListener::new(jail.vsock()).unwrap();
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
        let (_dir, jail, listener) = listener_in_tmpdir();
        let base = jail.vsock().host().to_string();

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
        let (_dir, jail, listener) = listener_in_tmpdir();
        let base = jail.vsock().host().to_string();

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
        let (_dir, _jail, listener) = listener_in_tmpdir();

        // No data sent — should timeout with an error
        let result = listener.collect_results(
            Duration::from_millis(200),
            TEST_MAX_DATA_SIZE,
            None,
            TEST_GRACE_PERIOD,
        );

        let err = result.unwrap_err();
        assert!(
            err.to_string().contains("timed out"),
            "error should mention timeout, got: {err}"
        );
    }

    #[test]
    fn collect_non_utf8_stdout() {
        let (_dir, jail, listener) = listener_in_tmpdir();
        let base = jail.vsock().host().to_string();

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
        let (_dir, jail, listener) = listener_in_tmpdir();
        let base = jail.vsock().host().to_string();

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
    fn try_accept_and_read_truncates_at_max_data_size() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.sock");
        let listener = UnixListener::bind(&path).unwrap();
        listener.set_nonblocking(true).unwrap();

        let max_size = 100;
        // Send 5x more than max_size
        let mut stream = UnixStream::connect(&path).unwrap();
        stream.write_all(&vec![0xAB; 500]).unwrap();
        drop(stream);

        std::thread::sleep(Duration::from_millis(10));

        let data = try_accept_and_read(&listener, max_size).unwrap();
        assert_eq!(
            data.len(),
            max_size,
            "data should be truncated to exactly max_data_size"
        );
        assert!(
            data.iter().all(|&b| b == 0xAB),
            "truncated data should contain the correct bytes"
        );
    }

    #[test]
    fn collect_cancelled_returns_error() {
        let (_dir, _jail, listener) = listener_in_tmpdir();

        // Set the cancel flag before collecting
        let cancel_flag = Arc::new(AtomicBool::new(true));

        let result = listener.collect_results(
            Duration::from_secs(5),
            TEST_MAX_DATA_SIZE,
            Some(&cancel_flag),
            Duration::from_secs(1),
        );

        let err = result.unwrap_err();
        assert!(
            matches!(err, FirecrackerError::Cancelled),
            "error should be Cancelled, got: {err}"
        );
    }
}
