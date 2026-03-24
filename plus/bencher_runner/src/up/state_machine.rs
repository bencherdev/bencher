//! Sans-IO state machine for the runner WebSocket channel protocol.
//!
//! This module contains pure decision logic with no I/O. The driver in `mod.rs`
//! executes effects and feeds inputs, keeping all I/O at the boundary.
//!
//! Not gated behind `cfg(target_os = "linux")` — tests run on any platform.

use std::time::Duration;

use bencher_json::{
    JobUuid, JsonClaimedJob,
    runner::{JsonIterationOutput, RunnerMessage, ServerMessage},
};

/// Margin added to `poll_timeout` for the WS read timeout, giving the server
/// time to send `NoJob` after its own deadline.
const POLL_TIMEOUT_MARGIN_SECS: u64 = 30;

/// Maximum number of times to retry sending a pending result before dropping it.
const MAX_PENDING_RESULT_RETRIES: u32 = 3;

/// Timeout for waiting for server ACK after sending terminal messages.
const ACK_TIMEOUT: Duration = Duration::from_secs(5);

// --- Public types ---

/// Channel protocol state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChannelState {
    /// Not connected. May have a pending result to retry.
    Disconnected,
    /// Pending result sent, waiting for server ACK.
    AwaitingPendingAck,
    /// `Ready` sent, waiting for `Job` or `NoJob`.
    AwaitingJob,
    /// Job executing. Heartbeat thread is active.
    Executing { job_uuid: JobUuid },
    /// Terminal message sent, waiting for server ACK.
    AwaitingTerminalAck {
        job_uuid: JobUuid,
        kind: TerminalKind,
    },
    /// Clean shutdown.
    ShutDown,
}

/// What kind of terminal message was sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalKind {
    Completed {
        exit_code: i32,
        output: Option<String>,
    },
    Failed {
        error: String,
    },
    Canceled,
}

/// Input events from the driver (I/O results).
#[derive(Debug)]
pub enum Input {
    /// WebSocket connected successfully.
    Connected,
    /// Connection attempt or send failed.
    ConnectionFailed,
    /// Server message received.
    Message(ServerMessage),
    /// Timed out waiting for server response.
    ReceiveTimeout,
    /// Job execution finished with this outcome.
    JobFinished(JobFinishResult),
    /// External shutdown signal.
    Shutdown,
}

/// Result of job execution, produced by the driver.
#[derive(Debug)]
pub enum JobFinishResult {
    Completed {
        exit_code: i32,
        output: Option<String>,
        results: Vec<JsonIterationOutput>,
    },
    Failed {
        error: String,
        results: Vec<JsonIterationOutput>,
    },
    Canceled,
}

/// Why the state machine is requesting a reconnection.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReconnectReason {
    /// Initial connection or reconnection attempt failed.
    ConnectFailed,
    /// Connection lost while polling for jobs.
    PollingConnectionLost,
    /// Timed out waiting for server response while polling.
    PollingTimeout,
    /// Connection lost while awaiting pending result ACK.
    PendingAckConnectionLost,
    /// Timed out waiting for pending result ACK.
    PendingAckTimeout,
    /// Unexpected message while awaiting pending result ACK.
    PendingAckUnexpectedMessage,
    /// Connection lost during job execution.
    ExecutingConnectionLost,
    /// Connection lost while awaiting terminal ACK.
    TerminalAckConnectionLost,
}

impl std::fmt::Display for ReconnectReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ConnectFailed => write!(f, "connect failed"),
            Self::PollingConnectionLost => {
                write!(f, "connection lost while polling for jobs")
            },
            Self::PollingTimeout => write!(f, "timed out polling for jobs"),
            Self::PendingAckConnectionLost => {
                write!(f, "connection lost while awaiting pending ACK")
            },
            Self::PendingAckTimeout => write!(f, "timed out awaiting pending ACK"),
            Self::PendingAckUnexpectedMessage => {
                write!(f, "unexpected message while awaiting pending ACK")
            },
            Self::ExecutingConnectionLost => {
                write!(f, "connection lost during execution")
            },
            Self::TerminalAckConnectionLost => {
                write!(f, "connection lost while awaiting terminal ACK")
            },
        }
    }
}

/// Effects for the driver to execute.
#[derive(Debug)]
pub enum Effect {
    /// Establish WebSocket connection.
    Connect,
    /// Send a runner message over the WebSocket.
    Send(RunnerMessage),
    /// Wait for a server message with timeout (for ACK).
    Receive(Duration),
    /// Wait for Job/NoJob from server (poll timeout + margin).
    WaitForJob(Duration),
    /// Execute a benchmark job. Driver feeds `Input::JobFinished` when done.
    ExecuteJob(Box<JsonClaimedJob>),
    /// Sleep before reconnect (with jitter).
    SleepBeforeReconnect(ReconnectReason),
    /// Close the WebSocket.
    Close,
    /// Report job outcome to the log/caller.
    ReportOutcome(JobOutcome),
    /// Log a message.
    Log(LogLevel, String),
    /// Exit the protocol loop.
    Exit,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogLevel {
    Info,
    Warn,
    Error,
}

/// Reported when a job's terminal message flow completes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobOutcome {
    pub job: JobUuid,
    pub kind: TerminalKind,
    pub acked: bool,
}

// --- State machine ---

pub struct ChannelStateMachine {
    state: ChannelState,
    poll_timeout_secs: u32,
    // Retry lifecycle for unACKed terminal messages:
    //
    // 1. Job finishes → build terminal message, set `in_flight`, send it
    // 2. ACK received with matching UUID → clear `in_flight`, done
    // 3. ACK fails (timeout, UUID mismatch, wrong message, connection lost):
    //    → move `in_flight` to `pending_result`, increment `pending_retry_count`
    // 4. On reconnect, `resolve_idle()` moves `pending_result` back to
    //    `in_flight` and resends it
    // 5. After MAX_PENDING_RESULT_RETRIES failures, drop the message
    /// Terminal message awaiting retry after a failed ACK attempt.
    pending_result: Option<RunnerMessage>,
    pending_retry_count: u32,
    /// Message currently in flight (sent, waiting for ACK).
    in_flight: Option<RunnerMessage>,
}

impl ChannelStateMachine {
    pub fn new(poll_timeout_secs: u32) -> Self {
        Self {
            state: ChannelState::Disconnected,
            poll_timeout_secs,
            pending_result: None,
            pending_retry_count: 0,
            in_flight: None,
        }
    }

    #[cfg(test)]
    pub fn state(&self) -> &ChannelState {
        &self.state
    }

    /// Returns the initial effects to start the protocol loop.
    pub fn initial_effects() -> Vec<Effect> {
        vec![Effect::Connect]
    }

    /// Process an input event. Returns effects for the driver to execute.
    pub fn step(&mut self, input: Input) -> Vec<Effect> {
        // Take ownership of state to avoid borrow checker issues with &mut self
        let state = std::mem::replace(&mut self.state, ChannelState::Disconnected);

        // ShutDown absorbs all inputs
        if state == ChannelState::ShutDown {
            self.state = ChannelState::ShutDown;
            return vec![];
        }

        // Shutdown from any state
        if matches!(input, Input::Shutdown) {
            self.state = ChannelState::ShutDown;
            return vec![Effect::Close, Effect::Exit];
        }

        match state {
            ChannelState::Disconnected => self.handle_disconnected(&input),
            ChannelState::AwaitingPendingAck => self.handle_awaiting_pending_ack(input),
            ChannelState::AwaitingJob => self.handle_awaiting_job(input),
            ChannelState::Executing { job_uuid } => self.handle_executing(job_uuid, input),
            ChannelState::AwaitingTerminalAck { job_uuid, kind } => {
                self.handle_awaiting_terminal_ack(job_uuid, kind, input)
            },
            // ShutDown is handled above; this arm is unreachable.
            ChannelState::ShutDown => vec![],
        }
    }

    // --- Per-state handlers ---

    fn handle_disconnected(&mut self, input: &Input) -> Vec<Effect> {
        match input {
            Input::Connected => {
                let mut effects = vec![Effect::Log(
                    LogLevel::Info,
                    "Channel connected. Polling for jobs...".to_owned(),
                )];
                effects.extend(self.resolve_idle());
                effects
            },
            Input::ConnectionFailed => {
                self.state = ChannelState::Disconnected;
                vec![
                    Effect::SleepBeforeReconnect(ReconnectReason::ConnectFailed),
                    Effect::Connect,
                ]
            },
            input @ (Input::Message(_)
            | Input::ReceiveTimeout
            | Input::JobFinished(_)
            | Input::Shutdown) => self.unexpected(ChannelState::Disconnected, input),
        }
    }

    fn handle_awaiting_pending_ack(&mut self, input: Input) -> Vec<Effect> {
        match input {
            Input::Message(ServerMessage::Ack { .. }) => {
                self.pending_retry_count = 0;
                self.in_flight = None;
                let mut effects = vec![Effect::Log(
                    LogLevel::Info,
                    "Pending result ACKed by server".to_owned(),
                )];
                effects.extend(self.resolve_idle());
                effects
            },
            // Retry ACK timed out — connection is likely dead. Close and reconnect.
            // The pending message is preserved for retry after reconnection.
            Input::ReceiveTimeout => {
                self.pending_result = self.in_flight.take();
                self.state = ChannelState::Disconnected;
                vec![
                    Effect::Log(LogLevel::Warn, "Retry ACK timed out".to_owned()),
                    Effect::Close,
                    Effect::SleepBeforeReconnect(ReconnectReason::PendingAckTimeout),
                    Effect::Connect,
                ]
            },
            Input::Message(other) => {
                self.pending_result = self.in_flight.take();
                self.state = ChannelState::Disconnected;
                vec![
                    Effect::Log(
                        LogLevel::Warn,
                        format!("Expected ACK for pending result, got {other:?}"),
                    ),
                    Effect::Close,
                    Effect::SleepBeforeReconnect(ReconnectReason::PendingAckUnexpectedMessage),
                    Effect::Connect,
                ]
            },
            Input::ConnectionFailed => {
                self.pending_result = self.in_flight.take();
                self.state = ChannelState::Disconnected;
                vec![
                    Effect::SleepBeforeReconnect(ReconnectReason::PendingAckConnectionLost),
                    Effect::Connect,
                ]
            },
            input @ (Input::Connected | Input::JobFinished(_) | Input::Shutdown) => {
                self.unexpected(ChannelState::AwaitingPendingAck, &input)
            },
        }
    }

    fn handle_awaiting_job(&mut self, input: Input) -> Vec<Effect> {
        match input {
            Input::Message(ServerMessage::Job(job)) => {
                let job_uuid = job.uuid;
                self.state = ChannelState::Executing { job_uuid };
                vec![
                    Effect::Log(LogLevel::Info, format!("Received job: {job_uuid}")),
                    Effect::Send(RunnerMessage::Running),
                    Effect::ExecuteJob(job),
                ]
            },
            Input::Message(ServerMessage::NoJob) => self.resolve_idle(),
            Input::ReceiveTimeout => {
                self.state = ChannelState::Disconnected;
                vec![
                    Effect::Close,
                    Effect::SleepBeforeReconnect(ReconnectReason::PollingTimeout),
                    Effect::Connect,
                ]
            },
            Input::ConnectionFailed => {
                self.state = ChannelState::Disconnected;
                vec![
                    Effect::SleepBeforeReconnect(ReconnectReason::PollingConnectionLost),
                    Effect::Connect,
                ]
            },
            input @ (Input::Connected
            | Input::Message(ServerMessage::Ack { .. } | ServerMessage::Cancel)
            | Input::JobFinished(_)
            | Input::Shutdown) => self.unexpected(ChannelState::AwaitingJob, &input),
        }
    }

    fn handle_executing(&mut self, job_uuid: JobUuid, input: Input) -> Vec<Effect> {
        match input {
            Input::JobFinished(result) => {
                let (msg, kind) = build_terminal_message(job_uuid, result);
                self.in_flight = Some(msg.clone());
                self.state = ChannelState::AwaitingTerminalAck { job_uuid, kind };
                vec![Effect::Send(msg), Effect::Receive(ACK_TIMEOUT)]
            },
            Input::ConnectionFailed => {
                self.state = ChannelState::Disconnected;
                vec![
                    Effect::SleepBeforeReconnect(ReconnectReason::ExecutingConnectionLost),
                    Effect::Connect,
                ]
            },
            input @ (Input::Connected
            | Input::Message(_)
            | Input::ReceiveTimeout
            | Input::Shutdown) => self.unexpected(ChannelState::Executing { job_uuid }, &input),
        }
    }

    fn handle_awaiting_terminal_ack(
        &mut self,
        job_uuid: JobUuid,
        kind: TerminalKind,
        input: Input,
    ) -> Vec<Effect> {
        match input {
            Input::Message(ServerMessage::Ack { job: ack_job }) => {
                let acked = ack_job.as_ref() == Some(&job_uuid);
                let mut effects = Vec::new();
                if acked {
                    self.in_flight = None;
                } else {
                    self.pending_result = self.in_flight.take();
                    effects.push(Effect::Log(
                        LogLevel::Warn,
                        format!("ACK job UUID mismatch: expected {job_uuid}, got {ack_job:?}"),
                    ));
                }
                effects.push(Effect::ReportOutcome(JobOutcome {
                    job: job_uuid,
                    kind,
                    acked,
                }));
                effects.extend(self.resolve_idle());
                effects
            },
            // First ACK attempt timed out — retry on the same connection via
            // resolve_idle(). If the connection is actually dead, the retry send
            // will fail and produce ConnectionFailed, triggering a reconnect.
            Input::ReceiveTimeout => {
                self.pending_result = self.in_flight.take();
                let mut effects = vec![Effect::ReportOutcome(JobOutcome {
                    job: job_uuid,
                    kind,
                    acked: false,
                })];
                effects.extend(self.resolve_idle());
                effects
            },
            Input::Message(other) => {
                self.pending_result = self.in_flight.take();
                let mut effects = vec![
                    Effect::Log(LogLevel::Warn, format!("Expected ACK, got {other:?}")),
                    Effect::ReportOutcome(JobOutcome {
                        job: job_uuid,
                        kind,
                        acked: false,
                    }),
                ];
                effects.extend(self.resolve_idle());
                effects
            },
            Input::ConnectionFailed => {
                self.pending_result = self.in_flight.take();
                self.state = ChannelState::Disconnected;
                vec![
                    Effect::ReportOutcome(JobOutcome {
                        job: job_uuid,
                        kind,
                        acked: false,
                    }),
                    Effect::SleepBeforeReconnect(ReconnectReason::TerminalAckConnectionLost),
                    Effect::Connect,
                ]
            },
            input @ (Input::Connected | Input::JobFinished(_) | Input::Shutdown) => {
                self.unexpected(ChannelState::AwaitingTerminalAck { job_uuid, kind }, &input)
            },
        }
    }

    // --- Helpers ---

    /// Log an unexpected (state, input) combination and restore the state.
    fn unexpected(&mut self, state: ChannelState, input: &Input) -> Vec<Effect> {
        let msg = format!("Unexpected input {input:?} in state {state:?}");
        self.state = state;
        vec![Effect::Log(LogLevel::Error, msg)]
    }

    /// Resolve the idle decision point: if there's a pending result to retry,
    /// send it; otherwise send Ready and wait for a job.
    fn resolve_idle(&mut self) -> Vec<Effect> {
        if let Some(pending) = self.pending_result.take() {
            self.pending_retry_count += 1;
            if self.pending_retry_count > MAX_PENDING_RESULT_RETRIES {
                self.pending_retry_count = 0;
                let mut effects = vec![Effect::Log(
                    LogLevel::Error,
                    format!(
                        "Exceeded {MAX_PENDING_RESULT_RETRIES} retries for pending result, dropping message"
                    ),
                )];
                // Recurse: no pending now, will send Ready
                effects.extend(self.resolve_idle());
                return effects;
            }
            self.in_flight = Some(pending.clone());
            self.state = ChannelState::AwaitingPendingAck;
            vec![
                Effect::Log(
                    LogLevel::Info,
                    format!(
                        "Resending unACKed terminal message (attempt {}/{MAX_PENDING_RESULT_RETRIES})...",
                        self.pending_retry_count,
                    ),
                ),
                Effect::Send(pending),
                Effect::Receive(ACK_TIMEOUT),
            ]
        } else {
            let poll_timeout = bencher_valid::PollTimeout::try_from(self.poll_timeout_secs).ok();
            self.state = ChannelState::AwaitingJob;
            let wait_duration =
                Duration::from_secs(u64::from(self.poll_timeout_secs) + POLL_TIMEOUT_MARGIN_SECS);
            vec![
                Effect::Send(RunnerMessage::Ready { poll_timeout }),
                Effect::WaitForJob(wait_duration),
            ]
        }
    }

    #[cfg(test)]
    fn with_state(mut self, state: ChannelState) -> Self {
        self.state = state;
        self
    }

    #[cfg(test)]
    fn with_pending(mut self, msg: RunnerMessage, retry_count: u32) -> Self {
        self.pending_result = Some(msg);
        self.pending_retry_count = retry_count;
        self
    }

    #[cfg(test)]
    fn with_in_flight(mut self, msg: RunnerMessage) -> Self {
        self.in_flight = Some(msg);
        self
    }
}

/// Build the terminal [`RunnerMessage`] and [`TerminalKind`] from a job finish
/// result.
fn build_terminal_message(
    job_uuid: JobUuid,
    result: JobFinishResult,
) -> (RunnerMessage, TerminalKind) {
    match result {
        JobFinishResult::Completed {
            exit_code,
            output,
            results,
        } => {
            let msg = RunnerMessage::Completed {
                job: job_uuid,
                results,
            };
            let kind = TerminalKind::Completed { exit_code, output };
            (msg, kind)
        },
        JobFinishResult::Failed { error, results } => {
            let msg = RunnerMessage::Failed {
                job: job_uuid,
                results,
                error: error.clone(),
            };
            let kind = TerminalKind::Failed { error };
            (msg, kind)
        },
        JobFinishResult::Canceled => {
            let msg = RunnerMessage::Canceled { job: job_uuid };
            let kind = TerminalKind::Canceled;
            (msg, kind)
        },
    }
}

#[cfg(test)]
#[expect(clippy::indexing_slicing)]
mod tests {
    use super::*;

    fn test_job_uuid() -> JobUuid {
        "550e8400-e29b-41d4-a716-446655440000".parse().unwrap()
    }

    fn other_job_uuid() -> JobUuid {
        "660e8400-e29b-41d4-a716-446655440000".parse().unwrap()
    }

    fn test_sm() -> ChannelStateMachine {
        ChannelStateMachine::new(30)
    }

    fn test_completed_msg() -> RunnerMessage {
        RunnerMessage::Completed {
            job: test_job_uuid(),
            results: vec![],
        }
    }

    fn test_claimed_job() -> Box<JsonClaimedJob> {
        let json = serde_json::json!({
            "uuid": "550e8400-e29b-41d4-a716-446655440000",
            "spec": {
                "uuid": "00000000-0000-0000-0000-000000000001",
                "name": "test-spec",
                "slug": "test-spec",
                "os": "linux",
                "architecture": "x86_64",
                "cpu": 2,
                "memory": 0x4000_0000u64,
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
        Box::new(serde_json::from_value(json).unwrap())
    }

    // --- Connection ---

    #[test]
    fn initial_effects_emit_connect() {
        let effects = ChannelStateMachine::initial_effects();
        assert_eq!(effects.len(), 1);
        assert!(matches!(effects[0], Effect::Connect));
    }

    #[test]
    fn connected_without_pending_sends_ready() {
        let mut sm = test_sm();
        let effects = sm.step(Input::Connected);
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::Send(RunnerMessage::Ready { .. })))
        );
        assert!(effects.iter().any(|e| matches!(e, Effect::WaitForJob(_))));
        assert_eq!(*sm.state(), ChannelState::AwaitingJob);
    }

    #[test]
    fn connected_with_pending_sends_pending_first() {
        let mut sm = test_sm().with_pending(test_completed_msg(), 0);
        let effects = sm.step(Input::Connected);
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::Send(RunnerMessage::Completed { .. })))
        );
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, Effect::Send(RunnerMessage::Ready { .. })))
        );
        assert_eq!(*sm.state(), ChannelState::AwaitingPendingAck);
    }

    #[test]
    fn connection_failure_triggers_reconnect() {
        let mut sm = test_sm();
        let effects = sm.step(Input::ConnectionFailed);
        assert!(effects.iter().any(|e| matches!(
            e,
            Effect::SleepBeforeReconnect(ReconnectReason::ConnectFailed)
        )));
        assert!(effects.iter().any(|e| matches!(e, Effect::Connect)));
        assert_eq!(*sm.state(), ChannelState::Disconnected);
    }

    // --- AwaitingPendingAck ---

    #[test]
    fn pending_retry_count_resets_on_ack() {
        let mut sm = test_sm()
            .with_state(ChannelState::AwaitingPendingAck)
            .with_in_flight(test_completed_msg());
        sm.pending_retry_count = 2;

        let effects = sm.step(Input::Message(ServerMessage::Ack { job: None }));
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::Log(LogLevel::Info, s) if s.contains("ACKed")))
        );
        assert_eq!(sm.pending_retry_count, 0);
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::Send(RunnerMessage::Ready { .. })))
        );
    }

    #[test]
    fn receive_timeout_during_pending_ack_reconnects() {
        let mut sm = test_sm()
            .with_state(ChannelState::AwaitingPendingAck)
            .with_in_flight(test_completed_msg());

        let effects = sm.step(Input::ReceiveTimeout);
        assert!(effects.iter().any(|e| matches!(e, Effect::Close)));
        assert!(effects.iter().any(|e| matches!(
            e,
            Effect::SleepBeforeReconnect(ReconnectReason::PendingAckTimeout)
        )));
        assert_eq!(*sm.state(), ChannelState::Disconnected);
        assert!(sm.pending_result.is_some());
    }

    #[test]
    fn pending_dropped_after_max_retries() {
        let mut sm = test_sm().with_pending(test_completed_msg(), MAX_PENDING_RESULT_RETRIES);

        let effects = sm.step(Input::Connected);
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::Log(LogLevel::Error, s) if s.contains("dropping")))
        );
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::Send(RunnerMessage::Ready { .. })))
        );
        assert_eq!(*sm.state(), ChannelState::AwaitingJob);
        assert!(sm.pending_result.is_none());
        assert_eq!(sm.pending_retry_count, 0);
    }

    // --- AwaitingJob ---

    #[test]
    fn job_received_starts_execution() {
        let mut sm = test_sm().with_state(ChannelState::AwaitingJob);
        let job = test_claimed_job();
        let job_uuid = job.uuid;

        let effects = sm.step(Input::Message(ServerMessage::Job(job)));
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::Send(RunnerMessage::Running)))
        );
        assert!(effects.iter().any(|e| matches!(e, Effect::ExecuteJob(_))));
        assert_eq!(*sm.state(), ChannelState::Executing { job_uuid });
    }

    #[test]
    fn no_job_returns_to_awaiting_job() {
        let mut sm = test_sm().with_state(ChannelState::AwaitingJob);
        let effects = sm.step(Input::Message(ServerMessage::NoJob));
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::Send(RunnerMessage::Ready { .. })))
        );
        assert!(effects.iter().any(|e| matches!(e, Effect::WaitForJob(_))));
        assert_eq!(*sm.state(), ChannelState::AwaitingJob);
    }

    #[test]
    fn awaiting_job_timeout_reconnects() {
        let mut sm = test_sm().with_state(ChannelState::AwaitingJob);
        let effects = sm.step(Input::ReceiveTimeout);
        assert!(effects.iter().any(|e| matches!(e, Effect::Close)));
        assert!(effects.iter().any(|e| matches!(
            e,
            Effect::SleepBeforeReconnect(ReconnectReason::PollingTimeout)
        )));
        assert_eq!(*sm.state(), ChannelState::Disconnected);
    }

    #[test]
    fn awaiting_job_connection_failed_reconnects() {
        let mut sm = test_sm().with_state(ChannelState::AwaitingJob);
        let effects = sm.step(Input::ConnectionFailed);
        assert!(effects.iter().any(|e| matches!(
            e,
            Effect::SleepBeforeReconnect(ReconnectReason::PollingConnectionLost)
        )));
        assert_eq!(*sm.state(), ChannelState::Disconnected);
    }

    // --- Executing ---

    #[test]
    fn job_completed_sends_terminal_and_awaits_ack() {
        let job_uuid = test_job_uuid();
        let mut sm = test_sm().with_state(ChannelState::Executing { job_uuid });

        let effects = sm.step(Input::JobFinished(JobFinishResult::Completed {
            exit_code: 0,
            output: Some("hello".to_owned()),
            results: vec![],
        }));
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::Send(RunnerMessage::Completed { .. })))
        );
        assert!(effects.iter().any(|e| matches!(e, Effect::Receive(_))));
        assert!(matches!(
            sm.state(),
            ChannelState::AwaitingTerminalAck {
                kind: TerminalKind::Completed { exit_code: 0, .. },
                ..
            }
        ));
        assert!(sm.in_flight.is_some());
    }

    #[test]
    fn job_failed_sends_terminal_and_awaits_ack() {
        let job_uuid = test_job_uuid();
        let mut sm = test_sm().with_state(ChannelState::Executing { job_uuid });

        let effects = sm.step(Input::JobFinished(JobFinishResult::Failed {
            error: "oom".to_owned(),
            results: vec![],
        }));
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::Send(RunnerMessage::Failed { .. })))
        );
        assert!(matches!(
            sm.state(),
            ChannelState::AwaitingTerminalAck {
                kind: TerminalKind::Failed { .. },
                ..
            }
        ));
    }

    #[test]
    fn job_canceled_sends_terminal_and_awaits_ack() {
        let job_uuid = test_job_uuid();
        let mut sm = test_sm().with_state(ChannelState::Executing { job_uuid });

        let effects = sm.step(Input::JobFinished(JobFinishResult::Canceled));
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::Send(RunnerMessage::Canceled { .. })))
        );
        assert!(matches!(
            sm.state(),
            ChannelState::AwaitingTerminalAck {
                kind: TerminalKind::Canceled,
                ..
            }
        ));
    }

    #[test]
    fn executing_connection_failed_reconnects() {
        let mut sm = test_sm().with_state(ChannelState::Executing {
            job_uuid: test_job_uuid(),
        });
        let effects = sm.step(Input::ConnectionFailed);
        assert!(effects.iter().any(|e| matches!(
            e,
            Effect::SleepBeforeReconnect(ReconnectReason::ExecutingConnectionLost)
        )));
        assert_eq!(*sm.state(), ChannelState::Disconnected);
    }

    // --- AwaitingTerminalAck ---

    #[test]
    fn ack_uuid_match_marks_acked() {
        let job_uuid = test_job_uuid();
        let mut sm = test_sm()
            .with_state(ChannelState::AwaitingTerminalAck {
                job_uuid,
                kind: TerminalKind::Completed {
                    exit_code: 0,
                    output: None,
                },
            })
            .with_in_flight(test_completed_msg());

        let effects = sm.step(Input::Message(ServerMessage::Ack {
            job: Some(job_uuid),
        }));
        let outcome = effects
            .iter()
            .find_map(|e| {
                if let Effect::ReportOutcome(o) = e {
                    Some(o)
                } else {
                    None
                }
            })
            .expect("should have ReportOutcome");
        assert!(outcome.acked);
        assert!(sm.pending_result.is_none());
        assert!(sm.in_flight.is_none());
    }

    #[test]
    fn ack_uuid_mismatch_marks_not_acked() {
        let job_uuid = test_job_uuid();
        let mut sm = test_sm()
            .with_state(ChannelState::AwaitingTerminalAck {
                job_uuid,
                kind: TerminalKind::Completed {
                    exit_code: 0,
                    output: None,
                },
            })
            .with_in_flight(test_completed_msg());

        let effects = sm.step(Input::Message(ServerMessage::Ack {
            job: Some(other_job_uuid()),
        }));
        let outcome = effects
            .iter()
            .find_map(|e| {
                if let Effect::ReportOutcome(o) = e {
                    Some(o)
                } else {
                    None
                }
            })
            .expect("should have ReportOutcome");
        assert!(!outcome.acked);
        // resolve_idle consumed pending_result into in_flight for retry
        assert!(sm.in_flight.is_some());
    }

    #[test]
    fn receive_timeout_during_terminal_ack_saves_pending() {
        let job_uuid = test_job_uuid();
        let mut sm = test_sm()
            .with_state(ChannelState::AwaitingTerminalAck {
                job_uuid,
                kind: TerminalKind::Completed {
                    exit_code: 0,
                    output: None,
                },
            })
            .with_in_flight(test_completed_msg());

        let effects = sm.step(Input::ReceiveTimeout);
        let outcome = effects
            .iter()
            .find_map(|e| {
                if let Effect::ReportOutcome(o) = e {
                    Some(o)
                } else {
                    None
                }
            })
            .expect("should have ReportOutcome");
        assert!(!outcome.acked);
        // resolve_idle picked up the pending for retry
        assert!(matches!(
            sm.state(),
            ChannelState::AwaitingPendingAck | ChannelState::AwaitingJob
        ));
    }

    #[test]
    fn terminal_ack_connection_failed_saves_pending() {
        let job_uuid = test_job_uuid();
        let mut sm = test_sm()
            .with_state(ChannelState::AwaitingTerminalAck {
                job_uuid,
                kind: TerminalKind::Completed {
                    exit_code: 0,
                    output: None,
                },
            })
            .with_in_flight(test_completed_msg());

        let effects = sm.step(Input::ConnectionFailed);
        assert!(effects.iter().any(|e| matches!(
            e,
            Effect::SleepBeforeReconnect(ReconnectReason::TerminalAckConnectionLost)
        )));
        assert_eq!(*sm.state(), ChannelState::Disconnected);
        assert!(sm.pending_result.is_some());
    }

    // --- Issue 1: Send failure preserves message ---

    #[test]
    fn send_failure_preserves_message_as_pending() {
        let job_uuid = test_job_uuid();
        let mut sm = test_sm().with_state(ChannelState::Executing { job_uuid });

        // Job finishes → SM prepares terminal message
        let effects = sm.step(Input::JobFinished(JobFinishResult::Completed {
            exit_code: 0,
            output: None,
            results: vec![],
        }));
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::Send(RunnerMessage::Completed { .. })))
        );
        assert!(sm.in_flight.is_some());

        // Driver's Send fails → feeds ConnectionFailed
        let _effects = sm.step(Input::ConnectionFailed);
        assert_eq!(*sm.state(), ChannelState::Disconnected);
        assert!(sm.pending_result.is_some());

        // Reconnect → pending message resent
        let effects = sm.step(Input::Connected);
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::Send(RunnerMessage::Completed { .. })))
        );
        assert_eq!(*sm.state(), ChannelState::AwaitingPendingAck);
    }

    // --- Shutdown ---

    #[test]
    fn shutdown_from_disconnected() {
        let mut sm = test_sm();
        let effects = sm.step(Input::Shutdown);
        assert!(effects.iter().any(|e| matches!(e, Effect::Close)));
        assert!(effects.iter().any(|e| matches!(e, Effect::Exit)));
        assert_eq!(*sm.state(), ChannelState::ShutDown);
    }

    #[test]
    fn shutdown_during_execution_closes() {
        let mut sm = test_sm().with_state(ChannelState::Executing {
            job_uuid: test_job_uuid(),
        });
        let effects = sm.step(Input::Shutdown);
        assert!(effects.iter().any(|e| matches!(e, Effect::Close)));
        assert!(effects.iter().any(|e| matches!(e, Effect::Exit)));
        assert_eq!(*sm.state(), ChannelState::ShutDown);
    }

    #[test]
    fn shutdown_while_already_shut_down_is_noop() {
        let mut sm = test_sm().with_state(ChannelState::ShutDown);
        let effects = sm.step(Input::Shutdown);
        assert!(effects.is_empty());
        assert_eq!(*sm.state(), ChannelState::ShutDown);
    }

    // --- Full protocol sequences ---

    #[test]
    fn full_happy_path_sequence() {
        let mut sm = test_sm();

        // Connect
        let _effects = sm.step(Input::Connected);
        assert_eq!(*sm.state(), ChannelState::AwaitingJob);

        // Receive job
        let job = test_claimed_job();
        let job_uuid = job.uuid;
        let _effects = sm.step(Input::Message(ServerMessage::Job(job)));
        assert_eq!(*sm.state(), ChannelState::Executing { job_uuid });

        // Job completes
        let _effects = sm.step(Input::JobFinished(JobFinishResult::Completed {
            exit_code: 0,
            output: None,
            results: vec![],
        }));
        assert!(matches!(
            sm.state(),
            ChannelState::AwaitingTerminalAck { .. }
        ));

        // Receive ACK
        let effects = sm.step(Input::Message(ServerMessage::Ack {
            job: Some(job_uuid),
        }));
        assert_eq!(*sm.state(), ChannelState::AwaitingJob);
        let outcome = effects
            .iter()
            .find_map(|e| {
                if let Effect::ReportOutcome(o) = e {
                    Some(o)
                } else {
                    None
                }
            })
            .expect("should have ReportOutcome");
        assert!(outcome.acked);
    }

    #[test]
    fn reconnect_preserves_pending_across_connections() {
        let job_uuid = test_job_uuid();
        let mut sm = test_sm().with_state(ChannelState::Executing { job_uuid });

        // Job completes
        let _effects = sm.step(Input::JobFinished(JobFinishResult::Completed {
            exit_code: 0,
            output: None,
            results: vec![],
        }));

        // ACK timeout → resolve_idle retries
        let _effects = sm.step(Input::ReceiveTimeout);
        assert_eq!(*sm.state(), ChannelState::AwaitingPendingAck);

        // Retry ACK timeout (connection broken)
        let _effects = sm.step(Input::ReceiveTimeout);
        assert_eq!(*sm.state(), ChannelState::Disconnected);
        assert!(sm.pending_result.is_some());

        // Reconnect → resend
        let effects = sm.step(Input::Connected);
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::Send(RunnerMessage::Completed { .. })))
        );
        assert_eq!(*sm.state(), ChannelState::AwaitingPendingAck);
    }

    #[test]
    fn poll_timeout_margin_applied_to_wait() {
        let mut sm = ChannelStateMachine::new(60);
        let effects = sm.step(Input::Connected);
        let wait = effects
            .iter()
            .find_map(|e| {
                if let Effect::WaitForJob(d) = e {
                    Some(d)
                } else {
                    None
                }
            })
            .expect("should have WaitForJob");
        assert_eq!(*wait, Duration::from_secs(60 + POLL_TIMEOUT_MARGIN_SECS));
    }

    #[test]
    fn ack_timeout_uses_correct_duration() {
        let job_uuid = test_job_uuid();
        let mut sm = test_sm().with_state(ChannelState::Executing { job_uuid });

        let effects = sm.step(Input::JobFinished(JobFinishResult::Completed {
            exit_code: 0,
            output: None,
            results: vec![],
        }));
        let recv = effects
            .iter()
            .find_map(|e| {
                if let Effect::Receive(d) = e {
                    Some(d)
                } else {
                    None
                }
            })
            .expect("should have Receive");
        assert_eq!(*recv, ACK_TIMEOUT);
    }

    #[test]
    fn unexpected_input_preserves_state() {
        let mut sm = test_sm().with_state(ChannelState::Disconnected);
        let effects = sm.step(Input::ReceiveTimeout);
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::Log(LogLevel::Error, _)))
        );
        assert_eq!(*sm.state(), ChannelState::Disconnected);
    }

    // --- Effect ordering ---

    #[test]
    fn connection_failure_effects_are_ordered() {
        let mut sm = test_sm();
        let effects = sm.step(Input::ConnectionFailed);
        assert_eq!(effects.len(), 2);
        assert!(matches!(
            effects[0],
            Effect::SleepBeforeReconnect(ReconnectReason::ConnectFailed)
        ));
        assert!(matches!(effects[1], Effect::Connect));
    }

    #[test]
    fn awaiting_job_timeout_effects_are_ordered() {
        let mut sm = test_sm().with_state(ChannelState::AwaitingJob);
        let effects = sm.step(Input::ReceiveTimeout);
        assert_eq!(effects.len(), 3);
        assert!(matches!(effects[0], Effect::Close));
        assert!(matches!(
            effects[1],
            Effect::SleepBeforeReconnect(ReconnectReason::PollingTimeout)
        ));
        assert!(matches!(effects[2], Effect::Connect));
    }

    // --- Unexpected message during Executing ---

    #[test]
    fn unexpected_message_during_executing_preserves_state() {
        let job_uuid = test_job_uuid();
        let mut sm = test_sm().with_state(ChannelState::Executing { job_uuid });
        let effects = sm.step(Input::Message(ServerMessage::NoJob));
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::Log(LogLevel::Error, _)))
        );
        assert_eq!(*sm.state(), ChannelState::Executing { job_uuid });
    }

    // --- Pending result assertions ---

    #[test]
    fn pending_dropped_after_max_retries_sends_ready() {
        let mut sm = test_sm().with_pending(test_completed_msg(), MAX_PENDING_RESULT_RETRIES);
        let effects = sm.step(Input::Connected);
        // Verify both the drop log and the Ready message are present
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::Log(LogLevel::Error, s) if s.contains("dropping")))
        );
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::Send(RunnerMessage::Ready { .. })))
        );
        assert!(effects.iter().any(|e| matches!(e, Effect::WaitForJob(_))));
        assert_eq!(*sm.state(), ChannelState::AwaitingJob);
        assert!(sm.pending_result.is_none());
        assert!(sm.in_flight.is_none());
    }
}
