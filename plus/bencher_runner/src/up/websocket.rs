use std::net::TcpStream;
use std::time::Duration;

use bencher_json::JsonClaimedJob;
use bencher_json::runner::{RunnerMessage, ServerMessage};
use tungstenite::handshake::client::generate_key;
use tungstenite::http::Request;
use tungstenite::protocol::CloseFrame;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};
use url::Url;

use super::error::WebSocketError;

pub struct JobChannel {
    ws: WebSocket<MaybeTlsStream<TcpStream>>,
}

impl JobChannel {
    pub fn connect(ws_url: &Url, token: &str) -> Result<Self, WebSocketError> {
        let request = Request::builder()
            .uri(ws_url.as_str())
            .header(
                bencher_json::AUTHORIZATION,
                bencher_json::bearer_header(token),
            )
            .header("Sec-WebSocket-Version", "13")
            .header("Connection", "Upgrade")
            .header("Upgrade", "websocket")
            .header("Sec-WebSocket-Key", generate_key())
            .header(
                "Host",
                match ws_url.port() {
                    Some(port) => {
                        format!("{}:{port}", ws_url.host_str().unwrap_or("localhost"))
                    },
                    None => ws_url.host_str().unwrap_or("localhost").to_owned(),
                },
            )
            .body(())
            .map_err(|e| WebSocketError::Connection(format!("Failed to build request: {e}")))?;

        let (ws, _response) =
            tungstenite::connect(request).map_err(|e| WebSocketError::Connection(e.to_string()))?;

        Ok(Self { ws })
    }

    pub fn send_message(&mut self, msg: &RunnerMessage) -> Result<(), WebSocketError> {
        let json = serde_json::to_string(msg)
            .map_err(|e| WebSocketError::Send(format!("Failed to serialize message: {e}")))?;
        self.ws
            .send(Message::Text(json.into()))
            .map_err(|e| WebSocketError::Send(e.to_string()))?;
        Ok(())
    }

    /// Block-read until the server sends `Job(..)` or `NoJob`.
    ///
    /// Returns `Ok(Some(job))` on `Job`, `Ok(None)` on `NoJob`,
    /// or an error on timeout/disconnect.
    ///
    /// Uses an `Instant`-based deadline so that Ping frames (which reset the OS
    /// read timeout) cannot extend the wait beyond the original `timeout`.
    pub fn wait_for_job(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<JsonClaimedJob>, WebSocketError> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Err(WebSocketError::Receive(
                    "Timed out waiting for job".to_owned(),
                ));
            }

            let stream = self.ws.get_mut();
            set_read_timeout(stream, Some(remaining))?;
            let msg = self
                .ws
                .read()
                .map_err(|e| WebSocketError::Receive(e.to_string()))?;
            let stream = self.ws.get_mut();
            set_read_timeout(stream, None)?;

            match msg {
                Message::Text(text) => {
                    let server_msg: ServerMessage = serde_json::from_str(&text).map_err(|e| {
                        WebSocketError::UnexpectedMessage(format!(
                            "Failed to parse server message: {e} (raw: {text})"
                        ))
                    })?;
                    match server_msg {
                        ServerMessage::Job(job) => return Ok(Some(*job)),
                        ServerMessage::NoJob => return Ok(None),
                        ServerMessage::Ack { .. } => {
                            // Stale Ack from the previous job completion — safe to ignore.
                            // This happens when the server's Ack arrives after the runner
                            // has already moved on to requesting the next job.
                        },
                        ServerMessage::Cancel => {
                            return Err(WebSocketError::Protocol(format!(
                                "Expected Job or NoJob, got {server_msg:?}"
                            )));
                        },
                    }
                },
                Message::Ping(data) => {
                    self.ws
                        .send(Message::Pong(data))
                        .map_err(|e| WebSocketError::Send(format!("Failed to send pong: {e}")))?;
                },
                Message::Close(frame) => return Self::handle_close_frame(frame).map(|_| None),
                Message::Binary(_) | Message::Pong(_) | Message::Frame(_) => {},
            }
        }
    }

    /// Send a WebSocket close frame (best-effort).
    pub fn close(&mut self) {
        drop(self.ws.close(None));
    }

    pub fn try_read_message(&mut self) -> Result<Option<ServerMessage>, WebSocketError> {
        // Set socket to non-blocking for a quick check
        let stream = self.ws.get_mut();
        set_nonblocking(stream, true)?;
        let result = self.ws.read();
        // Always restore blocking mode, even if read failed
        let restore_result = set_nonblocking(self.ws.get_mut(), false);

        // Process read result first (more informative error than restore failure)
        let msg = match result {
            Ok(Message::Text(text)) => {
                let msg: ServerMessage = serde_json::from_str(&text).map_err(|e| {
                    WebSocketError::UnexpectedMessage(format!(
                        "Failed to parse server message: {e} (raw: {text})"
                    ))
                })?;
                Ok(Some(msg))
            },
            Ok(Message::Ping(data)) => {
                // Respond to ping with pong
                self.ws
                    .send(Message::Pong(data))
                    .map_err(|e| WebSocketError::Send(format!("Failed to send pong: {e}")))?;
                Ok(None)
            },
            Ok(Message::Close(frame)) => Self::handle_close_frame(frame),
            Ok(_) => Ok(None),
            Err(tungstenite::Error::Io(e)) if e.kind() == std::io::ErrorKind::WouldBlock => {
                Ok(None)
            },
            Err(e) => Err(WebSocketError::Receive(e.to_string())),
        };

        // Then check restore (read errors take priority over restore errors)
        restore_result?;
        msg
    }

    pub fn read_message_timeout(
        &mut self,
        timeout: Duration,
    ) -> Result<Option<ServerMessage>, WebSocketError> {
        let deadline = std::time::Instant::now() + timeout;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            if remaining.is_zero() {
                return Ok(None);
            }

            let stream = self.ws.get_mut();
            set_read_timeout(stream, Some(remaining))?;
            let result = self.ws.read();
            let stream = self.ws.get_mut();
            set_read_timeout(stream, None)?;

            match result {
                Ok(Message::Text(text)) => {
                    let msg: ServerMessage = serde_json::from_str(&text).map_err(|e| {
                        WebSocketError::UnexpectedMessage(format!(
                            "Failed to parse server message: {e} (raw: {text})"
                        ))
                    })?;
                    return Ok(Some(msg));
                },
                Ok(Message::Ping(data)) => {
                    self.ws
                        .send(Message::Pong(data))
                        .map_err(|e| WebSocketError::Send(format!("Failed to send pong: {e}")))?;
                    // Continue looping with reduced remaining time
                },
                Ok(Message::Close(frame)) => return Self::handle_close_frame(frame),
                Ok(_) => {
                    // Continue looping past non-text frames
                },
                Err(tungstenite::Error::Io(e))
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    return Ok(None);
                },
                Err(e) => return Err(WebSocketError::Receive(e.to_string())),
            }
        }
    }

    fn handle_close_frame(
        frame: Option<CloseFrame>,
    ) -> Result<Option<ServerMessage>, WebSocketError> {
        let reason = frame.and_then(|f| {
            serde_json::from_str::<bencher_json::runner::CloseReason>(&f.reason).ok()
        });
        match reason {
            Some(reason) => Err(WebSocketError::Receive(format!(
                "Server closed connection: {reason:?}"
            ))),
            None => Err(WebSocketError::Receive(
                "Server closed connection".to_owned(),
            )),
        }
    }
}

fn get_tcp_stream(stream: &MaybeTlsStream<TcpStream>) -> Result<&TcpStream, WebSocketError> {
    match stream {
        MaybeTlsStream::Plain(s) => Ok(s),
        MaybeTlsStream::Rustls(s) => Ok(s.get_ref()),
        _ => Err(WebSocketError::Connection(
            "Unsupported TLS stream type".to_owned(),
        )),
    }
}

fn set_nonblocking(
    stream: &MaybeTlsStream<TcpStream>,
    nonblocking: bool,
) -> Result<(), WebSocketError> {
    get_tcp_stream(stream)?
        .set_nonblocking(nonblocking)
        .map_err(|e| WebSocketError::Connection(format!("Failed to set nonblocking: {e}")))
}

fn set_read_timeout(
    stream: &MaybeTlsStream<TcpStream>,
    timeout: Option<Duration>,
) -> Result<(), WebSocketError> {
    get_tcp_stream(stream)?
        .set_read_timeout(timeout)
        .map_err(|e| WebSocketError::Connection(format!("Failed to set read timeout: {e}")))
}

#[cfg(test)]
#[expect(clippy::indexing_slicing, reason = "Test assertions on JSON values")]
mod tests {
    use std::collections::BTreeMap;

    use bencher_json::{JobUuid, runner::JsonIterationOutput};
    use camino::Utf8PathBuf;

    use super::*;

    fn test_job_uuid() -> JobUuid {
        "550e8400-e29b-41d4-a716-446655440000".parse().unwrap()
    }

    // --- RunnerMessage serialization ---

    #[test]
    fn running_serializes() {
        let json = serde_json::to_string(&RunnerMessage::Running).unwrap();
        assert_eq!(json, r#"{"event":"running"}"#);
    }

    #[test]
    fn heartbeat_serializes() {
        let json = serde_json::to_string(&RunnerMessage::Heartbeat).unwrap();
        assert_eq!(json, r#"{"event":"heartbeat"}"#);
    }

    #[test]
    fn completed_serializes_with_all_fields() {
        let mut output = BTreeMap::new();
        output.insert(
            Utf8PathBuf::from("/tmp/results.json"),
            "benchmark results here".to_owned(),
        );
        let msg = RunnerMessage::Completed {
            job: test_job_uuid(),
            results: vec![JsonIterationOutput {
                exit_code: 0,
                stdout: Some("stdout output".to_owned()),
                stderr: Some("stderr output".to_owned()),
                output: Some(output),
            }],
        };
        let json: serde_json::Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["event"], "completed");
        assert_eq!(json["job"], test_job_uuid().to_string());
        assert_eq!(json["results"][0]["exit_code"], 0);
        assert_eq!(json["results"][0]["stdout"], "stdout output");
        assert_eq!(json["results"][0]["stderr"], "stderr output");
        assert_eq!(
            json["results"][0]["output"]["/tmp/results.json"],
            "benchmark results here"
        );
    }

    #[test]
    fn completed_serializes_minimal() {
        let msg = RunnerMessage::Completed {
            job: test_job_uuid(),
            results: vec![JsonIterationOutput {
                exit_code: 1,
                stdout: None,
                stderr: None,
                output: None,
            }],
        };
        let json: serde_json::Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["event"], "completed");
        assert_eq!(json["job"], test_job_uuid().to_string());
        assert_eq!(json["results"][0]["exit_code"], 1);
        assert!(json["results"][0].get("stdout").is_none());
        assert!(json["results"][0].get("stderr").is_none());
        assert!(json["results"][0].get("output").is_none());
    }

    #[test]
    fn failed_serializes_with_all_fields() {
        let msg = RunnerMessage::Failed {
            job: test_job_uuid(),
            results: vec![JsonIterationOutput {
                exit_code: 137,
                stdout: Some("partial stdout".to_owned()),
                stderr: Some("error details".to_owned()),
                output: None,
            }],
            error: "OOM killed".to_owned(),
        };
        let json: serde_json::Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["event"], "failed");
        assert_eq!(json["job"], test_job_uuid().to_string());
        assert_eq!(json["results"][0]["exit_code"], 137);
        assert_eq!(json["error"], "OOM killed");
        assert_eq!(json["results"][0]["stdout"], "partial stdout");
        assert_eq!(json["results"][0]["stderr"], "error details");
    }

    #[test]
    fn failed_serializes_minimal() {
        let msg = RunnerMessage::Failed {
            job: test_job_uuid(),
            results: Vec::new(),
            error: "timeout".to_owned(),
        };
        let json: serde_json::Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["event"], "failed");
        assert_eq!(json["job"], test_job_uuid().to_string());
        assert!(json["results"].as_array().unwrap().is_empty());
        assert_eq!(json["error"], "timeout");
    }

    #[test]
    fn canceled_serializes() {
        let msg = RunnerMessage::Canceled {
            job: test_job_uuid(),
        };
        let json: serde_json::Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["event"], "canceled");
        assert_eq!(json["job"], test_job_uuid().to_string());
    }

    // --- ServerMessage deserialization ---

    #[test]
    fn ack_deserializes() {
        let msg: ServerMessage = serde_json::from_str(r#"{"event":"ack"}"#).unwrap();
        assert!(matches!(msg, ServerMessage::Ack { job: None }));
    }

    #[test]
    fn ack_with_job_deserializes() {
        let json = format!(r#"{{"event":"ack","job":"{}"}}"#, test_job_uuid());
        let msg: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(msg, ServerMessage::Ack { job: Some(_) }));
    }

    #[test]
    fn cancel_deserializes() {
        let msg: ServerMessage = serde_json::from_str(r#"{"event":"cancel"}"#).unwrap();
        assert!(matches!(msg, ServerMessage::Cancel));
    }

    #[test]
    fn unknown_event_fails() {
        let result = serde_json::from_str::<ServerMessage>(r#"{"event":"unknown"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn missing_event_field_fails() {
        let result = serde_json::from_str::<ServerMessage>(r#"{"type":"ack"}"#);
        assert!(result.is_err());
    }

    #[test]
    fn empty_json_fails() {
        let result = serde_json::from_str::<ServerMessage>("{}");
        assert!(result.is_err());
    }

    // --- Host header construction ---

    #[test]
    fn host_header_includes_port_when_present() {
        let url: Url = "ws://localhost:8080/channel".parse().unwrap();
        let host = match url.port() {
            Some(port) => format!("{}:{port}", url.host_str().unwrap_or("localhost")),
            None => url.host_str().unwrap_or("localhost").to_owned(),
        };
        assert_eq!(host, "localhost:8080");
    }

    #[test]
    fn host_header_omits_port_when_absent() {
        let url: Url = "ws://example.com/channel".parse().unwrap();
        let host = match url.port() {
            Some(port) => format!("{}:{port}", url.host_str().unwrap_or("localhost")),
            None => url.host_str().unwrap_or("localhost").to_owned(),
        };
        assert_eq!(host, "example.com");
    }
}
