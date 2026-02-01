//! Host-side vsock client for communicating with VMs.
//!
//! This module provides a client for connecting to a VM via its vsock Unix socket.
//! The client can send benchmark requests and receive results.

use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use camino::Utf8Path;

use crate::error::VmmError;

/// Default buffer allocation for vsock connections.
const DEFAULT_BUF_ALLOC: u32 = 64 * 1024;

/// A client for communicating with a VM via vsock.
pub struct VsockClient {
    /// The Unix stream connection.
    stream: UnixStream,
    /// Buffer for reading lines.
    reader: BufReader<UnixStream>,
}

impl VsockClient {
    /// Connect to a VM's vsock Unix socket.
    ///
    /// # Arguments
    ///
    /// * `socket_path` - Path to the Unix socket created by the VMM
    /// * `timeout` - Optional connection timeout
    ///
    /// # Returns
    ///
    /// A connected vsock client.
    pub fn connect(socket_path: &Utf8Path, timeout: Option<Duration>) -> Result<Self, VmmError> {
        let stream = UnixStream::connect(socket_path)?;

        if let Some(t) = timeout {
            stream.set_read_timeout(Some(t))?;
            stream.set_write_timeout(Some(t))?;
        }

        let reader = BufReader::new(stream.try_clone()?);

        Ok(Self { stream, reader })
    }

    /// Set read timeout.
    pub fn set_read_timeout(&self, timeout: Option<Duration>) -> Result<(), VmmError> {
        self.stream.set_read_timeout(timeout)?;
        Ok(())
    }

    /// Set write timeout.
    pub fn set_write_timeout(&self, timeout: Option<Duration>) -> Result<(), VmmError> {
        self.stream.set_write_timeout(timeout)?;
        Ok(())
    }

    /// Send a message to the VM.
    ///
    /// # Arguments
    ///
    /// * `message` - The message to send
    pub fn send(&mut self, message: &[u8]) -> Result<(), VmmError> {
        self.stream.write_all(message)?;
        self.stream.flush()?;
        Ok(())
    }

    /// Send a line of text to the VM (appends newline).
    pub fn send_line(&mut self, line: &str) -> Result<(), VmmError> {
        writeln!(self.stream, "{line}")?;
        self.stream.flush()?;
        Ok(())
    }

    /// Receive a message from the VM.
    ///
    /// # Arguments
    ///
    /// * `buffer` - Buffer to read into
    ///
    /// # Returns
    ///
    /// The number of bytes read.
    pub fn recv(&mut self, buffer: &mut [u8]) -> Result<usize, VmmError> {
        let n = self.reader.read(buffer)?;
        Ok(n)
    }

    /// Read a line from the VM.
    ///
    /// # Returns
    ///
    /// The line read (without trailing newline).
    pub fn recv_line(&mut self) -> Result<String, VmmError> {
        let mut line = String::new();
        self.reader.read_line(&mut line)?;
        // Remove trailing newline
        if line.ends_with('\n') {
            line.pop();
            if line.ends_with('\r') {
                line.pop();
            }
        }
        Ok(line)
    }

    /// Read all available data from the VM.
    ///
    /// This is useful for reading complete responses.
    pub fn recv_all(&mut self) -> Result<Vec<u8>, VmmError> {
        let mut data = Vec::new();
        self.reader.read_to_end(&mut data)?;
        Ok(data)
    }

    /// Send a JSON request and receive a JSON response.
    ///
    /// This is a convenience method for the common pattern of sending a
    /// JSON request and receiving a JSON response, each on a single line.
    pub fn request(&mut self, request: &str) -> Result<String, VmmError> {
        self.send_line(request)?;
        self.recv_line()
    }

    /// Close the connection.
    pub fn close(self) -> Result<(), VmmError> {
        drop(self.stream);
        Ok(())
    }
}

/// A builder for vsock client connections.
pub struct VsockClientBuilder {
    socket_path: String,
    read_timeout: Option<Duration>,
    write_timeout: Option<Duration>,
}

impl VsockClientBuilder {
    /// Create a new builder.
    pub fn new(socket_path: &Utf8Path) -> Self {
        Self {
            socket_path: socket_path.to_string(),
            read_timeout: None,
            write_timeout: None,
        }
    }

    /// Set the read timeout.
    #[must_use]
    pub fn read_timeout(mut self, timeout: Duration) -> Self {
        self.read_timeout = Some(timeout);
        self
    }

    /// Set the write timeout.
    #[must_use]
    pub fn write_timeout(mut self, timeout: Duration) -> Self {
        self.write_timeout = Some(timeout);
        self
    }

    /// Set both read and write timeouts.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.read_timeout = Some(timeout);
        self.write_timeout = Some(timeout);
        self
    }

    /// Connect to the VM.
    pub fn connect(self) -> Result<VsockClient, VmmError> {
        let client = VsockClient::connect(Utf8Path::new(&self.socket_path), None)?;

        if let Some(t) = self.read_timeout {
            client.set_read_timeout(Some(t))?;
        }
        if let Some(t) = self.write_timeout {
            client.set_write_timeout(Some(t))?;
        }

        Ok(client)
    }
}

/// A protocol handler for benchmark requests/responses.
///
/// This wraps a VsockClient and provides a higher-level interface
/// for sending benchmark requests and receiving results.
pub struct BenchmarkClient {
    client: VsockClient,
}

impl BenchmarkClient {
    /// Create a new benchmark client.
    pub fn new(client: VsockClient) -> Self {
        Self { client }
    }

    /// Connect to a VM.
    pub fn connect(socket_path: &Utf8Path) -> Result<Self, VmmError> {
        let client = VsockClient::connect(socket_path, Some(Duration::from_secs(30)))?;
        Ok(Self::new(client))
    }

    /// Run a benchmark command in the VM.
    ///
    /// # Arguments
    ///
    /// * `command` - The command to run
    /// * `args` - Command arguments
    ///
    /// # Returns
    ///
    /// The benchmark results as a JSON string.
    pub fn run_benchmark(&mut self, command: &str, args: &[&str]) -> Result<String, VmmError> {
        // Build the request
        let request = serde_json::json!({
            "command": command,
            "args": args,
        });

        // Send request and get response
        let response = self.client.request(&request.to_string())?;

        Ok(response)
    }

    /// Send a shutdown command to the VM.
    pub fn shutdown(&mut self) -> Result<(), VmmError> {
        let request = serde_json::json!({
            "command": "shutdown",
        });
        self.client.send_line(&request.to_string())?;
        Ok(())
    }

    /// Check if the VM is ready to receive commands.
    pub fn ping(&mut self) -> Result<bool, VmmError> {
        let request = serde_json::json!({
            "command": "ping",
        });
        let response = self.client.request(&request.to_string())?;
        Ok(response.contains("pong"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixListener;
    use std::thread;

    /// Create a temporary socket path for testing.
    fn temp_socket_path() -> String {
        format!(
            "/tmp/bencher_vmm_test_{}.sock",
            std::process::id()
        )
    }

    #[test]
    fn test_vsock_client_send_recv() {
        let socket_path = temp_socket_path();
        let _ = std::fs::remove_file(&socket_path);

        // Create a listener
        let listener = UnixListener::bind(&socket_path).unwrap();

        // Spawn a thread to accept and echo
        let socket_path_clone = socket_path.clone();
        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let mut buf = [0u8; 1024];
            let n = stream.read(&mut buf).unwrap();
            stream.write_all(&buf[..n]).unwrap();
        });

        // Connect client
        let mut client = VsockClient::connect(
            Utf8Path::new(&socket_path),
            Some(Duration::from_secs(5)),
        )
        .unwrap();

        // Send and receive
        let message = b"hello world";
        client.send(message).unwrap();

        let mut response = [0u8; 1024];
        let n = client.recv(&mut response).unwrap();

        assert_eq!(&response[..n], message);

        handle.join().unwrap();
        let _ = std::fs::remove_file(&socket_path);
    }

    #[test]
    fn test_vsock_client_send_recv_line() {
        let socket_path = temp_socket_path() + "_line";
        let _ = std::fs::remove_file(&socket_path);

        let listener = UnixListener::bind(&socket_path).unwrap();

        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut writer = stream;

            let mut line = String::new();
            reader.read_line(&mut line).unwrap();

            // Echo back with "echo: " prefix
            writeln!(writer, "echo: {}", line.trim()).unwrap();
        });

        let mut client = VsockClient::connect(
            Utf8Path::new(&socket_path),
            Some(Duration::from_secs(5)),
        )
        .unwrap();

        client.send_line("test message").unwrap();
        let response = client.recv_line().unwrap();

        assert_eq!(response, "echo: test message");

        handle.join().unwrap();
        let _ = std::fs::remove_file(&socket_path);
    }

    #[test]
    fn test_vsock_client_request_response() {
        let socket_path = temp_socket_path() + "_req";
        let _ = std::fs::remove_file(&socket_path);

        let listener = UnixListener::bind(&socket_path).unwrap();

        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut writer = stream;

            let mut line = String::new();
            reader.read_line(&mut line).unwrap();

            // Parse as JSON and respond
            if line.contains("ping") {
                writeln!(writer, r#"{{"status": "pong"}}"#).unwrap();
            } else {
                writeln!(writer, r#"{{"error": "unknown"}}"#).unwrap();
            }
        });

        let mut client = VsockClient::connect(
            Utf8Path::new(&socket_path),
            Some(Duration::from_secs(5)),
        )
        .unwrap();

        let response = client.request(r#"{"command": "ping"}"#).unwrap();
        assert!(response.contains("pong"));

        handle.join().unwrap();
        let _ = std::fs::remove_file(&socket_path);
    }

    #[test]
    fn test_vsock_client_builder() {
        let socket_path = temp_socket_path() + "_builder";
        let _ = std::fs::remove_file(&socket_path);

        let listener = UnixListener::bind(&socket_path).unwrap();

        let handle = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            stream.write_all(b"ok").unwrap();
        });

        let mut client = VsockClientBuilder::new(Utf8Path::new(&socket_path))
            .timeout(Duration::from_secs(5))
            .connect()
            .unwrap();

        let mut buf = [0u8; 2];
        let n = client.recv(&mut buf).unwrap();
        assert_eq!(&buf[..n], b"ok");

        handle.join().unwrap();
        let _ = std::fs::remove_file(&socket_path);
    }

    #[test]
    fn test_benchmark_client_ping() {
        let socket_path = temp_socket_path() + "_bench";
        let _ = std::fs::remove_file(&socket_path);

        let listener = UnixListener::bind(&socket_path).unwrap();

        let handle = thread::spawn(move || {
            let (stream, _) = listener.accept().unwrap();
            let mut reader = BufReader::new(stream.try_clone().unwrap());
            let mut writer = stream;

            let mut line = String::new();
            reader.read_line(&mut line).unwrap();

            if line.contains("ping") {
                writeln!(writer, r#"{{"status": "pong"}}"#).unwrap();
            }
        });

        let vsock = VsockClient::connect(
            Utf8Path::new(&socket_path),
            Some(Duration::from_secs(5)),
        )
        .unwrap();

        let mut client = BenchmarkClient::new(vsock);
        let result = client.ping().unwrap();
        assert!(result);

        handle.join().unwrap();
        let _ = std::fs::remove_file(&socket_path);
    }
}
