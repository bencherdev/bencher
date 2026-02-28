use serde::{Deserialize, Serialize};

use super::job::JsonIterationOutput;

/// Messages sent from the runner to the server over the WebSocket channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum RunnerMessage {
    /// Job setup complete, benchmark execution starting.
    Running,
    /// Periodic heartbeat, keeps job alive and triggers billing.
    Heartbeat,
    /// Benchmark completed successfully.
    Completed {
        /// Per-iteration results
        results: Vec<JsonIterationOutput>,
    },
    /// Benchmark failed.
    Failed {
        /// Per-iteration results collected before failure
        results: Vec<JsonIterationOutput>,
        /// Error description
        error: String,
    },
    /// Acknowledge cancellation from server.
    Canceled,
}

/// Messages sent from the server to the runner over the WebSocket channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Acknowledge received message.
    Ack,
    /// Job was canceled, stop execution immediately.
    Cancel,
}

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    clippy::panic,
    clippy::wildcard_enum_match_arm,
    reason = "Test assertions on JSON values"
)]
mod tests {
    use std::collections::BTreeMap;

    use camino::Utf8PathBuf;

    use super::*;

    #[test]
    fn running_roundtrip() {
        let msg = RunnerMessage::Running;
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: RunnerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, RunnerMessage::Running));
    }

    #[test]
    fn heartbeat_roundtrip() {
        let msg = RunnerMessage::Heartbeat;
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: RunnerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, RunnerMessage::Heartbeat));
    }

    #[test]
    fn completed_roundtrip() {
        let msg = RunnerMessage::Completed {
            results: vec![JsonIterationOutput {
                exit_code: 0,
                stdout: Some("hello".into()),
                stderr: Some("world".into()),
                output: None,
            }],
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: RunnerMessage = serde_json::from_str(&json).unwrap();
        match deserialized {
            RunnerMessage::Completed { results } => {
                assert_eq!(results.len(), 1);
                assert_eq!(results[0].exit_code, 0);
                assert_eq!(results[0].stdout.as_deref(), Some("hello"));
                assert_eq!(results[0].stderr.as_deref(), Some("world"));
            },
            other => panic!("Expected Completed, got {other:?}"),
        }
    }

    #[test]
    fn completed_minimal_roundtrip() {
        let msg = RunnerMessage::Completed {
            results: Vec::new(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: RunnerMessage = serde_json::from_str(&json).unwrap();
        match deserialized {
            RunnerMessage::Completed { results } => {
                assert!(results.is_empty());
            },
            other => panic!("Expected Completed, got {other:?}"),
        }
    }

    #[test]
    fn failed_roundtrip() {
        let msg = RunnerMessage::Failed {
            results: vec![JsonIterationOutput {
                exit_code: 1,
                stdout: Some("out".into()),
                stderr: Some("err".into()),
                output: None,
            }],
            error: "something broke".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: RunnerMessage = serde_json::from_str(&json).unwrap();
        match deserialized {
            RunnerMessage::Failed { results, error } => {
                assert_eq!(results.len(), 1);
                assert_eq!(results[0].exit_code, 1);
                assert_eq!(error, "something broke");
            },
            other => panic!("Expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn failed_no_results_roundtrip() {
        let msg = RunnerMessage::Failed {
            results: Vec::new(),
            error: "startup failure".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: RunnerMessage = serde_json::from_str(&json).unwrap();
        match deserialized {
            RunnerMessage::Failed { results, error } => {
                assert!(results.is_empty());
                assert_eq!(error, "startup failure");
            },
            other => panic!("Expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn canceled_roundtrip() {
        let msg = RunnerMessage::Canceled;
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: RunnerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, RunnerMessage::Canceled));
    }

    #[test]
    fn server_ack_roundtrip() {
        let msg = ServerMessage::Ack;
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, ServerMessage::Ack));
    }

    #[test]
    fn server_cancel_roundtrip() {
        let msg = ServerMessage::Cancel;
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, ServerMessage::Cancel));
    }

    #[test]
    fn completed_with_file_output_roundtrip() {
        let mut output = BTreeMap::new();
        output.insert(
            Utf8PathBuf::from("/tmp/results.json"),
            r#"{"metric": 42}"#.to_owned(),
        );
        let msg = RunnerMessage::Completed {
            results: vec![JsonIterationOutput {
                exit_code: 0,
                stdout: None,
                stderr: None,
                output: Some(output),
            }],
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: RunnerMessage = serde_json::from_str(&json).unwrap();
        match deserialized {
            RunnerMessage::Completed { results } => {
                assert_eq!(results.len(), 1);
                let output = results[0].output.as_ref().unwrap();
                assert_eq!(
                    output.get(Utf8PathBuf::from("/tmp/results.json").as_path()),
                    Some(&r#"{"metric": 42}"#.to_owned())
                );
            },
            other => panic!("Expected Completed, got {other:?}"),
        }
    }
}
