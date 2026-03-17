use bencher_valid::PollTimeout;
use serde::{Deserialize, Serialize};

use super::job::{JobUuid, JsonClaimedJob, JsonIterationOutput};

/// Messages sent from the runner to the server over the WebSocket channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum RunnerMessage {
    /// Runner is idle, requesting a job.
    Ready {
        /// Maximum time to wait for a job (long-poll), in seconds (1-900)
        #[serde(skip_serializing_if = "Option::is_none")]
        poll_timeout: Option<PollTimeout>,
    },
    /// Job setup complete, benchmark execution starting.
    Running,
    /// Periodic heartbeat, keeps job alive and triggers billing.
    Heartbeat,
    /// Benchmark completed successfully.
    Completed {
        /// The job this result belongs to (enables retry on reconnect)
        job: JobUuid,
        /// Per-iteration results
        results: Vec<JsonIterationOutput>,
    },
    /// Benchmark failed.
    Failed {
        /// The job this result belongs to (enables retry on reconnect)
        job: JobUuid,
        /// Per-iteration results collected before failure
        results: Vec<JsonIterationOutput>,
        /// Error description
        error: String,
    },
    /// Acknowledge cancellation from server.
    Canceled {
        /// The job this cancellation belongs to (enables retry on reconnect)
        job: JobUuid,
    },
}

/// Messages sent from the server to the runner over the WebSocket channel.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "event", rename_all = "snake_case")]
pub enum ServerMessage {
    /// Acknowledge received message.
    Ack {
        /// The job this acknowledgement is for (None for non-job messages like Ready)
        #[serde(skip_serializing_if = "Option::is_none")]
        job: Option<JobUuid>,
    },
    /// Server assigned a job (boxed because it's large).
    Job(Box<JsonClaimedJob>),
    /// Poll timeout expired, no job available.
    NoJob,
    /// Job was canceled, stop execution immediately.
    Cancel,
}

/// Reason for closing a WebSocket connection, sent in the close frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CloseReason {
    /// Job completed successfully (runner sent `Completed`).
    JobCompleted,
    /// Job failed (runner sent `Failed`).
    JobFailed,
    /// Job was canceled (server detected cancellation).
    JobCanceled,
    /// Runner acknowledged cancellation (runner sent `Canceled`).
    JobCanceledByRunner,
    /// No valid protocol message within the heartbeat window.
    HeartbeatTimeout,
    /// Job exceeded its configured timeout + grace period.
    JobTimeoutExceeded,
}

#[cfg(test)]
#[expect(
    clippy::indexing_slicing,
    clippy::wildcard_enum_match_arm,
    reason = "Test assertions on JSON values"
)]
mod tests {
    use std::collections::BTreeMap;

    use camino::Utf8PathBuf;

    use super::*;

    fn test_job_uuid() -> JobUuid {
        "550e8400-e29b-41d4-a716-446655440000".parse().unwrap()
    }

    #[test]
    fn ready_no_timeout_roundtrip() {
        let msg = RunnerMessage::Ready { poll_timeout: None };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"event":"ready"}"#);
        let deserialized: RunnerMessage = serde_json::from_str(&json).unwrap();
        match deserialized {
            RunnerMessage::Ready { poll_timeout } => assert!(poll_timeout.is_none()),
            other => panic!("Expected Ready, got {other:?}"),
        }
    }

    #[test]
    fn ready_with_timeout_roundtrip() {
        let msg = RunnerMessage::Ready {
            poll_timeout: Some(PollTimeout::try_from(30).unwrap()),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: RunnerMessage = serde_json::from_str(&json).unwrap();
        match deserialized {
            RunnerMessage::Ready { poll_timeout } => {
                assert_eq!(u32::from(poll_timeout.unwrap()), 30);
            },
            other => panic!("Expected Ready, got {other:?}"),
        }
    }

    #[test]
    fn server_job_roundtrip() {
        // Build a minimal JsonClaimedJob via JSON
        let job_json = serde_json::json!({
            "uuid": "550e8400-e29b-41d4-a716-446655440000",
            "spec": {
                "uuid": "00000000-0000-0000-0000-000000000001",
                "name": "test-spec",
                "slug": "test-spec",
                "architecture": "x86_64",
                "cpu": 2,
                "memory": 0x4000_0000,
                "disk": 0x2_8000_0000i64,
                "network": false,
                "created": "2025-01-01T00:00:00Z",
                "modified": "2025-01-01T00:00:00Z"
            },
            "config": {
                "registry": "https://registry.bencher.dev",
                "project": "11111111-2222-3333-4444-555555555555",
                "digest": "sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3",
                "timeout": 300
            },
            "oci_token": "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiJ0ZXN0In0.SflKxwRJSMeKKF2QT4fwpMeJf36POk6yJV_adQssw5c",
            "timeout": 300,
            "created": "2025-01-01T00:00:00Z"
        });
        let claimed: JsonClaimedJob = serde_json::from_value(job_json).unwrap();
        let msg = ServerMessage::Job(Box::new(claimed));
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: ServerMessage = serde_json::from_str(&json).unwrap();
        match deserialized {
            ServerMessage::Job(job) => {
                assert_eq!(job.uuid.to_string(), "550e8400-e29b-41d4-a716-446655440000");
            },
            other => panic!("Expected Job, got {other:?}"),
        }
    }

    #[test]
    fn server_no_job_roundtrip() {
        let msg = ServerMessage::NoJob;
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"event":"no_job"}"#);
        let deserialized: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, ServerMessage::NoJob));
    }

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
        let job_uuid = test_job_uuid();
        let msg = RunnerMessage::Completed {
            job: job_uuid,
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
            RunnerMessage::Completed { job, results } => {
                assert_eq!(job, job_uuid);
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
        let job_uuid = test_job_uuid();
        let msg = RunnerMessage::Completed {
            job: job_uuid,
            results: Vec::new(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: RunnerMessage = serde_json::from_str(&json).unwrap();
        match deserialized {
            RunnerMessage::Completed { job, results } => {
                assert_eq!(job, job_uuid);
                assert!(results.is_empty());
            },
            other => panic!("Expected Completed, got {other:?}"),
        }
    }

    #[test]
    fn failed_roundtrip() {
        let job_uuid = test_job_uuid();
        let msg = RunnerMessage::Failed {
            job: job_uuid,
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
            RunnerMessage::Failed {
                job,
                results,
                error,
            } => {
                assert_eq!(job, job_uuid);
                assert_eq!(results.len(), 1);
                assert_eq!(results[0].exit_code, 1);
                assert_eq!(error, "something broke");
            },
            other => panic!("Expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn failed_no_results_roundtrip() {
        let job_uuid = test_job_uuid();
        let msg = RunnerMessage::Failed {
            job: job_uuid,
            results: Vec::new(),
            error: "startup failure".into(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: RunnerMessage = serde_json::from_str(&json).unwrap();
        match deserialized {
            RunnerMessage::Failed {
                job,
                results,
                error,
            } => {
                assert_eq!(job, job_uuid);
                assert!(results.is_empty());
                assert_eq!(error, "startup failure");
            },
            other => panic!("Expected Failed, got {other:?}"),
        }
    }

    #[test]
    fn canceled_roundtrip() {
        let job_uuid = test_job_uuid();
        let msg = RunnerMessage::Canceled { job: job_uuid };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: RunnerMessage = serde_json::from_str(&json).unwrap();
        match deserialized {
            RunnerMessage::Canceled { job } => {
                assert_eq!(job, job_uuid);
            },
            other => panic!("Expected Canceled, got {other:?}"),
        }
    }

    #[test]
    fn server_ack_no_job_roundtrip() {
        let msg = ServerMessage::Ack { job: None };
        let json = serde_json::to_string(&msg).unwrap();
        assert_eq!(json, r#"{"event":"ack"}"#);
        let deserialized: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, ServerMessage::Ack { job: None }));
    }

    #[test]
    fn server_ack_with_job_roundtrip() {
        let job_uuid = test_job_uuid();
        let msg = ServerMessage::Ack {
            job: Some(job_uuid),
        };
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: ServerMessage = serde_json::from_str(&json).unwrap();
        match deserialized {
            ServerMessage::Ack { job } => assert_eq!(job, Some(job_uuid)),
            other => panic!("Expected Ack, got {other:?}"),
        }
    }

    #[test]
    fn server_cancel_roundtrip() {
        let msg = ServerMessage::Cancel;
        let json = serde_json::to_string(&msg).unwrap();
        let deserialized: ServerMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(deserialized, ServerMessage::Cancel));
    }

    #[test]
    fn close_reason_serde_roundtrip() {
        let variants = [
            (CloseReason::JobCompleted, "\"job_completed\""),
            (CloseReason::JobFailed, "\"job_failed\""),
            (CloseReason::JobCanceled, "\"job_canceled\""),
            (
                CloseReason::JobCanceledByRunner,
                "\"job_canceled_by_runner\"",
            ),
            (CloseReason::HeartbeatTimeout, "\"heartbeat_timeout\""),
            (CloseReason::JobTimeoutExceeded, "\"job_timeout_exceeded\""),
        ];
        for (variant, expected_json) in variants {
            let json = serde_json::to_string(&variant).unwrap();
            assert_eq!(json, expected_json, "serialize {variant:?}");
            // RFC 6455 §5.5: control frame payload ≤ 125 bytes.
            // Close frames use 2 bytes for the status code, leaving 123 bytes
            // for the reason string. We serialize CloseReason as JSON into the
            // close frame's reason field, so each variant must fit.
            assert!(
                json.len() <= 123,
                "CloseReason {variant:?} serializes to {len} bytes, exceeding the \
                 123-byte WebSocket close frame reason limit (RFC 6455 §5.5)",
                len = json.len(),
            );
            let deserialized: CloseReason = serde_json::from_str(&json).unwrap();
            assert_eq!(deserialized, variant, "roundtrip {variant:?}");
        }
    }

    #[test]
    fn completed_with_file_output_roundtrip() {
        let job_uuid = test_job_uuid();
        let mut output = BTreeMap::new();
        output.insert(
            Utf8PathBuf::from("/tmp/results.json"),
            r#"{"metric": 42}"#.to_owned(),
        );
        let msg = RunnerMessage::Completed {
            job: job_uuid,
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
            RunnerMessage::Completed { job, results } => {
                assert_eq!(job, job_uuid);
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
