//! Minimal HTTP/1.1 client for Firecracker's REST API over Unix socket.

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::Duration;

use crate::firecracker::config::{Action, BootSource, Drive, MachineConfig, VsockConfig};
use crate::firecracker::error::FirecrackerError;

/// Client for the Firecracker REST API.
pub struct FirecrackerClient {
    socket_path: String,
}

impl FirecrackerClient {
    /// Create a new client for the given API socket path.
    pub fn new(socket_path: &str) -> Self {
        Self {
            socket_path: socket_path.to_owned(),
        }
    }

    /// Wait for the Firecracker API socket to become ready.
    pub fn wait_for_ready(&self, timeout: Duration) -> Result<(), FirecrackerError> {
        let start = std::time::Instant::now();
        let poll_interval = Duration::from_millis(50);

        while start.elapsed() < timeout {
            if Path::new(&self.socket_path).exists() {
                // Try to connect
                if let Ok(mut stream) = UnixStream::connect(&self.socket_path) {
                    stream.set_read_timeout(Some(Duration::from_secs(1))).ok();
                    stream.set_write_timeout(Some(Duration::from_secs(1))).ok();

                    let request = "GET / HTTP/1.1\r\nHost: localhost\r\nAccept: */*\r\n\r\n";
                    if stream.write_all(request.as_bytes()).is_ok() {
                        let mut buf = [0u8; 256];
                        if let Ok(n) = stream.read(&mut buf) {
                            if n > 0 {
                                return Ok(());
                            }
                        }
                    }
                }
            }
            std::thread::sleep(poll_interval);
        }

        Err(FirecrackerError::SocketNotReady(timeout))
    }

    /// Configure the machine (vCPUs, memory).
    pub fn put_machine_config(&self, config: &MachineConfig) -> Result<(), FirecrackerError> {
        let body = serde_json::to_string(config).map_err(|e| {
            FirecrackerError::ProcessStart(format!("serialize machine config: {e}"))
        })?;
        let (status, response_body) = self.http_put("/machine-config", &body)?;
        if status >= 300 {
            return Err(FirecrackerError::Api {
                status,
                body: response_body,
            });
        }
        Ok(())
    }

    /// Configure the boot source (kernel and boot args).
    pub fn put_boot_source(&self, config: &BootSource) -> Result<(), FirecrackerError> {
        let body = serde_json::to_string(config)
            .map_err(|e| FirecrackerError::ProcessStart(format!("serialize boot source: {e}")))?;
        let (status, response_body) = self.http_put("/boot-source", &body)?;
        if status >= 300 {
            return Err(FirecrackerError::Api {
                status,
                body: response_body,
            });
        }
        Ok(())
    }

    /// Configure a block device (drive).
    pub fn put_drive(&self, config: &Drive) -> Result<(), FirecrackerError> {
        let body = serde_json::to_string(config)
            .map_err(|e| FirecrackerError::ProcessStart(format!("serialize drive: {e}")))?;
        let path = format!("/drives/{}", config.drive_id);
        let (status, response_body) = self.http_put(&path, &body)?;
        if status >= 300 {
            return Err(FirecrackerError::Api {
                status,
                body: response_body,
            });
        }
        Ok(())
    }

    /// Configure the vsock device.
    pub fn put_vsock(&self, config: &VsockConfig) -> Result<(), FirecrackerError> {
        let body = serde_json::to_string(config)
            .map_err(|e| FirecrackerError::ProcessStart(format!("serialize vsock: {e}")))?;
        let (status, response_body) = self.http_put("/vsock", &body)?;
        if status >= 300 {
            return Err(FirecrackerError::Api {
                status,
                body: response_body,
            });
        }
        Ok(())
    }

    /// Perform a VM action (start, shutdown, etc.).
    pub fn put_action(&self, action: &Action) -> Result<(), FirecrackerError> {
        let body = serde_json::to_string(action)
            .map_err(|e| FirecrackerError::ProcessStart(format!("serialize action: {e}")))?;
        let (status, response_body) = self.http_put("/actions", &body)?;
        if status >= 300 {
            return Err(FirecrackerError::Api {
                status,
                body: response_body,
            });
        }
        Ok(())
    }

    /// Send an HTTP PUT request over the Unix socket.
    ///
    /// Returns the HTTP status code and response body.
    fn http_put(&self, path: &str, json_body: &str) -> Result<(u16, String), FirecrackerError> {
        let mut stream = UnixStream::connect(&self.socket_path)?;
        stream.set_read_timeout(Some(Duration::from_secs(5)))?;
        stream.set_write_timeout(Some(Duration::from_secs(5)))?;

        let request = format!(
            "PUT {path} HTTP/1.1\r\n\
             Host: localhost\r\n\
             Accept: application/json\r\n\
             Content-Type: application/json\r\n\
             Content-Length: {}\r\n\
             \r\n\
             {json_body}",
            json_body.len()
        );

        stream.write_all(request.as_bytes())?;

        // Read response
        let mut response = Vec::with_capacity(4096);
        let mut buf = [0u8; 4096];
        loop {
            match stream.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    response.extend_from_slice(&buf[..n]);
                    // Check if we have the full response (look for end of headers + body)
                    if response_complete(&response) {
                        break;
                    }
                },
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    eprintln!(
                        "Warning: Firecracker API read terminated early (WouldBlock) for PUT {path}, {read_bytes} bytes read so far",
                        read_bytes = response.len()
                    );
                    break;
                },
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    eprintln!(
                        "Warning: Firecracker API read timed out for PUT {path}, {read_bytes} bytes read so far",
                        read_bytes = response.len()
                    );
                    break;
                },
                Err(e) => return Err(FirecrackerError::Io(e)),
            }
        }

        if !response.is_empty() && !response_complete(&response) {
            eprintln!(
                "Warning: Firecracker API response for PUT {path} may be truncated ({} bytes received)",
                response.len()
            );
        }

        let (status, response_body) = parse_http_response(&response)?;

        if status >= 300 && response_body.is_empty() {
            eprintln!(
                "Warning: Firecracker API returned HTTP {status} with no body for PUT {path} ({} bytes raw response)",
                response.len()
            );
        }

        Ok((status, response_body))
    }
}

/// Check if we have received a complete HTTP response.
fn response_complete(data: &[u8]) -> bool {
    let header_end = find_header_end(data);
    let Some(header_end) = header_end else {
        return false;
    };

    let headers = String::from_utf8_lossy(&data[..header_end]);

    // Check for Content-Length (case-insensitive)
    for line in headers.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some(value) = lower.strip_prefix("content-length:") {
            if let Ok(len) = value.trim().parse::<usize>() {
                let body_start = header_end + 4; // Skip \r\n\r\n
                return data.len() >= body_start + len;
            }
        }
    }

    // No Content-Length, check for Transfer-Encoding: chunked or assume complete
    // For Firecracker's simple responses, no Content-Length usually means empty body
    true
}

/// Find the end of HTTP headers (position of first \r\n in \r\n\r\n sequence).
fn find_header_end(data: &[u8]) -> Option<usize> {
    data.windows(4).position(|w| w == b"\r\n\r\n")
}

/// Parse an HTTP response into status code and body.
fn parse_http_response(data: &[u8]) -> Result<(u16, String), FirecrackerError> {
    let response = String::from_utf8_lossy(data);

    // Parse status line: "HTTP/1.1 204 No Content\r\n..."
    let status_line = response
        .lines()
        .next()
        .ok_or_else(|| FirecrackerError::ProcessStart("empty HTTP response".to_owned()))?;

    let status_code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|s| s.parse().ok())
        .unwrap_or(500);

    // Extract body (after \r\n\r\n)
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_owned())
        .unwrap_or_default();

    Ok((status_code, body))
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- find_header_end ---

    #[test]
    fn find_header_end_normal_response() {
        let data = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}";
        assert_eq!(find_header_end(data), Some(34));
    }

    #[test]
    fn find_header_end_no_terminator() {
        let data = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n";
        assert_eq!(find_header_end(data), None);
    }

    #[test]
    fn find_header_end_empty() {
        assert_eq!(find_header_end(b""), None);
    }

    #[test]
    fn find_header_end_just_terminator() {
        assert_eq!(find_header_end(b"\r\n\r\n"), Some(0));
    }

    // --- response_complete ---

    #[test]
    fn response_complete_with_content_length_fulfilled() {
        let data = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\n{}";
        assert!(response_complete(data));
    }

    #[test]
    fn response_complete_with_content_length_incomplete() {
        let data = b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n{}";
        assert!(!response_complete(data));
    }

    #[test]
    fn response_complete_no_content_length() {
        // No Content-Length means assume complete (Firecracker convention)
        let data = b"HTTP/1.1 204 No Content\r\n\r\n";
        assert!(response_complete(data));
    }

    #[test]
    fn response_complete_no_header_end() {
        let data = b"HTTP/1.1 200 OK\r\nContent";
        assert!(!response_complete(data));
    }

    #[test]
    fn response_complete_zero_content_length() {
        let data = b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n";
        assert!(response_complete(data));
    }

    // --- parse_http_response ---

    #[test]
    fn parse_http_200_with_body() {
        let data = b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\n{\"status\":\"ok\"}";
        let (status, body) = parse_http_response(data).unwrap();
        assert_eq!(status, 200);
        assert_eq!(body, "{\"status\":\"ok\"}");
    }

    #[test]
    fn parse_http_204_no_content() {
        let data = b"HTTP/1.1 204 No Content\r\n\r\n";
        let (status, body) = parse_http_response(data).unwrap();
        assert_eq!(status, 204);
        assert_eq!(body, "");
    }

    #[test]
    fn parse_http_400_error() {
        let data = b"HTTP/1.1 400 Bad Request\r\n\r\n{\"error\":\"bad\"}";
        let (status, body) = parse_http_response(data).unwrap();
        assert_eq!(status, 400);
        assert_eq!(body, "{\"error\":\"bad\"}");
    }

    #[test]
    fn parse_http_empty_response_errors() {
        let result = parse_http_response(b"");
        assert!(result.is_err());
    }

    #[test]
    fn parse_http_malformed_status_defaults_to_500() {
        // No status code in the status line
        let data = b"HTTP/1.1\r\n\r\n";
        let (status, _) = parse_http_response(data).unwrap();
        assert_eq!(status, 500);
    }

    #[test]
    fn parse_http_non_numeric_status_defaults_to_500() {
        let data = b"HTTP/1.1 abc OK\r\n\r\n";
        let (status, _) = parse_http_response(data).unwrap();
        assert_eq!(status, 500);
    }

    #[test]
    fn parse_http_no_header_body_separator() {
        // Status line only, no \r\n\r\n — body should be empty
        let data = b"HTTP/1.1 200 OK\r\n";
        let (status, body) = parse_http_response(data).unwrap();
        assert_eq!(status, 200);
        assert_eq!(body, "");
    }
}
