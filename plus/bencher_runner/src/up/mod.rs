// All `up` code is used on Linux; suppress dead-code warnings on other platforms
// where `Up::run()` is a stub.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

use std::sync::atomic::AtomicBool;
#[cfg(target_os = "linux")]
use std::sync::atomic::Ordering;

use url::Url;

use crate::cpu::CpuLayout;
#[cfg(target_os = "linux")]
use crate::firecracker::FirecrackerLogLevel;
use crate::tuning::TuningConfig;

mod api_client;
mod error;
mod job;
mod state_machine;
mod websocket;

pub use error::UpError;

#[cfg(target_os = "linux")]
use api_client::RunnerApiClient;
#[cfg(target_os = "linux")]
use bencher_json::runner::{RunnerMessage, ServerMessage};
#[cfg(target_os = "linux")]
use error::WebSocketError;
#[cfg(target_os = "linux")]
use job::execute_job;
#[cfg(target_os = "linux")]
use state_machine::{ChannelStateMachine, Effect, Input, LogLevel};

#[cfg(target_os = "linux")]
use std::collections::VecDeque;
#[cfg(target_os = "linux")]
use std::sync::{Arc, Mutex};
#[cfg(target_os = "linux")]
use std::time::Duration;
#[cfg(target_os = "linux")]
use websocket::JobChannel;

#[cfg(target_os = "linux")]
const TRANSIENT_RETRY_BASE: Duration = Duration::from_secs(5);
#[cfg(target_os = "linux")]
const TRANSIENT_RETRY_JITTER: u64 = 5;

#[cfg(target_os = "linux")]
fn transient_retry_delay() -> Duration {
    use rand::Rng as _;
    let jitter = rand::rng().random_range(0..=TRANSIENT_RETRY_JITTER);
    TRANSIENT_RETRY_BASE + Duration::from_secs(jitter)
}

/// Global shutdown flag set by signal handler.
/// Async-signal-safe: only uses `AtomicBool::store`.
static SHUTDOWN: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
pub struct UpConfig {
    pub host: Url,
    pub token: bencher_valid::Secret,
    pub runner: String,
    pub poll_timeout_secs: u32,
    pub tuning: TuningConfig,
    /// CPU layout for isolating benchmark cores from housekeeping tasks.
    pub cpu_layout: CpuLayout,
    /// Maximum size in bytes for collected stdout/stderr.
    pub max_output_size: Option<usize>,
    /// Maximum number of output files to decode.
    pub max_file_count: Option<u32>,
    /// Grace period in seconds after exit code before final collection.
    pub grace_period: Option<bencher_json::GracePeriod>,
    /// Firecracker process log level.
    #[cfg(target_os = "linux")]
    pub firecracker_log_level: FirecrackerLogLevel,
}

pub struct Up {
    config: UpConfig,
}

impl Up {
    pub fn new(config: UpConfig) -> Self {
        Self { config }
    }

    #[cfg(target_os = "linux")]
    #[expect(clippy::print_stdout)]
    pub fn run(self) -> Result<(), UpError> {
        install_signal_handlers();

        println!("Bencher Runner starting...");
        println!("  Host: {}", self.config.host);
        println!("  Runner: {}", self.config.runner);
        println!("  Poll timeout: {}s", self.config.poll_timeout_secs);

        // Log CPU layout
        let layout = &self.config.cpu_layout;
        if layout.has_isolation() {
            println!(
                "  CPU isolation: housekeeping={}, benchmark={}",
                layout.housekeeping_cpuset(),
                layout.benchmark_cpuset()
            );
        } else {
            println!("  CPU isolation: disabled (insufficient cores)");
        }

        // Apply host tuning — guard restores settings on drop
        let _tuning_guard = crate::tuning::apply(&self.config.tuning);

        let client = RunnerApiClient::new(
            self.config.host.clone(),
            String::from(self.config.token.clone()),
            self.config.runner.clone(),
        )?;

        let channel_url = client.channel_url()?;

        println!("Connecting to channel...");

        run_driver(&self.config, &channel_url, client.token())
    }

    #[cfg(not(target_os = "linux"))]
    pub fn run(self) -> Result<(), UpError> {
        Err(UpError::Config("Runner requires Linux".to_owned()))
    }
}

/// Effect-driven protocol loop. The state machine decides what to do; this
/// function executes effects and feeds I/O results back.
#[cfg(target_os = "linux")]
#[expect(clippy::print_stdout)]
fn run_driver(config: &UpConfig, channel_url: &Url, token: &str) -> Result<(), UpError> {
    let mut sm = ChannelStateMachine::new(config.poll_timeout_secs);
    let mut effects: VecDeque<Effect> =
        ChannelStateMachine::initial_effects().into_iter().collect();
    let mut ws: Option<Arc<Mutex<JobChannel>>> = None;

    while let Some(effect) = effects.pop_front() {
        if SHUTDOWN.load(Ordering::SeqCst) {
            println!("Shutdown signal received, exiting...");
            effects.clear();
            effects.extend(sm.step(Input::Shutdown));
            continue;
        }

        match execute_effect(effect, config, channel_url, token, &mut ws) {
            EffectResult::Continue => {},
            EffectResult::Input(input) => {
                effects.clear();
                effects.extend(sm.step(input));
            },
            EffectResult::Exit => {
                return if SHUTDOWN.load(Ordering::SeqCst) {
                    Err(UpError::Shutdown)
                } else {
                    Ok(())
                };
            },
        }
    }

    Ok(())
}

#[cfg(target_os = "linux")]
enum EffectResult {
    /// Effect produced no input; continue to next effect.
    Continue,
    /// Effect produced an input; feed it to the state machine.
    Input(Input),
    /// Exit the protocol loop.
    Exit,
}

#[cfg(target_os = "linux")]
#[expect(clippy::print_stdout, clippy::print_stderr)]
fn execute_effect(
    effect: Effect,
    config: &UpConfig,
    channel_url: &Url,
    token: &str,
    ws: &mut Option<Arc<Mutex<JobChannel>>>,
) -> EffectResult {
    match effect {
        Effect::Connect => match JobChannel::connect(channel_url, token) {
            Ok(new_ws) => {
                *ws = Some(Arc::new(Mutex::new(new_ws)));
                EffectResult::Input(Input::Connected)
            },
            Err(e) => {
                println!("WebSocket connection failed: {e}");
                EffectResult::Input(Input::ConnectionFailed)
            },
        },
        Effect::Send(msg) => match try_send(ws.as_ref(), &msg) {
            Ok(()) => EffectResult::Continue,
            Err(e) => {
                eprintln!("Send failed: {e}");
                EffectResult::Input(Input::ConnectionFailed)
            },
        },
        Effect::Receive(timeout) => EffectResult::Input(receive_input(ws.as_ref(), timeout)),
        Effect::WaitForJob(timeout) => {
            EffectResult::Input(wait_for_job_input(ws.as_ref(), timeout))
        },
        Effect::ExecuteJob(job) => {
            let Some(ws_ref) = ws.as_ref() else {
                eprintln!("Error: WS not connected during job execution");
                return EffectResult::Input(Input::ConnectionFailed);
            };
            let result = execute_job(config, &job, ws_ref);
            EffectResult::Input(Input::JobFinished(result))
        },
        Effect::SleepBeforeReconnect => {
            let delay = transient_retry_delay();
            println!("Reconnecting in {} seconds...", delay.as_secs());
            std::thread::sleep(delay);
            EffectResult::Continue
        },
        Effect::Close => {
            if let Some(ws_ref) = ws.as_ref()
                && let Ok(mut ws_guard) = ws_ref.lock()
            {
                ws_guard.close();
            }
            *ws = None;
            EffectResult::Continue
        },
        Effect::ReportOutcome(outcome) => {
            report_outcome(&outcome);
            EffectResult::Continue
        },
        Effect::Log(level, msg) => {
            log_message(level, &msg);
            EffectResult::Continue
        },
        Effect::Exit => EffectResult::Exit,
    }
}

#[cfg(target_os = "linux")]
fn try_send(
    ws: Option<&Arc<Mutex<JobChannel>>>,
    msg: &RunnerMessage,
) -> Result<(), WebSocketError> {
    let ws_ref = ws.ok_or_else(|| WebSocketError::Send("Not connected".to_owned()))?;
    let mut ws_guard = ws_ref
        .lock()
        .map_err(|e| WebSocketError::Send(format!("Lock failed: {e}")))?;
    ws_guard.send_message(msg)
}

#[cfg(target_os = "linux")]
fn receive_input(ws: Option<&Arc<Mutex<JobChannel>>>, timeout: Duration) -> Input {
    let Some(ws_ref) = ws else {
        return Input::ConnectionFailed;
    };
    let Ok(mut ws_guard) = ws_ref.lock() else {
        return Input::ConnectionFailed;
    };
    match ws_guard.read_message_timeout(timeout) {
        Ok(Some(msg)) => Input::Message(msg),
        Ok(None) => Input::ReceiveTimeout,
        Err(_) => Input::ConnectionFailed,
    }
}

#[cfg(target_os = "linux")]
fn wait_for_job_input(ws: Option<&Arc<Mutex<JobChannel>>>, timeout: Duration) -> Input {
    let Some(ws_ref) = ws else {
        return Input::ConnectionFailed;
    };
    let Ok(mut ws_guard) = ws_ref.lock() else {
        return Input::ConnectionFailed;
    };
    match ws_guard.wait_for_job(timeout) {
        Ok(Some(job)) => Input::Message(ServerMessage::Job(Box::new(job))),
        Ok(None) => Input::Message(ServerMessage::NoJob),
        // wait_for_job conflates timeout and connection errors in Err;
        // ConnectionFailed is the safer default — the state machine will
        // reconnect either way, and Close on a dead connection is a no-op.
        Err(_) => Input::ConnectionFailed,
    }
}

#[cfg(target_os = "linux")]
#[expect(clippy::print_stdout, clippy::print_stderr)]
fn report_outcome(outcome: &state_machine::JobOutcome) {
    let state_machine::JobOutcome { job, kind, acked } = outcome;
    match kind {
        state_machine::TerminalKind::Completed { exit_code, output } => {
            println!("Job {job} completed (exit_code={exit_code})");
            if let Some(out) = output {
                let preview: String = out.chars().take(200).collect();
                println!("  Output: {preview}");
            }
        },
        state_machine::TerminalKind::Failed { error } => {
            println!("Job {job} failed: {error}");
        },
        state_machine::TerminalKind::Canceled => {
            println!("Job {job} was canceled");
        },
    }
    if !acked {
        eprintln!("Warning: did not receive server ACK for job {job}");
    }
    println!("Polling for jobs...");
}

#[cfg(target_os = "linux")]
#[expect(clippy::print_stdout, clippy::print_stderr)]
fn log_message(level: LogLevel, msg: &str) {
    match level {
        LogLevel::Info => println!("{msg}"),
        LogLevel::Warn => eprintln!("Warning: {msg}"),
        LogLevel::Error => eprintln!("Error: {msg}"),
    }
}

/// Install signal handlers for SIGINT and SIGTERM.
///
/// The handler sets the global `SHUTDOWN` flag. `AtomicBool::store` is
/// async-signal-safe, so this is safe to call from a signal handler context.
#[cfg(target_os = "linux")]
fn install_signal_handlers() {
    use nix::sys::signal::{SaFlags, SigAction, SigHandler, SigSet, Signal, sigaction};

    let handler = SigHandler::Handler(signal_handler);
    let action = SigAction::new(handler, SaFlags::empty(), SigSet::empty());

    // SAFETY: `signal_handler` only performs `AtomicBool::store` with
    // `Ordering::SeqCst`, which is async-signal-safe per POSIX.
    #[expect(unsafe_code)]
    unsafe {
        let _ = sigaction(Signal::SIGINT, &action);
        let _ = sigaction(Signal::SIGTERM, &action);
    }
}

#[cfg(target_os = "linux")]
extern "C" fn signal_handler(_sig: libc::c_int) {
    SHUTDOWN.store(true, Ordering::SeqCst);
}
