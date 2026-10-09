//! Minimal HTTP/1.1 client for Firecracker's REST API over Unix socket.
#![expect(
    clippy::print_stderr,
    clippy::indexing_slicing,
    reason = "low-level HTTP client for Firecracker socket API"
)]

use std::io::{Read as _, Write as _};
use std::os::fd::AsRawFd as _;
use std::os::unix::net::UnixStream;
use std::time::{Duration, Instant};

use nix::errno::Errno;
use nix::sys::socket::{AddressFamily, SockFlag, SockType, UnixAddr, connect, socket};

use crate::firecracker::config::{Action, BootSource, Drive, MachineConfig, VsockConfig};
use crate::firecracker::error::FirecrackerError;
use crate::jail::SocketPath;

/// Longest one API call may take, from its connect to the end of the response.
const API_CALL_TIMEOUT: Duration = Duration::from_secs(5);

/// Far above any answer an honest Firecracker gives, so only a compromised VMM
/// fails a call on it.
pub(crate) const API_RESPONSE_CAP_KIB: usize = 64;

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
        match connect_until(
            self.socket_path.as_str(),
            Instant::now() + Duration::from_secs(1),
        ) {
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
        let (status, response_body) =
            self.http_put("/machine-config", &body, Instant::now() + API_CALL_TIMEOUT)?;
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
        let (status, response_body) =
            self.http_put("/boot-source", &body, Instant::now() + API_CALL_TIMEOUT)?;
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
        let (status, response_body) =
            self.http_put(&path, &body, Instant::now() + API_CALL_TIMEOUT)?;
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
        let (status, response_body) =
            self.http_put("/vsock", &body, Instant::now() + API_CALL_TIMEOUT)?;
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
        self.put_action_until(action, Instant::now() + API_CALL_TIMEOUT)
    }

    /// For a caller whose own deadline has to cover the call.
    pub fn put_action_until(
        &self,
        action: &Action,
        deadline: Instant,
    ) -> Result<(), FirecrackerError> {
        let body = serde_json::to_string(action).map_err(|e| FirecrackerError::ApiEncoding {
            context: "serialize action",
            source: e,
        })?;
        let (status, response_body) = self.http_put("/actions", &body, deadline)?;
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
    fn http_put(
        &self,
        path: &str,
        json_body: &str,
        deadline: Instant,
    ) -> Result<(u16, String), FirecrackerError> {
        let timed_out =
            || FirecrackerError::Timeout(format!("Firecracker did not answer PUT {path} in time"));
        // Only failures about the socket itself name it; errors on an
        // established stream stay plain I/O.
        let mut stream = connect_until(self.socket_path.as_str(), deadline).map_err(|e| {
            if e.kind() == std::io::ErrorKind::WouldBlock {
                timed_out()
            } else {
                self.unusable(e)
            }
        })?;
        // The request fits the empty socket buffer, so this bounds a write
        // that never waits in practice.
        stream
            .set_write_timeout(Some(time_left(deadline).ok_or_else(timed_out)?))
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
            let wait = time_left(deadline).ok_or_else(timed_out)?;
            stream
                .set_read_timeout(Some(wait))
                .map_err(|e| self.unusable(e))?;
            match stream.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    response.extend_from_slice(&buf[..n]);
                    // A declared length past the cap fails at once, before its body arrives.
                    let length = response_length(&response);
                    if response.len() > API_RESPONSE_CAP_KIB * 1024
                        || length.is_some_and(|length| length > API_RESPONSE_CAP_KIB * 1024)
                    {
                        return Err(FirecrackerError::ApiResponseTooLarge {
                            path: path.to_owned(),
                        });
                    }
                    if length.is_some_and(|length| response.len() >= length) {
                        break;
                    }
                },
                // A read with a timeout is interrupted by any signal rather
                // than restarted.
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {},
                // The check above decides whether the deadline has passed,
                // since the wait can end up to a tick early.
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {},
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

/// Like `UnixStream::connect`, but a full listen backlog is waited on only
/// until `deadline`, failing with `WouldBlock`.
fn connect_until(path: &str, deadline: Instant) -> std::io::Result<UnixStream> {
    let address = UnixAddr::new(path)?;
    let stream = UnixStream::from(socket(
        AddressFamily::Unix,
        SockType::Stream,
        SockFlag::SOCK_CLOEXEC,
        None,
    )?);
    loop {
        let Some(left) = time_left(deadline) else {
            return Err(std::io::ErrorKind::WouldBlock.into());
        };
        // Linux waits on a full backlog for at most the send timeout.
        stream.set_write_timeout(Some(left))?;
        match connect(stream.as_raw_fd(), &address) {
            Ok(()) => break,
            Err(Errno::EINTR) => {},
            Err(errno) => return Err(errno.into()),
        }
    }
    // Otherwise the caller's writes inherit what was left of the connect's wait.
    stream.set_write_timeout(None)?;
    Ok(stream)
}

/// `None` once `deadline` has passed, since a socket timeout cannot be zero.
fn time_left(deadline: Instant) -> Option<Duration> {
    let left = deadline.saturating_duration_since(Instant::now());
    (!left.is_zero()).then_some(left)
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
    response_length(data).is_some_and(|length| data.len() >= length)
}

/// The length of the whole response, as its headers declare it once they end.
fn response_length(data: &[u8]) -> Option<usize> {
    let header_end = find_header_end(data)?;
    let body_start = header_end + 4; // Skip \r\n\r\n

    let headers = String::from_utf8_lossy(&data[..header_end]);

    // Check for Content-Length (case-insensitive)
    for line in headers.lines() {
        let lower = line.to_ascii_lowercase();
        if let Some(value) = lower.strip_prefix("content-length:")
            && let Ok(len) = value.trim().parse::<usize>()
        {
            // Saturating, since a length that overflows is just past the cap.
            return Some(body_start.saturating_add(len));
        }
    }

    // No Content-Length, check for Transfer-Encoding: chunked or assume complete
    // For Firecracker's simple responses, no Content-Length usually means empty body
    Some(body_start)
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
    use std::os::unix::net::UnixListener;

    use camino::Utf8Path;

    use super::*;
    use crate::firecracker::config::ActionType;
    use crate::firecracker::test_util::interrupt;
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

    #[test]
    fn a_signal_while_waiting_for_the_response_does_not_fail_the_call() {
        // Prevents a signal or stop during an API call failing the job's VM setup.
        let dir = tempfile::tempdir().unwrap();
        let jail = JailPaths::new(Utf8Path::from_path(dir.path()).unwrap()).unwrap();
        let vmm = UnixListener::bind(jail.api_socket().host().as_path()).unwrap();
        let socket = jail.api_socket().socket().clone();

        let client = std::thread::spawn(move || {
            FirecrackerClient::new(&socket).put_action(&Action {
                action_type: ActionType::SendCtrlAltDel,
            })
        });
        let (mut stream, _) = vmm.accept().unwrap();
        assert!(
            stream.read(&mut [0u8; 512]).unwrap() > 0,
            "the client must send its request before it waits"
        );
        drop(stream.write_all(b"HTTP/1.1 204 No Content\r\n"));
        interrupt(&client);
        drop(stream.write_all(b"\r\n"));
        drop(stream);

        client.join().unwrap().unwrap();
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

    #[test]
    fn a_trickling_vmm_cannot_hold_an_api_call() {
        // Prevents a VMM that paces its response holding the job thread, as the
        // teardown request after a timed out job would.
        let (_dir, jail, vmm) = vmm_in_tmpdir();
        let socket = jail.api_socket().socket().clone();

        let started = Instant::now();
        let client = std::thread::spawn(move || {
            FirecrackerClient::new(&socket).put_action_until(&ctrl_alt_del(), deadline_in_300ms())
        });
        let (mut stream, _) = vmm.accept().unwrap();
        assert!(stream.read(&mut [0u8; 512]).unwrap() > 0);
        drop(stream.write_all(b"HTTP/1.1 204 No Content\r\n"));
        while !client.is_finished() && started.elapsed() < SLOW && stream.write_all(b"X").is_ok() {
            std::thread::sleep(Duration::from_millis(20));
        }
        let held = started.elapsed();
        drop(stream);

        assert!(
            held < SLOW,
            "the call must end at its deadline, held {held:?}"
        );
        let result = client.join().unwrap();
        assert!(
            matches!(result, Err(FirecrackerError::Timeout(_))),
            "an unfinished response is a timeout, got: {result:?}"
        );
    }

    #[test]
    fn a_silent_vmm_fails_the_call_at_its_deadline() {
        // Prevents acting on a response cut short, as if a status line alone
        // were a 204.
        let (_dir, jail, vmm) = vmm_in_tmpdir();
        let socket = jail.api_socket().socket().clone();

        let started = Instant::now();
        let client = std::thread::spawn(move || {
            FirecrackerClient::new(&socket).put_action_until(&ctrl_alt_del(), deadline_in_300ms())
        });
        let (mut stream, _) = vmm.accept().unwrap();
        assert!(stream.read(&mut [0u8; 512]).unwrap() > 0);
        drop(stream.write_all(b"HTTP/1.1 204 No Content\r\n"));
        while !client.is_finished() && started.elapsed() < SLOW {
            std::thread::sleep(Duration::from_millis(10));
        }
        let held = started.elapsed();
        drop(stream);

        assert!(
            held < SLOW,
            "the call must end at its deadline, held {held:?}"
        );
        let result = client.join().unwrap();
        assert!(
            matches!(result, Err(FirecrackerError::Timeout(_))),
            "an unfinished response is a timeout, got: {result:?}"
        );
    }

    #[test]
    fn a_vmm_that_never_accepts_cannot_hold_the_connect() {
        // Prevents a connect that waits on a full listen backlog outlasting the
        // call's deadline.
        use nix::sys::socket::{
            AddressFamily, Backlog, SockFlag, SockType, UnixAddr, bind, listen, socket,
        };

        let dir = tempfile::tempdir().unwrap();
        let jail = JailPaths::new(Utf8Path::from_path(dir.path()).unwrap()).unwrap();
        let host = jail.api_socket().host().as_path();
        let vmm = socket(
            AddressFamily::Unix,
            SockType::Stream,
            SockFlag::SOCK_CLOEXEC,
            None,
        )
        .unwrap();
        bind(vmm.as_raw_fd(), &UnixAddr::new(host.as_str()).unwrap()).unwrap();
        listen(&vmm, Backlog::new(0).unwrap()).unwrap();
        // A backlog of zero holds one connection, so this fills it.
        let _queued = UnixStream::connect(host).unwrap();
        let socket = jail.api_socket().socket().clone();

        let started = Instant::now();
        let client = std::thread::spawn(move || {
            FirecrackerClient::new(&socket).put_action_until(&ctrl_alt_del(), deadline_in_300ms())
        });
        while !client.is_finished() && started.elapsed() < SLOW {
            std::thread::sleep(Duration::from_millis(10));
        }
        let held = started.elapsed();
        // Wakes a connect that is still waiting.
        drop(vmm);

        assert!(
            held < SLOW,
            "the connect must end at the deadline, held {held:?}"
        );
        let result = client.join().unwrap();
        assert!(
            matches!(result, Err(FirecrackerError::Timeout(_))),
            "a connect that never completes is a timeout, got: {result:?}"
        );
    }

    #[test]
    fn a_reset_connection_fails_the_call_at_once() {
        // Prevents retrying every error, which turns a reset into a wait for the
        // deadline.
        use std::os::fd::AsFd as _;

        use nix::poll::{PollFd, PollFlags, PollTimeout, poll};

        let (_dir, jail, vmm) = vmm_in_tmpdir();
        let socket = jail.api_socket().socket().clone();

        let client = std::thread::spawn(move || {
            FirecrackerClient::new(&socket)
                .put_action_until(&ctrl_alt_del(), Instant::now() + Duration::from_secs(30))
        });
        let (stream, _) = vmm.accept().unwrap();
        let mut request = [PollFd::new(stream.as_fd(), PollFlags::POLLIN)];
        poll(&mut request, PollTimeout::try_from(1000).unwrap()).unwrap();
        // Closing with the request unread resets the client's end.
        drop(stream);

        let result = client.join().unwrap();
        assert!(
            matches!(
                &result,
                Err(FirecrackerError::Io(e)) if e.kind() == std::io::ErrorKind::ConnectionReset
            ),
            "a reset is an I/O error, got: {result:?}"
        );
    }

    #[test]
    fn an_endless_response_fails_the_call_at_the_cap() {
        // Prevents a VMM that never ends its answer growing the runner's memory
        // for as long as the deadline allows.
        let (_dir, jail, vmm) = vmm_in_tmpdir();
        let socket = jail.api_socket().socket().clone();

        let started = Instant::now();
        let client = std::thread::spawn(move || {
            FirecrackerClient::new(&socket)
                .put_action_until(&ctrl_alt_del(), Instant::now() + Duration::from_secs(30))
        });
        let (mut stream, _) = vmm.accept().unwrap();
        assert!(stream.read(&mut [0u8; 512]).unwrap() > 0);
        // Headers that never end, the one answer no declared length cuts short,
        // so only the cap can end the read.
        drop(stream.write_all(b"HTTP/1.1 200 OK\r\nX-Padding: "));
        let mut sent = 0;
        while sent < 16 * 1024 * 1024
            && started.elapsed() < SLOW
            && let Ok(n) = stream.write(&[b'X'; 4096])
        {
            sent += n;
        }
        drop(stream);

        let result = client.join().unwrap();
        assert!(
            matches!(result, Err(FirecrackerError::ApiResponseTooLarge { .. })),
            "an answer past the cap fails the call, got: {result:?}"
        );
        let message = result.unwrap_err().to_string();
        assert!(
            message.contains(&format!("{API_RESPONSE_CAP_KIB} KiB")),
            "the error must name the cap: {message}"
        );
        // The cap, plus at most a socket buffer in flight.
        assert!(
            sent < 1024 * 1024,
            "the call must stop reading at the cap, but took {sent} bytes"
        );
    }

    #[test]
    fn a_complete_response_over_the_cap_fails_the_call() {
        // Prevents checking the cap only while the response is unfinished,
        // which lets one that ends past the cap through.
        let (_dir, jail, vmm) = vmm_in_tmpdir();
        let socket = jail.api_socket().socket().clone();

        let client = std::thread::spawn(move || {
            FirecrackerClient::new(&socket)
                .put_action_until(&ctrl_alt_del(), Instant::now() + Duration::from_secs(30))
        });
        let (mut stream, _) = vmm.accept().unwrap();
        assert!(stream.read(&mut [0u8; 512]).unwrap() > 0);
        // Its blank line, which completes it, is its last byte, one past the cap.
        let head = "HTTP/1.1 200 OK\r\nX-Padding: ";
        let end = "\r\n\r\n";
        let padding = "X".repeat(API_RESPONSE_CAP_KIB * 1024 + 1 - head.len() - end.len());
        drop(stream.write_all(format!("{head}{padding}{end}").as_bytes()));
        drop(stream);

        let result = client.join().unwrap();
        assert!(
            matches!(result, Err(FirecrackerError::ApiResponseTooLarge { .. })),
            "an answer past the cap fails the call, got: {result:?}"
        );
    }

    #[test]
    fn a_declared_length_past_the_cap_fails_the_call_at_once() {
        // Prevents a declared length that overflows panicking the client, and
        // one past the cap waiting on its body.
        let (_dir, jail, vmm) = vmm_in_tmpdir();
        let socket = jail.api_socket().socket().clone();

        let client = std::thread::spawn(move || {
            FirecrackerClient::new(&socket)
                .put_action_until(&ctrl_alt_del(), Instant::now() + Duration::from_secs(30))
        });
        let (mut stream, _) = vmm.accept().unwrap();
        assert!(stream.read(&mut [0u8; 512]).unwrap() > 0);
        let head = format!(
            "HTTP/1.1 204 No Content\r\nContent-Length: {}\r\n\r\n",
            usize::MAX
        );
        drop(stream.write_all(head.as_bytes()));

        // Joined with the stream still open, so only an error at once ends the call.
        let result = client.join().unwrap();
        drop(stream);
        assert!(
            matches!(result, Err(FirecrackerError::ApiResponseTooLarge { .. })),
            "a declared length past the cap fails the call, got: {result:?}"
        );
    }

    /// Long past every deadline these tests set, and short of
    /// `API_CALL_TIMEOUT`, so a call that ignores its deadline fails.
    const SLOW: Duration = Duration::from_secs(3);

    fn vmm_in_tmpdir() -> (tempfile::TempDir, JailPaths, UnixListener) {
        let dir = tempfile::tempdir().unwrap();
        let jail = JailPaths::new(Utf8Path::from_path(dir.path()).unwrap()).unwrap();
        let vmm = UnixListener::bind(jail.api_socket().host().as_path()).unwrap();
        (dir, jail, vmm)
    }

    fn deadline_in_300ms() -> Instant {
        Instant::now() + Duration::from_millis(300)
    }

    fn ctrl_alt_del() -> Action {
        Action {
            action_type: ActionType::SendCtrlAltDel,
        }
    }
}
