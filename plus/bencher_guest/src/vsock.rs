//! Vsock communication for guest-host interaction.
//!
//! This module provides helpers for connecting to the host via vsock
//! and sending/receiving benchmark data.
//!
//! On Linux, this uses real vsock for VM-host communication.
//! On other platforms (macOS), it falls back to Unix sockets for development.

use std::io::{self, BufRead as _, BufReader};

use thiserror::Error;

use crate::protocol::{BenchmarkParams, BenchmarkResults};

/// The host's CID (Context ID) for vsock.
/// CID 2 is always the host in the vsock address space.
pub const HOST_CID: u32 = 2;

/// Maximum size for incoming parameters (1 MiB).
const MAX_RECEIVE_SIZE: u64 = 1024 * 1024;

/// Maximum size for outgoing results (10 MiB, accommodates stdout/stderr).
const MAX_SEND_SIZE: usize = 10 * 1024 * 1024;

/// Default port for Bencher communication.
pub const DEFAULT_PORT: u32 = 5000;

/// Errors that can occur during vsock communication.
#[derive(Debug, Error)]
pub enum VsockError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Connection failed: {0}")]
    Connection(String),

    #[error("Protocol error: {0}")]
    Protocol(String),
}

/// A connection to the host via vsock.
pub struct VsockConnection {
    inner: ConnectionInner,
}

// Linux implementation using the vsock crate
#[cfg(target_os = "linux")]
type ConnectionInner = vsock::VsockStream;

// Non-Linux fallback using Unix sockets
#[cfg(not(target_os = "linux"))]
use std::os::unix::net::UnixStream;

#[cfg(not(target_os = "linux"))]
type ConnectionInner = UnixStream;

impl VsockConnection {
    /// Connect to the host via vsock.
    #[cfg(target_os = "linux")]
    pub fn connect(cid: u32, port: u32) -> Result<Self, VsockError> {
        let stream = vsock::VsockStream::connect_with_cid_port(cid, port).map_err(|e| {
            VsockError::Connection(format!(
                "Failed to connect to vsock CID {cid} port {port}: {e}"
            ))
        })?;
        Ok(Self { inner: stream })
    }

    /// Connect to Unix socket fallback for non-Linux development.
    #[cfg(not(target_os = "linux"))]
    pub fn connect(_cid: u32, port: u32) -> Result<Self, VsockError> {
        // On non-Linux, fall back to Unix sockets for development/testing.
        let socket_path = format!("/tmp/bencher-vsock-{port}.sock");

        let stream = UnixStream::connect(&socket_path).map_err(|e| {
            VsockError::Connection(format!(
                "Failed to connect to Unix socket {socket_path}: {e} \
                 (Note: real vsock is only available on Linux)"
            ))
        })?;
        Ok(Self { inner: stream })
    }

    /// Send benchmark results to the host.
    ///
    /// Rejects payloads exceeding [`MAX_SEND_SIZE`] bytes to prevent runaway output.
    pub fn send_results(&mut self, results: &BenchmarkResults) -> Result<(), VsockError> {
        use std::io::Write as _;

        let json = serde_json::to_vec(results)?;
        if json.len() > MAX_SEND_SIZE {
            return Err(VsockError::Protocol(format!(
                "Results size {} exceeds {} byte limit",
                json.len(),
                MAX_SEND_SIZE
            )));
        }
        self.inner.write_all(&json)?;
        self.inner.write_all(b"\n")?;
        self.inner.flush()?;
        Ok(())
    }

    /// Receive benchmark parameters from the host.
    ///
    /// Limits input to [`MAX_RECEIVE_SIZE`] bytes to prevent memory exhaustion.
    pub fn receive_params(&mut self) -> Result<BenchmarkParams, VsockError> {
        use std::io::Read as _;

        let limited = (&self.inner).take(MAX_RECEIVE_SIZE);
        let mut reader = BufReader::new(limited);
        let mut line = String::new();
        let bytes_read = reader.read_line(&mut line)?;
        if bytes_read == 0 {
            return Err(VsockError::Protocol("Empty params received".to_owned()));
        }
        if !line.ends_with('\n') {
            return Err(VsockError::Protocol(format!(
                "Params exceeded {MAX_RECEIVE_SIZE} byte limit"
            )));
        }
        let params = serde_json::from_str(&line)?;
        Ok(params)
    }

    /// Read raw data from the connection.
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, VsockError> {
        use std::io::Read as _;
        Ok(self.inner.read(buf)?)
    }

    /// Write raw data to the connection.
    pub fn write(&mut self, buf: &[u8]) -> Result<usize, VsockError> {
        use std::io::Write as _;
        Ok(self.inner.write(buf)?)
    }

    /// Flush the connection.
    pub fn flush(&mut self) -> Result<(), VsockError> {
        use std::io::Write as _;
        Ok(self.inner.flush()?)
    }
}

/// Connect to the host via vsock.
///
/// On Linux, this uses real vsock to connect to the host.
/// On other platforms, this falls back to Unix sockets for development.
///
/// # Returns
///
/// A `VsockConnection` for communicating with the host.
pub fn connect_to_host() -> Result<VsockConnection, VsockError> {
    connect_to_host_port(DEFAULT_PORT)
}

/// Connect to the host via vsock on a specific port.
pub fn connect_to_host_port(port: u32) -> Result<VsockConnection, VsockError> {
    VsockConnection::connect(HOST_CID, port)
}

/// Send results to the host using the default connection.
///
/// This is a convenience function that:
/// 1. Connects to the host
/// 2. Sends the results
/// 3. Closes the connection
pub fn send_results(results: &BenchmarkResults) -> Result<(), VsockError> {
    let mut conn = connect_to_host()?;
    conn.send_results(results)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{HOST_CID, VsockError};

    #[test]
    fn test_vsock_error_display() {
        let err = VsockError::Connection("test error".to_owned());
        assert!(err.to_string().contains("test error"));
    }

    #[test]
    fn test_host_cid() {
        // Host CID is always 2 in vsock
        assert_eq!(HOST_CID, 2);
    }
}
