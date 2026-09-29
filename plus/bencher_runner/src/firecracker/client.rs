//! Minimal HTTP/1.1 client for Firecracker's REST API over Unix socket.
#![expect(
    clippy::print_stderr,
    clippy::indexing_slicing,
    reason = "low-level HTTP client for Firecracker socket API"
)]

use std::io::{Read as _, Write as _};
use std::os::unix::net::UnixStream;
use std::time::Duration;

use crate::firecracker::config::{Action, BootSource, Drive, MachineConfig, VsockConfig};
use crate::firecracker::error::FirecrackerError;
use crate::jail::SocketPath;

/// Client for the Firecracker REST API.
pub struct FirecrackerClient<'a> {
    /// Borrowed, so the client cannot outlive the descriptor this path names.
    socket_path: &'a SocketPath,
}

impl<'a> FirecrackerClient<'a> {
    /// Takes the socket view, since the runner connects from outside the chroot.
    pub fn new(socket_path: &'a SocketPath) -> Self {
        Self { socket_path }
    }

    /// `Ok(false)` while Firecracker is not listening yet, and an error for an
    /// address that waiting cannot fix.
    pub fn try_ready(&self) -> Result<bool, FirecrackerError> {
        match UnixStream::connect(self.socket_path.as_str()) {
            Ok(mut stream) => {
                drop(stream.set_read_timeout(Some(Duration::from_secs(1))));
                drop(stream.set_write_timeout(Some(Duration::from_secs(1))));

                let request = "GET / HTTP/1.1\r\nHost: localhost\r\nAccept: */*\r\n\r\n";
                if stream.write_all(request.as_bytes()).is_ok() {
                    let mut buf = [0u8; 256];
                    if let Ok(n) = stream.read(&mut buf)
                        && n > 0
                    {
                        return Ok(true);
                    }
                }
                Ok(false)
            },
            Err(e) if is_not_listening_yet(&e) => Ok(false),
            Err(e) => Err(FirecrackerError::SocketUnusable {
                path: self.socket_path.clone(),
                source: e,
            }),
        }
    }

    /// Configure the machine (vCPUs, memory).
    pub fn put_machine_config(&self, config: &MachineConfig) -> Result<(), FirecrackerError> {
        let body = serde_json::to_string(config).map_err(|e| FirecrackerError::ApiEncoding {
            context: "serialize machine config",
            source: e,
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
        let body = serde_json::to_string(config).map_err(|e| FirecrackerError::ApiEncoding {
            context: "serialize boot source",
            source: e,
        })?;
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
        let body = serde_json::to_string(config).map_err(|e| FirecrackerError::ApiEncoding {
            context: "serialize drive",
            source: e,
        })?;
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
        let body = serde_json::to_string(config).map_err(|e| FirecrackerError::ApiEncoding {
            context: "serialize vsock",
            source: e,
        })?;
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
        let body = serde_json::to_string(action).map_err(|e| FirecrackerError::ApiEncoding {
            context: "serialize action",
            source: e,
        })?;
        let (status, response_body) = self.http_put("/actions", &body)?;
        if status >= 300 {
            return Err(FirecrackerError::Api {
                status,
                body: response_body,
            });
        }
        Ok(())
    }

    fn unusable(&self, source: std::io::Error) -> FirecrackerError {
        FirecrackerError::SocketUnusable {
            path: self.socket_path.clone(),
            source,
        }
    }

    /// Send an HTTP PUT request over the Unix socket.
    ///
    /// Returns the HTTP status code and response body.
    fn http_put(&self, path: &str, json_body: &str) -> Result<(u16, String), FirecrackerError> {
        // Only failures about the socket itself name it; errors on an
        // established stream stay plain I/O.
        let mut stream =
            UnixStream::connect(self.socket_path.as_str()).map_err(|e| self.unusable(e))?;
        stream
            .set_read_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| self.unusable(e))?;
        stream
            .set_write_timeout(Some(Duration::from_secs(5)))
            .map_err(|e| self.unusable(e))?;

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

/// The errors a booting VMM produces; any other describes the address itself.
fn is_not_listening_yet(error: &std::io::Error) -> bool {
    matches!(
        error.kind(),
        std::io::ErrorKind::NotFound
            | std::io::ErrorKind::ConnectionRefused
            | std::io::ErrorKind::WouldBlock
            | std::io::ErrorKind::Interrupted
    )
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
        if let Some(value) = lower.strip_prefix("content-length:")
            && let Ok(len) = value.trim().parse::<usize>()
        {
            let body_start = header_end + 4; // Skip \r\n\r\n
            return data.len() >= body_start + len;
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
        .ok_or(FirecrackerError::MalformedResponse("empty HTTP response"))?;

    let status_code: u16 = status_line
        .split_whitespace()
        .nth(1)
        .and_then(|code| code.parse().ok())
        .ok_or(FirecrackerError::MalformedResponse(
            "HTTP status line carries no status code",
        ))?;

    // Extract body (after \r\n\r\n)
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, b)| b.to_owned())
        .unwrap_or_default();

    Ok((status_code, body))
}

#[cfg(test)]
mod tests {
    use camino::Utf8Path;

    use super::*;
    use crate::firecracker::config::ActionType;
    use crate::jail::JailPaths;

    #[test]
    fn an_api_call_that_cannot_reach_the_socket_names_it() {
        // Prevents a call after readiness failing as a bare I/O error that never names the socket.
        let dir = tempfile::tempdir().unwrap();
        let jail = JailPaths::new(Utf8Path::from_path(dir.path()).unwrap()).unwrap();
        let client = FirecrackerClient::new(jail.api_socket().socket());

        let Err(err) = client.put_action(&Action {
            action_type: ActionType::SendCtrlAltDel,
        }) else {
            panic!("nothing is listening on this path");
        };

        assert!(
            matches!(err, FirecrackerError::SocketUnusable { .. }),
            "an address that cannot be reached is not a bare I/O error: {err}"
        );
        assert!(
            err.to_string()
                .contains(jail.api_socket().socket().as_str()),
            "the error must name the socket: {err}"
        );
    }

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
        parse_http_response(b"").unwrap_err();
    }

    #[test]
    fn a_status_line_without_a_status_is_malformed_not_a_500() {
        // Prevents an unparseable status being reported as a 500 Firecracker never sent.
        let data = b"HTTP/1.1\r\n\r\n";

        let err = parse_http_response(data).unwrap_err();

        assert!(
            matches!(err, FirecrackerError::MalformedResponse(_)),
            "a response that could not be parsed is not a status: {err}"
        );
    }

    #[test]
    fn a_status_that_is_not_a_number_is_malformed_not_a_500() {
        let data = b"HTTP/1.1 abc OK\r\n\r\n";

        let err = parse_http_response(data).unwrap_err();

        assert!(
            matches!(err, FirecrackerError::MalformedResponse(_)),
            "{err}"
        );
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
