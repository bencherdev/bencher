//! Sans-IO state machine for the runner WebSocket channel protocol.
//!
//! This module contains pure decision logic with no I/O. The driver in `mod.rs`
//! executes effects and feeds inputs, keeping all I/O at the boundary.
//!
//! Not gated behind `cfg(target_os = "linux")` — tests run on any platform.

use std::time::Duration;

use bencher_json::{
    DateTime, JobUuid, JsonClaimedJob,
    runner::{
        JsonIterationOutput, JsonPaused, JsonReady, JsonRunnerHealth, JsonRunnerMetadata,
        PauseReason, RunnerMessage, ServerMessage,
    },
};
use bencher_valid::Sha256;
use url::Url;

/// Margin added to `poll_timeout` for the WS read timeout, giving the server
/// time to send `NoJob` after its own deadline.
const POLL_TIMEOUT_MARGIN_SECS: u64 = 30;

/// Maximum number of times to retry sending a pending result before dropping it.
const MAX_PENDING_RESULT_RETRIES: u32 = 5;

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
    /// The host is being probed, to send `Ready` or `Paused`.
    Probing,
    /// `Ready` or `Paused` sent, waiting for `Job` or `NoJob`.
    AwaitingJob,
    /// Job executing. Heartbeat thread is active.
    Executing { job_uuid: JobUuid },
    /// Terminal message sent, waiting for server ACK.
    AwaitingTerminalAck {
        job_uuid: JobUuid,
        kind: TerminalKind,
    },
    /// Self-update in progress. WebSocket is closed.
    Updating,
    /// Clean shutdown.
    ShutDown,
}

/// What kind of terminal message was sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TerminalKind {
    Completed {
        exit_code: i32,
        stdout_bytes: usize,
        stderr_bytes: usize,
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
    /// Self-update download/verify/exec failed.
    SelfUpdateFailed,
    /// The host probe finished.
    Probed(Probe),
}

/// What the host probe found at the idle decision point.
#[derive(Debug)]
pub struct Probe {
    /// Why the runner should take no Job, empty when it may.
    pub reasons: Vec<PauseReason>,
    /// The host's disk health, sent with `Ready` and `Paused`.
    pub health: Option<JsonRunnerHealth>,
    /// When the probe ran, by the runner's clock.
    pub now: DateTime,
}

/// Result of job execution, produced by the driver.
#[derive(Debug)]
pub enum JobFinishResult {
    Completed {
        exit_code: i32,
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
    /// Self-update failed, reconnecting to resume on old version.
    SelfUpdateFailed,
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
            Self::SelfUpdateFailed => write!(f, "self-update failed"),
        }
    }
}

/// Effects for the driver to execute.
#[derive(Debug)]
pub enum Effect {
    /// Establish WebSocket connection.
    Connect,
    /// Probe the host. Driver feeds `Input::Probed`.
    Probe,
    /// Release the job lock a clear probe took, so host maintenance may run.
    ReleaseJobLock,
    /// A pause began, for these reasons.
    PauseBegan(Vec<PauseReason>),
    /// A pause ended.
    PauseEnded,
    /// Wait this long before a reconnect while paused, ahead of its usual delay.
    HoldOff(Duration),
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
    /// Download, verify, and exec a new runner binary.
    SelfUpdate {
        version: String,
        url: Url,
        checksum: Sha256,
    },
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
    runner: Option<JsonRunnerMetadata>,
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
    /// Shutdown arrived while a Job's result was owed, so exit once it is sent.
    stopping: bool,
    /// Set from the probe that began a pause until one finds the host clear.
    pause: Option<Pause>,
}

#[derive(Debug)]
struct Pause {
    since: DateTime,
    reasons: Vec<PauseReason>,
}

impl ChannelStateMachine {
    pub fn new(poll_timeout_secs: u32, runner: Option<JsonRunnerMetadata>) -> Self {
        Self {
            state: ChannelState::Disconnected,
            poll_timeout_secs,
            runner,
            pending_result: None,
            pending_retry_count: 0,
            in_flight: None,
            stopping: false,
            pause: None,
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

        if matches!(input, Input::Shutdown) {
            return self.shut_down(state);
        }

        match state {
            ChannelState::Disconnected => self.handle_disconnected(&input),
            ChannelState::AwaitingPendingAck => self.handle_awaiting_pending_ack(input),
            ChannelState::Probing => self.handle_probing(input),
            ChannelState::AwaitingJob => self.handle_awaiting_job(input),
            ChannelState::Executing { job_uuid } => self.handle_executing(job_uuid, input),
            ChannelState::AwaitingTerminalAck { job_uuid, kind } => {
                self.handle_awaiting_terminal_ack(job_uuid, kind, input)
            },
            ChannelState::Updating => self.handle_updating(input),
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
                self.reconnect(ReconnectReason::ConnectFailed)
            },
            input @ (Input::Message(_)
            | Input::ReceiveTimeout
            | Input::JobFinished(_)
            | Input::Shutdown
            | Input::SelfUpdateFailed
            | Input::Probed(_)) => self.unexpected(ChannelState::Disconnected, input),
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
                let mut effects = vec![
                    Effect::Log(LogLevel::Warn, "Retry ACK timed out".to_owned()),
                    Effect::Close,
                ];
                effects.extend(self.reconnect(ReconnectReason::PendingAckTimeout));
                effects
            },
            Input::Message(other) => {
                self.pending_result = self.in_flight.take();
                self.state = ChannelState::Disconnected;
                let mut effects = vec![
                    Effect::Log(
                        LogLevel::Warn,
                        format!("Expected ACK for pending result, got {other:?}"),
                    ),
                    Effect::Close,
                ];
                effects.extend(self.reconnect(ReconnectReason::PendingAckUnexpectedMessage));
                effects
            },
            Input::ConnectionFailed => {
                self.pending_result = self.in_flight.take();
                self.state = ChannelState::Disconnected;
                self.reconnect(ReconnectReason::PendingAckConnectionLost)
            },
            input @ (Input::Connected
            | Input::JobFinished(_)
            | Input::Shutdown
            | Input::SelfUpdateFailed
            | Input::Probed(_)) => self.unexpected(ChannelState::AwaitingPendingAck, &input),
        }
    }

    fn handle_probing(&mut self, input: Input) -> Vec<Effect> {
        match input {
            Input::Probed(probe) => self.send_ready_or_paused(probe),
            input @ (Input::Connected
            | Input::ConnectionFailed
            | Input::Message(_)
            | Input::ReceiveTimeout
            | Input::JobFinished(_)
            | Input::Shutdown
            | Input::SelfUpdateFailed) => self.unexpected(ChannelState::Probing, &input),
        }
    }

    fn handle_awaiting_job(&mut self, input: Input) -> Vec<Effect> {
        match input {
            Input::Message(ServerMessage::Job(job)) => {
                let job_uuid = job.uuid;
                // The server holds a paused runner's poll without claiming.
                if let Some(pause) = &self.pause {
                    let error = format!(
                        "The runner was paused for {}, so it ran no Job",
                        describe(&pause.reasons)
                    );
                    return self.fail_unrun(job_uuid, error);
                }
                self.state = ChannelState::Executing { job_uuid };
                vec![
                    Effect::Log(LogLevel::Info, format!("Received job: {job_uuid}")),
                    Effect::Send(RunnerMessage::Running),
                    Effect::ExecuteJob(job),
                ]
            },
            Input::Message(ServerMessage::NoJob) => {
                let mut effects = vec![Effect::ReleaseJobLock];
                effects.extend(self.resolve_idle());
                effects
            },
            Input::Message(ServerMessage::Update {
                version,
                url,
                checksum,
            }) => {
                self.state = ChannelState::Updating;
                vec![
                    Effect::ReleaseJobLock,
                    Effect::Log(
                        LogLevel::Info,
                        format!("Server requested update to version {version}"),
                    ),
                    Effect::Close,
                    Effect::SelfUpdate {
                        version,
                        url,
                        checksum,
                    },
                ]
            },
            Input::ReceiveTimeout => {
                self.state = ChannelState::Disconnected;
                let mut effects = vec![Effect::ReleaseJobLock, Effect::Close];
                effects.extend(self.reconnect(ReconnectReason::PollingTimeout));
                effects
            },
            Input::ConnectionFailed => {
                self.state = ChannelState::Disconnected;
                let mut effects = vec![Effect::ReleaseJobLock];
                effects.extend(self.reconnect(ReconnectReason::PollingConnectionLost));
                effects
            },
            input @ (Input::Connected
            | Input::Message(ServerMessage::Ack { .. } | ServerMessage::Cancel)
            | Input::JobFinished(_)
            | Input::Shutdown
            | Input::SelfUpdateFailed
            | Input::Probed(_)) => self.unexpected(ChannelState::AwaitingJob, &input),
        }
    }

    fn handle_executing(&mut self, job_uuid: JobUuid, input: Input) -> Vec<Effect> {
        match input {
            Input::JobFinished(result) => {
                let (msg, kind) = build_terminal_message(job_uuid, result);
                self.in_flight = Some(msg.clone());
                self.state = ChannelState::AwaitingTerminalAck { job_uuid, kind };
                vec![
                    Effect::ReleaseJobLock,
                    Effect::Send(msg),
                    Effect::Receive(ACK_TIMEOUT),
                ]
            },
            // `Running` was not sent, so the Job never ran.
            Input::ConnectionFailed => {
                if self.stopping {
                    return self.exit();
                }
                self.state = ChannelState::Disconnected;
                let mut effects = vec![Effect::ReleaseJobLock];
                effects.extend(self.reconnect(ReconnectReason::ExecutingConnectionLost));
                effects
            },
            input @ (Input::Connected
            | Input::Message(_)
            | Input::ReceiveTimeout
            | Input::Shutdown
            | Input::SelfUpdateFailed
            | Input::Probed(_)) => self.unexpected(ChannelState::Executing { job_uuid }, &input),
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
                let report = Effect::ReportOutcome(JobOutcome {
                    job: job_uuid,
                    kind,
                    acked: false,
                });
                if self.stopping {
                    let mut effects = vec![report];
                    effects.extend(self.exit());
                    return effects;
                }
                self.state = ChannelState::Disconnected;
                let mut effects = vec![report];
                effects.extend(self.reconnect(ReconnectReason::TerminalAckConnectionLost));
                effects
            },
            input @ (Input::Connected
            | Input::JobFinished(_)
            | Input::Shutdown
            | Input::SelfUpdateFailed
            | Input::Probed(_)) => {
                self.unexpected(ChannelState::AwaitingTerminalAck { job_uuid, kind }, &input)
            },
        }
    }

    fn handle_updating(&mut self, input: Input) -> Vec<Effect> {
        match input {
            Input::SelfUpdateFailed => {
                self.state = ChannelState::Disconnected;
                self.reconnect(ReconnectReason::SelfUpdateFailed)
            },
            input @ (Input::Connected
            | Input::ConnectionFailed
            | Input::Message(_)
            | Input::ReceiveTimeout
            | Input::JobFinished(_)
            | Input::Shutdown
            | Input::Probed(_)) => self.unexpected(ChannelState::Updating, &input),
        }
    }

    // --- Helpers ---

    /// Exits at once, unless a Job's result is still owed: then the Job runs
    /// on (the runner's stop cancels it), and the machine exits once its result
    /// is sent rather than polling for another.
    fn shut_down(&mut self, state: ChannelState) -> Vec<Effect> {
        if matches!(
            state,
            ChannelState::Executing { .. } | ChannelState::AwaitingTerminalAck { .. }
        ) {
            self.stopping = true;
            self.state = state;
            return vec![Effect::Log(
                LogLevel::Info,
                "Stopping once the running Job's result is sent".to_owned(),
            )];
        }
        self.exit()
    }

    fn exit(&mut self) -> Vec<Effect> {
        self.state = ChannelState::ShutDown;
        vec![Effect::Close, Effect::Exit]
    }

    /// Log an unexpected (state, input) combination and restore the state.
    fn unexpected(&mut self, state: ChannelState, input: &Input) -> Vec<Effect> {
        let msg = format!("Unexpected input {input:?} in state {state:?}");
        self.state = state;
        vec![Effect::Log(LogLevel::Error, msg)]
    }

    /// Resolve the idle decision point: if there's a pending result to retry,
    /// send it; otherwise probe the host for `Ready` or `Paused`. A shutdown
    /// that waited on a Job's result exits here instead.
    fn resolve_idle(&mut self) -> Vec<Effect> {
        if self.stopping {
            return self.exit();
        }
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
            self.state = ChannelState::Probing;
            vec![Effect::Probe]
        }
    }

    /// Send `Paused` while the probe found any reason to take no Job, else
    /// `Ready`, and wait for the server's answer either way.
    fn send_ready_or_paused(&mut self, probe: Probe) -> Vec<Effect> {
        let Probe {
            reasons,
            health,
            now,
        } = probe;
        let ready = JsonReady::new_with_health(
            bencher_valid::PollTimeout::try_from(self.poll_timeout_secs).ok(),
            self.runner.clone(),
            health,
        );
        let mut effects = Vec::new();
        let msg = if reasons.is_empty() {
            if self.pause.take().is_some() {
                effects.push(Effect::PauseEnded);
            }
            RunnerMessage::Ready(ready)
        } else {
            let since = if let Some(pause) = &self.pause {
                pause.since
            } else {
                effects.push(Effect::PauseBegan(reasons.clone()));
                now
            };
            self.pause = Some(Pause {
                since,
                reasons: reasons.clone(),
            });
            RunnerMessage::Paused(JsonPaused {
                reasons,
                since,
                ready,
            })
        };
        self.state = ChannelState::AwaitingJob;
        effects.extend([
            Effect::Send(msg),
            Effect::WaitForJob(Duration::from_secs(
                u64::from(self.poll_timeout_secs) + POLL_TIMEOUT_MARGIN_SECS,
            )),
        ]);
        effects
    }

    /// Report a Job failed without running it, through the usual ACK and retry.
    fn fail_unrun(&mut self, job_uuid: JobUuid, error: String) -> Vec<Effect> {
        let log = Effect::Log(LogLevel::Warn, format!("Received job {job_uuid}: {error}"));
        let (msg, kind) = build_terminal_message(
            job_uuid,
            JobFinishResult::Failed {
                error,
                results: Vec::new(),
            },
        );
        self.in_flight = Some(msg.clone());
        self.state = ChannelState::AwaitingTerminalAck { job_uuid, kind };
        vec![log, Effect::Send(msg), Effect::Receive(ACK_TIMEOUT)]
    }

    /// While paused, a reconnect first waits out one poll, so a server that
    /// closes on `Paused` sees no more connects than polls.
    fn reconnect(&self, reason: ReconnectReason) -> Vec<Effect> {
        let mut effects = Vec::new();
        if self.pause.is_some() {
            effects.push(Effect::HoldOff(Duration::from_secs(u64::from(
                self.poll_timeout_secs,
            ))));
        }
        effects.extend([Effect::SleepBeforeReconnect(reason), Effect::Connect]);
        effects
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
        JobFinishResult::Completed { exit_code, results } => {
            let kind = TerminalKind::Completed {
                exit_code,
                stdout_bytes: results
                    .iter()
                    .filter_map(|result| result.stdout.as_ref())
                    .map(String::len)
                    .sum(),
                stderr_bytes: results
                    .iter()
                    .filter_map(|result| result.stderr.as_ref())
                    .map(String::len)
                    .sum(),
            };
            let msg = RunnerMessage::Completed {
                job: job_uuid,
                results,
            };
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

/// What a pause is for, such as `a RAID sync on md0, host maintenance`.
fn describe(reasons: &[PauseReason]) -> String {
    reasons
        .iter()
        .map(|reason| match reason {
            PauseReason::Raid { array, .. } => format!("a RAID sync on {array}"),
            PauseReason::Maintenance { .. } => "host maintenance".to_owned(),
            PauseReason::Other => "an unknown reason".to_owned(),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

/// A claimed Job for tests, shared with the driver's.
#[cfg(test)]
pub(super) fn test_claimed_job() -> Box<JsonClaimedJob> {
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

#[cfg(test)]
mod tests {
    use bencher_json::runner::HealthState;
    use bencher_valid::{Architecture, OperatingSystem};

    use super::*;

    fn test_job_uuid() -> JobUuid {
        "550e8400-e29b-41d4-a716-446655440000".parse().unwrap()
    }

    fn other_job_uuid() -> JobUuid {
        "660e8400-e29b-41d4-a716-446655440000".parse().unwrap()
    }

    fn test_sm() -> ChannelStateMachine {
        ChannelStateMachine::new(
            30,
            Some(JsonRunnerMetadata {
                os: OperatingSystem::Linux,
                arch: Architecture::X86_64,
                version: bencher_json::BENCHER_API_VERSION.to_owned(),
                channel: None,
                checksum: None,
            }),
        )
    }

    fn test_completed_msg() -> RunnerMessage {
        RunnerMessage::Completed {
            job: test_job_uuid(),
            results: vec![],
        }
    }

    // 06:23 UTC, 11 July 2024, plus `secs`.
    fn at(secs: i64) -> DateTime {
        DateTime::try_from(1_720_678_980 + secs).unwrap()
    }

    fn raid() -> PauseReason {
        PauseReason::Raid {
            array: "md0".to_owned(),
            action: bencher_json::runner::MdSyncAction::Check,
            done: Some(10),
            total: Some(100),
            speed: Some(2048),
        }
    }

    fn clear() -> Input {
        Input::Probed(Probe {
            reasons: Vec::new(),
            health: None,
            now: at(0),
        })
    }

    fn busy(now: DateTime) -> Input {
        Input::Probed(Probe {
            reasons: vec![raid()],
            health: None,
            now,
        })
    }

    /// Steps `input`, which must end in a host probe, then answers the probe clear.
    fn probed(sm: &mut ChannelStateMachine, input: Input) -> Vec<Effect> {
        let mut effects = sm.step(input);
        assert!(
            matches!(effects.last(), Some(Effect::Probe)),
            "expected a probe: {effects:?}"
        );
        assert_eq!(*sm.state(), ChannelState::Probing);
        effects.extend(sm.step(clear()));
        effects
    }

    /// A machine that sent `Paused` for a RAID sync since `at(0)`.
    fn paused_sm() -> ChannelStateMachine {
        let mut sm = test_sm();
        sm.step(Input::Connected);
        sm.step(busy(at(0)));
        assert_eq!(*sm.state(), ChannelState::AwaitingJob);
        sm
    }

    fn sent_paused(effects: &[Effect]) -> Option<&JsonPaused> {
        effects.iter().find_map(|e| {
            if let Effect::Send(RunnerMessage::Paused(paused)) = e {
                Some(paused)
            } else {
                None
            }
        })
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
        let effects = probed(&mut sm, Input::Connected);
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

        let effects = probed(&mut sm, Input::Message(ServerMessage::Ack { job: None }));
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

        let effects = probed(&mut sm, Input::Connected);
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
        let effects = probed(&mut sm, Input::Message(ServerMessage::NoJob));
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
        let iteration = |stdout: &str, stderr: Option<&str>| JsonIterationOutput {
            exit_code: 0,
            stdout: Some(stdout.to_owned()),
            stderr: stderr.map(ToOwned::to_owned),
            output: None,
        };

        let effects = sm.step(Input::JobFinished(JobFinishResult::Completed {
            exit_code: 0,
            results: vec![iteration("hello", Some("oops")), iteration("hi!", None)],
        }));
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::Send(RunnerMessage::Completed { .. })))
        );
        assert!(effects.iter().any(|e| matches!(e, Effect::Receive(_))));
        // Fails if the counts cover only one iteration or swap the streams.
        assert!(
            matches!(
                sm.state(),
                ChannelState::AwaitingTerminalAck {
                    kind: TerminalKind::Completed {
                        exit_code: 0,
                        stdout_bytes: 8,
                        stderr_bytes: 4,
                    },
                    ..
                }
            ),
            "{:?}",
            sm.state()
        );
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
                    stdout_bytes: 0,
                    stderr_bytes: 0,
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
                    stdout_bytes: 0,
                    stderr_bytes: 0,
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
                    stdout_bytes: 0,
                    stderr_bytes: 0,
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
                    stdout_bytes: 0,
                    stderr_bytes: 0,
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

    // --- Updating ---

    #[test]
    fn update_message_transitions_to_updating() {
        let mut sm = test_sm().with_state(ChannelState::AwaitingJob);
        let effects = sm.step(Input::Message(ServerMessage::Update {
            version: "99.0.0".to_owned(),
            url: "https://example.com/runner".parse().unwrap(),
            checksum: "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"
                .parse()
                .unwrap(),
        }));
        assert_eq!(*sm.state(), ChannelState::Updating);
        assert!(effects.iter().any(|e| matches!(e, Effect::Close)));
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::SelfUpdate { .. }))
        );
    }

    #[test]
    fn self_update_failed_reconnects() {
        let mut sm = test_sm().with_state(ChannelState::Updating);
        let effects = sm.step(Input::SelfUpdateFailed);
        assert_eq!(*sm.state(), ChannelState::Disconnected);
        assert!(effects.iter().any(|e| matches!(
            e,
            Effect::SleepBeforeReconnect(ReconnectReason::SelfUpdateFailed)
        )));
        assert!(effects.iter().any(|e| matches!(e, Effect::Connect)));
    }

    #[test]
    fn shutdown_during_updating() {
        let mut sm = test_sm().with_state(ChannelState::Updating);
        let effects = sm.step(Input::Shutdown);
        assert!(effects.iter().any(|e| matches!(e, Effect::Close)));
        assert!(effects.iter().any(|e| matches!(e, Effect::Exit)));
        assert_eq!(*sm.state(), ChannelState::ShutDown);
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
    fn shutdown_during_execution_exits_once_the_result_is_acked() {
        // Prevents a shutdown dropping the running Job's result, which leaves
        // the server to find out only when the Job's heartbeat times out.
        let job_uuid = test_job_uuid();
        let mut sm = test_sm().with_state(ChannelState::Executing { job_uuid });

        let effects = sm.step(Input::Shutdown);
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, Effect::Close | Effect::Exit)),
            "the Job's result is still owed: {effects:?}"
        );
        assert_eq!(*sm.state(), ChannelState::Executing { job_uuid });

        let effects = sm.step(Input::JobFinished(JobFinishResult::Failed {
            error: "the runner stopped".to_owned(),
            results: vec![],
        }));
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::Send(RunnerMessage::Failed { .. }))),
            "the result goes out: {effects:?}"
        );

        let effects = sm.step(Input::Message(ServerMessage::Ack {
            job: Some(job_uuid),
        }));
        assert!(
            matches!(effects.last(), Some(Effect::Exit)),
            "exits once the result is acked: {effects:?}"
        );
        assert!(
            !effects.iter().any(|e| matches!(e, Effect::Send(_))),
            "polls for no other Job: {effects:?}"
        );
        assert_eq!(*sm.state(), ChannelState::ShutDown);
    }

    #[test]
    fn shutdown_during_execution_exits_when_the_result_goes_unacked() {
        // Prevents a shutting down runner retrying or reconnecting, which holds
        // it past the service manager's stop timeout.
        let job_uuid = test_job_uuid();
        let awaiting_ack = || ChannelState::AwaitingTerminalAck {
            job_uuid,
            kind: TerminalKind::Failed {
                error: "the runner stopped".to_owned(),
            },
        };
        for (state, input) in [
            (awaiting_ack(), Input::ReceiveTimeout),
            (awaiting_ack(), Input::ConnectionFailed),
            (awaiting_ack(), Input::Message(ServerMessage::NoJob)),
            (
                ChannelState::Executing { job_uuid },
                Input::ConnectionFailed,
            ),
        ] {
            let case = format!("{input:?} in {state:?}");
            let mut sm = test_sm()
                .with_state(state)
                .with_in_flight(test_completed_msg());
            sm.step(Input::Shutdown);

            let effects = sm.step(input);

            assert!(
                matches!(effects.last(), Some(Effect::Exit)),
                "{case}: {effects:?}"
            );
            assert!(
                !effects
                    .iter()
                    .any(|e| matches!(e, Effect::Send(_) | Effect::Connect)),
                "{case}: {effects:?}"
            );
        }
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
        let _effects = probed(&mut sm, Input::Connected);
        assert_eq!(*sm.state(), ChannelState::AwaitingJob);

        // Receive job
        let job = test_claimed_job();
        let job_uuid = job.uuid;
        let _effects = sm.step(Input::Message(ServerMessage::Job(job)));
        assert_eq!(*sm.state(), ChannelState::Executing { job_uuid });

        // Job completes
        let _effects = sm.step(Input::JobFinished(JobFinishResult::Completed {
            exit_code: 0,
            results: vec![],
        }));
        assert!(matches!(
            sm.state(),
            ChannelState::AwaitingTerminalAck { .. }
        ));

        // Receive ACK
        let effects = probed(
            &mut sm,
            Input::Message(ServerMessage::Ack {
                job: Some(job_uuid),
            }),
        );
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
        let mut sm = ChannelStateMachine::new(
            60,
            Some(JsonRunnerMetadata {
                os: OperatingSystem::Linux,
                arch: Architecture::X86_64,
                version: bencher_json::BENCHER_API_VERSION.to_owned(),
                channel: None,
                checksum: None,
            }),
        );
        let effects = probed(&mut sm, Input::Connected);
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
        assert_eq!(effects.len(), 4);
        assert!(matches!(effects[0], Effect::ReleaseJobLock));
        assert!(matches!(effects[1], Effect::Close));
        assert!(matches!(
            effects[2],
            Effect::SleepBeforeReconnect(ReconnectReason::PollingTimeout)
        ));
        assert!(matches!(effects[3], Effect::Connect));
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
        let effects = probed(&mut sm, Input::Connected);
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

    // --- Pause ---

    #[test]
    fn a_busy_probe_pauses_and_a_clear_one_sends_ready() {
        // Kills a probe whose reasons are ignored, a `Paused` missing what
        // `Ready` carries (the server's update check needs the metadata), and
        // a `NoJob` that answers without probing again.
        let mut sm = test_sm();
        assert!(matches!(
            sm.step(Input::Connected).last(),
            Some(Effect::Probe)
        ));

        let effects = sm.step(busy(at(0)));
        let paused = sent_paused(&effects).expect("a busy probe sends Paused");
        assert_eq!(paused.reasons, [raid()]);
        assert_eq!(paused.since, at(0));
        assert_eq!(paused.ready.poll_timeout.map(u32::from), Some(30));
        assert!(paused.ready.runner.is_some());
        assert!(effects.iter().any(|e| matches!(e, Effect::WaitForJob(_))));
        assert!(
            !effects
                .iter()
                .any(|e| matches!(e, Effect::Send(RunnerMessage::Ready(_))))
        );
        assert_eq!(*sm.state(), ChannelState::AwaitingJob);

        let effects = sm.step(Input::Message(ServerMessage::NoJob));
        assert!(
            matches!(effects.as_slice(), [Effect::ReleaseJobLock, Effect::Probe]),
            "{effects:?}"
        );

        let effects = sm.step(clear());
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::Send(RunnerMessage::Ready(_))))
        );
        assert!(sent_paused(&effects).is_none());
    }

    #[test]
    fn a_pause_keeps_its_start_and_is_logged_once_each_way() {
        // Kills a `since` that restarts with every probe or survives the
        // pause's end, and a start or end logged on every poll.
        let mut sm = paused_sm();
        let no_job = || Input::Message(ServerMessage::NoJob);
        let began = |effects: &[Effect]| {
            effects
                .iter()
                .filter(|e| matches!(e, Effect::PauseBegan(_)))
                .count()
        };
        let ended = |effects: &[Effect]| {
            effects
                .iter()
                .filter(|e| matches!(e, Effect::PauseEnded))
                .count()
        };

        sm.step(no_job());
        let effects = sm.step(busy(at(60)));
        assert_eq!(sent_paused(&effects).unwrap().since, at(0));
        assert_eq!(began(&effects), 0);
        assert_eq!(ended(&effects), 0);

        sm.step(no_job());
        let effects = sm.step(clear());
        assert_eq!(ended(&effects), 1);

        sm.step(no_job());
        let effects = sm.step(clear());
        assert_eq!(ended(&effects), 0);

        sm.step(no_job());
        let effects = sm.step(busy(at(180)));
        assert_eq!(sent_paused(&effects).unwrap().since, at(180));
        assert_eq!(began(&effects), 1);
    }

    #[test]
    fn a_paused_runner_still_updates() {
        // Kills a pause that ignores the server's `Update`, which would strand
        // a paused runner on its version.
        let mut sm = paused_sm();
        let effects = sm.step(Input::Message(ServerMessage::Update {
            version: "99.0.0".to_owned(),
            url: "https://example.com/runner".parse().unwrap(),
            checksum: "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"
                .parse()
                .unwrap(),
        }));
        assert!(
            effects
                .iter()
                .any(|e| matches!(e, Effect::SelfUpdate { .. }))
        );
        assert_eq!(*sm.state(), ChannelState::Updating);
    }

    #[test]
    fn a_job_after_a_pause_fails_without_running() {
        // Kills a paused runner that runs a Job the server sent it anyway.
        let mut sm = paused_sm();
        let job = test_claimed_job();
        let job_uuid = job.uuid;

        let effects = sm.step(Input::Message(ServerMessage::Job(job)));
        assert!(
            !effects.iter().any(|e| matches!(
                e,
                Effect::ExecuteJob(_) | Effect::Send(RunnerMessage::Running)
            )),
            "{effects:?}"
        );
        assert!(
            effects.iter().any(|e| matches!(
                e,
                Effect::Send(RunnerMessage::Failed { job, error, .. })
                    if *job == job_uuid && error.contains("a RAID sync on md0")
            )),
            "{effects:?}"
        );
        assert!(matches!(
            sm.state(),
            ChannelState::AwaitingTerminalAck {
                kind: TerminalKind::Failed { .. },
                ..
            }
        ));

        let effects = sm.step(Input::Message(ServerMessage::Ack {
            job: Some(job_uuid),
        }));
        assert!(matches!(effects.last(), Some(Effect::Probe)), "{effects:?}");
    }

    #[test]
    fn reconnects_hold_off_a_poll_only_while_paused() {
        // Kills a paused runner that reconnects every few seconds to a server
        // that closes on `Paused`, tripping its connect limit, on any path that
        // reconnects while paused, a hold of other than one poll, and a hold
        // that outlives the pause.
        const POLL: u32 = 55;
        let held = |effects: &[Effect], reason: ReconnectReason| {
            assert!(
                matches!(
                    effects,
                    [.., Effect::HoldOff(hold), Effect::SleepBeforeReconnect(r), Effect::Connect]
                        if *hold == Duration::from_secs(u64::from(POLL)) && *r == reason
                ),
                "{reason}: {effects:?}"
            );
        };
        let mut sm = ChannelStateMachine::new(POLL, None);
        sm.step(Input::Connected);
        sm.step(busy(at(0)));

        // A server that does not know `Paused` closes the channel.
        held(
            &sm.step(Input::ConnectionFailed),
            ReconnectReason::PollingConnectionLost,
        );
        // A connect the server's limiter refuses.
        held(
            &sm.step(Input::ConnectionFailed),
            ReconnectReason::ConnectFailed,
        );
        sm.step(Input::Connected);
        sm.step(busy(at(60)));
        held(
            &sm.step(Input::ReceiveTimeout),
            ReconnectReason::PollingTimeout,
        );

        // A new server has the paused runner update, and the update fails.
        sm.step(Input::Connected);
        sm.step(busy(at(120)));
        sm.step(Input::Message(ServerMessage::Update {
            version: "99.0.0".to_owned(),
            url: "https://example.com/runner".parse().unwrap(),
            checksum: "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"
                .parse()
                .unwrap(),
        }));
        held(
            &sm.step(Input::SelfUpdateFailed),
            ReconnectReason::SelfUpdateFailed,
        );

        // The result of a Job failed unrun is lost with the connection, and
        // each resend at the next connect fails another way.
        sm.step(Input::Connected);
        sm.step(busy(at(180)));
        sm.step(Input::Message(ServerMessage::Job(test_claimed_job())));
        held(
            &sm.step(Input::ConnectionFailed),
            ReconnectReason::TerminalAckConnectionLost,
        );
        for (input, reason) in [
            (Input::ReceiveTimeout, ReconnectReason::PendingAckTimeout),
            (
                Input::Message(ServerMessage::NoJob),
                ReconnectReason::PendingAckUnexpectedMessage,
            ),
            (
                Input::ConnectionFailed,
                ReconnectReason::PendingAckConnectionLost,
            ),
        ] {
            sm.step(Input::Connected);
            assert_eq!(*sm.state(), ChannelState::AwaitingPendingAck);
            held(&sm.step(input), reason);
        }

        sm.step(Input::Connected);
        sm.step(Input::Message(ServerMessage::Ack {
            job: Some(test_job_uuid()),
        }));
        sm.step(clear());
        let effects = sm.step(Input::ConnectionFailed);
        assert!(
            matches!(
                effects.as_slice(),
                [.., Effect::SleepBeforeReconnect(_), Effect::Connect]
            ) && !effects.iter().any(|e| matches!(e, Effect::HoldOff(_))),
            "{effects:?}"
        );
    }

    // --- Job lock ---

    fn releases(effects: &[Effect]) -> usize {
        effects
            .iter()
            .filter(|e| matches!(e, Effect::ReleaseJobLock))
            .count()
    }

    #[test]
    fn the_job_lock_is_held_from_ready_until_the_job_finishes() {
        // Kills a release before the Job ends, which lets maintenance run
        // during it, and a lock kept past the Job.
        let mut sm = test_sm();
        let effects = probed(&mut sm, Input::Connected);
        assert_eq!(releases(&effects), 0, "{effects:?}");
        let job = test_claimed_job();
        let effects = sm.step(Input::Message(ServerMessage::Job(job)));
        assert_eq!(releases(&effects), 0, "{effects:?}");

        let effects = sm.step(Input::JobFinished(JobFinishResult::Completed {
            exit_code: 0,
            results: vec![],
        }));
        assert!(
            matches!(
                effects.as_slice(),
                [
                    Effect::ReleaseJobLock,
                    Effect::Send(RunnerMessage::Completed { .. }),
                    ..
                ]
            ),
            "released before the result goes out: {effects:?}"
        );
    }

    #[test]
    fn the_job_lock_is_released_whenever_no_job_follows() {
        // Kills a lock kept through a `NoJob`, an update, a timeout, or a lost
        // connection, any of which holds maintenance off for good.
        let update = || {
            Input::Message(ServerMessage::Update {
                version: "99.0.0".to_owned(),
                url: "https://example.com/runner".parse().unwrap(),
                checksum: "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"
                    .parse()
                    .unwrap(),
            })
        };
        for (input, state) in [
            (
                Input::Message(ServerMessage::NoJob),
                ChannelState::AwaitingJob,
            ),
            (update(), ChannelState::AwaitingJob),
            (Input::ReceiveTimeout, ChannelState::AwaitingJob),
            (Input::ConnectionFailed, ChannelState::AwaitingJob),
            // `Running` failed to send, so the Job never ran.
            (
                Input::ConnectionFailed,
                ChannelState::Executing {
                    job_uuid: test_job_uuid(),
                },
            ),
        ] {
            let case = format!("{input:?} in {state:?}");
            let mut sm = test_sm().with_state(state);
            let effects = sm.step(input);
            assert_eq!(releases(&effects), 1, "{case}: {effects:?}");
            assert!(
                matches!(effects.first(), Some(Effect::ReleaseJobLock)),
                "released first: {case}: {effects:?}"
            );
        }
    }

    // --- Health ---

    #[test]
    fn the_probes_health_rides_on_ready_and_paused() {
        // Kills health dropped from either message, which leaves the server
        // with a stale report while the disks change.
        let health = |state| JsonRunnerHealth {
            state,
            findings: Vec::new(),
            arrays: Vec::new(),
            nvme: Vec::new(),
        };
        let sent_health = |effects: &[Effect]| {
            effects.iter().find_map(|e| {
                if let Effect::Send(RunnerMessage::Ready(ready)) = e {
                    Some(ready.health.clone())
                } else if let Effect::Send(RunnerMessage::Paused(paused)) = e {
                    Some(paused.ready.health.clone())
                } else {
                    None
                }
            })
        };
        let mut sm = test_sm();
        sm.step(Input::Connected);
        let effects = sm.step(Input::Probed(Probe {
            reasons: Vec::new(),
            health: Some(health(HealthState::Ok)),
            now: at(0),
        }));
        assert_eq!(sent_health(&effects), Some(Some(health(HealthState::Ok))));

        sm.step(Input::Message(ServerMessage::NoJob));
        let effects = sm.step(Input::Probed(Probe {
            reasons: vec![raid()],
            health: Some(health(HealthState::Failing)),
            now: at(60),
        }));
        assert_eq!(
            sent_health(&effects),
            Some(Some(health(HealthState::Failing)))
        );
    }
}
