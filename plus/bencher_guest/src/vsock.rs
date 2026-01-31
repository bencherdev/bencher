//! vsock communication for guest-host interaction.
//!
//! This module provides helpers for connecting to the host via vsock
//! and sending/receiving benchmark data.

use std::io::{self, BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;

use thiserror::Error;

use crate::protocol::{BenchmarkParams, BenchmarkResults};

/// The host's CID (Context ID) for vsock.
pub const HOST_CID: u32 = 2;

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
    stream: UnixStream,
}

impl VsockConnection {
    /// Send benchmark results to the host.
    pub fn send_results(&mut self, results: &BenchmarkResults) -> Result<(), VsockError> {
        let json = serde_json::to_vec(results)?;
        self.stream.write_all(&json)?;
        self.stream.write_all(b"\n")?;
        self.stream.flush()?;
        Ok(())
    }

    /// Receive benchmark parameters from the host.
    pub fn receive_params(&mut self) -> Result<BenchmarkParams, VsockError> {
        let mut reader = BufReader::new(&self.stream);
        let mut line = String::new();
        reader.read_line(&mut line)?;
        let params = serde_json::from_str(&line)?;
        Ok(params)
    }

    /// Read raw data from the connection.
    pub fn read(&mut self, buf: &mut [u8]) -> Result<usize, VsockError> {
        Ok(self.stream.read(buf)?)
    }

    /// Write raw data to the connection.
    pub fn write(&mut self, buf: &[u8]) -> Result<usize, VsockError> {
        Ok(self.stream.write(buf)?)
    }

    /// Flush the connection.
    pub fn flush(&mut self) -> Result<(), VsockError> {
        Ok(self.stream.flush()?)
    }
}

/// Connect to the host via vsock.
///
/// This function attempts to connect to the Bencher host using vsock.
/// In a real implementation, this would use the actual vsock system calls.
///
/// # Returns
///
/// A `VsockConnection` for communicating with the host.
pub fn connect_to_host() -> Result<VsockConnection, VsockError> {
    connect_to_host_port(DEFAULT_PORT)
}

/// Connect to the host via vsock on a specific port.
pub fn connect_to_host_port(port: u32) -> Result<VsockConnection, VsockError> {
    // In a real implementation, this would use:
    // let fd = socket(AF_VSOCK, SOCK_STREAM, 0);
    // connect(fd, sockaddr_vm { cid: HOST_CID, port });
    //
    // For now, we use a Unix socket as a placeholder for development.
    // This allows testing on systems without vsock support.

    let socket_path = format!("/tmp/bencher-vsock-{port}.sock");

    match UnixStream::connect(&socket_path) {
        Ok(stream) => Ok(VsockConnection { stream }),
        Err(e) => {
            // Try the actual vsock path as fallback
            // This is the path where the VMM would create the socket
            let vsock_path = format!("/dev/vsock/{port}");
            match UnixStream::connect(&vsock_path) {
                Ok(stream) => Ok(VsockConnection { stream }),
                Err(_) => Err(VsockError::Connection(format!(
                    "Failed to connect to host: {e}"
                ))),
            }
        }
    }
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
    use super::*;

    #[test]
    fn test_vsock_error_display() {
        let err = VsockError::Connection("test error".to_owned());
        assert!(err.to_string().contains("test error"));
    }
}
