use std::net::TcpStream;
use std::time::Duration;

use bencher_json::runner::{RunnerMessage, ServerMessage};
use tungstenite::handshake::client::generate_key;
use tungstenite::http::Request;
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
            .header("Host", ws_url.host_str().unwrap_or("localhost"))
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
            Ok(Message::Close(_)) => Err(WebSocketError::Receive(
                "Server closed connection".to_owned(),
            )),
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
        let stream = self.ws.get_mut();
        set_read_timeout(stream, Some(timeout))?;
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
                Ok(Some(msg))
            },
            Ok(Message::Ping(data)) => {
                self.ws
                    .send(Message::Pong(data))
                    .map_err(|e| WebSocketError::Send(format!("Failed to send pong: {e}")))?;
                Ok(None)
            },
            Ok(_) => Ok(None),
            Err(tungstenite::Error::Io(e))
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                Ok(None)
            },
            Err(e) => Err(WebSocketError::Receive(e.to_string())),
        }
    }

    pub fn close(&mut self) {
        drop(self.ws.close(None));
        // Drain remaining messages until close is acknowledged
        loop {
            match self.ws.read() {
                Ok(Message::Close(_)) | Err(_) => break,
                Ok(_) => {},
            }
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

    use bencher_json::runner::JsonIterationOutput;
    use camino::Utf8PathBuf;

    use super::*;

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
            results: vec![JsonIterationOutput {
                exit_code: 0,
                stdout: Some("stdout output".to_owned()),
                stderr: Some("stderr output".to_owned()),
                output: Some(output),
            }],
        };
        let json: serde_json::Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["event"], "completed");
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
            results: vec![JsonIterationOutput {
                exit_code: 1,
                stdout: None,
                stderr: None,
                output: None,
            }],
        };
        let json: serde_json::Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["event"], "completed");
        assert_eq!(json["results"][0]["exit_code"], 1);
        assert!(json["results"][0].get("stdout").is_none());
        assert!(json["results"][0].get("stderr").is_none());
        assert!(json["results"][0].get("output").is_none());
    }

    #[test]
    fn failed_serializes_with_all_fields() {
        let msg = RunnerMessage::Failed {
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
        assert_eq!(json["results"][0]["exit_code"], 137);
        assert_eq!(json["error"], "OOM killed");
        assert_eq!(json["results"][0]["stdout"], "partial stdout");
        assert_eq!(json["results"][0]["stderr"], "error details");
    }

    #[test]
    fn failed_serializes_minimal() {
        let msg = RunnerMessage::Failed {
            results: Vec::new(),
            error: "timeout".to_owned(),
        };
        let json: serde_json::Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["event"], "failed");
        assert!(json["results"].as_array().unwrap().is_empty());
        assert_eq!(json["error"], "timeout");
    }

    #[test]
    fn canceled_serializes() {
        let json = serde_json::to_string(&RunnerMessage::Canceled).unwrap();
        assert_eq!(json, r#"{"event":"canceled"}"#);
    }

    // --- ServerMessage deserialization ---

    #[test]
    fn ack_deserializes() {
        let msg: ServerMessage = serde_json::from_str(r#"{"event":"ack"}"#).unwrap();
        assert!(matches!(msg, ServerMessage::Ack));
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
}
