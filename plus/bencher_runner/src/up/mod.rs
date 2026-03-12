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
mod websocket;

pub use error::UpError;

#[cfg(target_os = "linux")]
use api_client::RunnerApiClient;
#[cfg(target_os = "linux")]
use bencher_json::runner::RunnerMessage;
#[cfg(target_os = "linux")]
use error::{ApiClientError, WebSocketError};
#[cfg(target_os = "linux")]
use job::{JobOutcome, execute_job};
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

/// Margin added to `poll_timeout` for the WS read timeout, giving the server
/// time to send `NoJob` after its own deadline.
#[cfg(target_os = "linux")]
const POLL_TIMEOUT_MARGIN_SECS: u64 = 30;

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

        // Track pending terminal messages that weren't ACKed across reconnections
        let mut pending_result: Option<RunnerMessage> = None;

        // Outer loop: connection management with reconnect
        loop {
            if SHUTDOWN.load(Ordering::SeqCst) {
                println!("Shutdown signal received, exiting...");
                return Err(UpError::Shutdown);
            }

            // Connect to persistent WebSocket channel
            let ws = match JobChannel::connect(&channel_url, client.token()) {
                Ok(ws) => ws,
                Err(e) => {
                    let delay = transient_retry_delay();
                    println!("WebSocket connection failed: {e}");
                    println!("Retrying in {} seconds...", delay.as_secs());
                    std::thread::sleep(delay);
                    continue;
                },
            };
            let ws = Arc::new(Mutex::new(ws));

            println!("Channel connected. Polling for jobs...");

            match run_channel_loop(&self.config, &ws, &mut pending_result) {
                Ok(()) => return Ok(()), // Clean shutdown
                Err(UpError::ApiClient(
                    ApiClientError::Unauthorized | ApiClientError::InvalidToken,
                )) => {
                    println!("Authentication failed. Check runner token.");
                    return Err(UpError::ApiClient(ApiClientError::Unauthorized));
                },
                Err(e) => {
                    // Send close frame so the server knows we disconnected intentionally
                    if let Ok(mut ws_guard) = ws.lock() {
                        ws_guard.close();
                    }
                    let delay = transient_retry_delay();
                    println!("Channel error: {e}");
                    println!("Reconnecting in {} seconds...", delay.as_secs());
                    std::thread::sleep(delay);
                },
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn run(self) -> Result<(), UpError> {
        Err(UpError::Config("Runner requires Linux".to_owned()))
    }
}

/// Inner loop: send Ready, wait for Job/NoJob, execute, repeat.
///
/// Returns `Ok(())` on clean shutdown, `Err` on WS or auth errors.
#[cfg(target_os = "linux")]
#[expect(clippy::print_stdout, clippy::print_stderr)]
fn run_channel_loop(
    config: &UpConfig,
    ws: &Arc<Mutex<JobChannel>>,
    pending_result: &mut Option<RunnerMessage>,
) -> Result<(), UpError> {
    loop {
        if SHUTDOWN.load(Ordering::SeqCst) {
            return Ok(());
        }

        // If we have a pending result from a previous job (ACK was not received),
        // resend it before requesting a new job.
        if let Some(msg) = pending_result.take() {
            println!("Resending unACKed terminal message...");
            let mut ws_guard = ws
                .lock()
                .map_err(|e| WebSocketError::Send(format!("Failed to lock WebSocket: {e}")))?;
            ws_guard.send_message(&msg)?;
            let timeout = Duration::from_secs(5);
            match ws_guard.read_message_timeout(timeout) {
                Ok(Some(_)) => {
                    println!("Pending result ACKed by server");
                },
                Ok(None) => {
                    // Timeout without receiving ACK — store back and return error to reconnect
                    eprintln!("Warning: retry ACK timed out");
                    *pending_result = Some(msg);
                    return Err(UpError::WebSocket(WebSocketError::Receive(
                        "ACK timeout".to_owned(),
                    )));
                },
                Err(e) => {
                    // ACK not received again — store back and return error to reconnect
                    eprintln!("Warning: retry ACK not received: {e}");
                    *pending_result = Some(msg);
                    return Err(UpError::WebSocket(WebSocketError::Receive(e.to_string())));
                },
            }
        }

        // Send Ready to request a job
        {
            let mut ws_guard = ws
                .lock()
                .map_err(|e| WebSocketError::Send(format!("Failed to lock WebSocket: {e}")))?;
            ws_guard.send_ready(config.poll_timeout_secs)?;
        }

        // Wait for Job or NoJob from server
        let timeout =
            Duration::from_secs(u64::from(config.poll_timeout_secs) + POLL_TIMEOUT_MARGIN_SECS);
        let job = {
            let mut ws_guard = ws
                .lock()
                .map_err(|e| WebSocketError::Send(format!("Failed to lock WebSocket: {e}")))?;
            ws_guard.wait_for_job(timeout)?
        };

        if let Some(job) = job {
            let job_uuid = job.uuid;
            println!("Received job: {job_uuid}");

            match execute_job(config, &job, ws) {
                Ok(JobOutcome::Completed {
                    exit_code,
                    output,
                    acked,
                    msg,
                }) => {
                    println!("Job {job_uuid} completed (exit_code={exit_code})");
                    if let Some(out) = &output {
                        let preview: String = out.chars().take(200).collect();
                        println!("  Output: {preview}");
                    }
                    if !acked {
                        *pending_result = Some(msg);
                    }
                },
                Ok(JobOutcome::Failed { error, acked, msg }) => {
                    println!("Job {job_uuid} failed: {error}");
                    if !acked {
                        *pending_result = Some(msg);
                    }
                },
                Ok(JobOutcome::Canceled { acked, msg }) => {
                    println!("Job {job_uuid} was canceled");
                    if !acked {
                        *pending_result = Some(msg);
                    }
                },
                Err(e) => {
                    println!("Job {job_uuid} error: {e}");
                    return Err(e); // WS likely broken, reconnect
                },
            }

            println!("Polling for jobs...");
        }
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
