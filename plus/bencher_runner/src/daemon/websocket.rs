use std::net::TcpStream;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tungstenite::handshake::client::generate_key;
use tungstenite::http::Request;
use tungstenite::stream::MaybeTlsStream;
use tungstenite::{Message, WebSocket};
use url::Url;

use super::error::WebSocketError;

#[derive(Debug, Serialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum RunnerMessage {
    Running,
    Heartbeat,
    Completed {
        exit_code: i32,
        output: Option<String>,
    },
    Failed {
        exit_code: Option<i32>,
        error: String,
    },
    Cancelled,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ServerMessage {
    Ack,
    Cancel,
}

pub struct JobChannel {
    ws: WebSocket<MaybeTlsStream<TcpStream>>,
}

impl JobChannel {
    pub fn connect(ws_url: &Url, token: &str) -> Result<Self, WebSocketError> {
        let request = Request::builder()
            .uri(ws_url.as_str())
            .header("Sec-WebSocket-Protocol", format!("bearer.{token}"))
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
        // Restore blocking mode
        let stream = self.ws.get_mut();
        set_nonblocking(stream, false)?;

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
        }
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
mod tests {
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
    fn completed_serializes_with_output() {
        let msg = RunnerMessage::Completed {
            exit_code: 0,
            output: Some("benchmark results here".to_owned()),
        };
        let json: serde_json::Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["event"], "completed");
        assert_eq!(json["exit_code"], 0);
        assert_eq!(json["output"], "benchmark results here");
    }

    #[test]
    fn completed_serializes_with_null_output() {
        let msg = RunnerMessage::Completed {
            exit_code: 1,
            output: None,
        };
        let json: serde_json::Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["event"], "completed");
        assert_eq!(json["exit_code"], 1);
        assert!(json["output"].is_null());
    }

    #[test]
    fn failed_serializes_with_exit_code() {
        let msg = RunnerMessage::Failed {
            exit_code: Some(137),
            error: "OOM killed".to_owned(),
        };
        let json: serde_json::Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["event"], "failed");
        assert_eq!(json["exit_code"], 137);
        assert_eq!(json["error"], "OOM killed");
    }

    #[test]
    fn failed_serializes_with_null_exit_code() {
        let msg = RunnerMessage::Failed {
            exit_code: None,
            error: "timeout".to_owned(),
        };
        let json: serde_json::Value = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["event"], "failed");
        assert!(json["exit_code"].is_null());
        assert_eq!(json["error"], "timeout");
    }

    #[test]
    fn cancelled_serializes() {
        let json = serde_json::to_string(&RunnerMessage::Cancelled).unwrap();
        assert_eq!(json, r#"{"event":"cancelled"}"#);
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
        let result = serde_json::from_str::<ServerMessage>(r#"{}"#);
        assert!(result.is_err());
    }
}
