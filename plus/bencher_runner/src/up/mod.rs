use std::sync::atomic::{AtomicBool, Ordering};

use bencher_json::RunnerResourceId;
use url::Url;

use crate::cpu::CpuLayout;
use crate::log_level::SandboxLogLevel;
use crate::tuning::TuningConfig;

mod api_client;
mod error;
mod job;
mod state_machine;
mod websocket;

pub use error::UpError;

use api_client::RunnerApiClient;
use bencher_json::runner::{RunnerMessage, ServerMessage};
use error::{SelfUpdateError, WebSocketError};
use job::execute_job;
use state_machine::{ChannelStateMachine, Effect, Input, LogLevel};

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use websocket::JobChannel;

const TRANSIENT_RETRY_BASE: Duration = Duration::from_secs(5);
const TRANSIENT_RETRY_JITTER: u64 = 5;

fn transient_retry_delay() -> Duration {
    use rand::RngExt as _;
    let jitter = rand::rng().random_range(0..=TRANSIENT_RETRY_JITTER);
    TRANSIENT_RETRY_BASE + Duration::from_secs(jitter)
}

/// Global shutdown flag set by signal handler.
/// Async-signal-safe: only uses `AtomicBool::store`.
static SHUTDOWN: AtomicBool = AtomicBool::new(false);

#[derive(Debug)]
pub struct UpConfig {
    pub host: Url,
    pub key: bencher_valid::Secret,
    pub runner: RunnerResourceId,
    pub poll_timeout_secs: u32,
    pub tuning: TuningConfig,
    /// CPU layout for isolating benchmark cores from housekeeping tasks.
    /// `None` until tuning is applied at runtime (so SMT state is reflected).
    pub cpu_layout: Option<CpuLayout>,
    /// Maximum size in bytes for collected stdout/stderr.
    pub max_output_size: Option<usize>,
    /// Maximum number of output files to decode.
    pub max_file_count: Option<u32>,
    /// Maximum number of symlinks to follow during path resolution.
    pub max_symlinks: Option<u32>,
    /// Grace period in seconds after exit code before final collection.
    pub grace_period: Option<bencher_json::GracePeriod>,
    /// Sandbox process log level.
    pub sandbox_log_level: SandboxLogLevel,
    /// Whether to allow non-sandboxed execution.
    pub allow_no_sandbox: bool,
    /// Disable auto-update: do not send runner metadata to the server.
    pub no_auto_update: bool,
    /// Maximum download size in bytes for self-update binaries.
    pub max_download_size: Option<u64>,
}

pub struct Up {
    config: UpConfig,
}

impl Up {
    pub fn new(config: UpConfig) -> Self {
        Self { config }
    }

    #[expect(clippy::print_stdout)]
    #[cfg_attr(
        not(target_os = "linux"),
        expect(unused_mut, reason = "mut needed on Linux for CPU layout detection")
    )]
    pub fn run(mut self) -> Result<(), UpError> {
        install_signal_handlers();

        println!("Bencher Runner starting...");
        println!("  Host: {}", self.config.host);
        println!("  Runner: {}", self.config.runner);
        println!("  Poll timeout: {}s", self.config.poll_timeout_secs);
        if self.config.allow_no_sandbox {
            println!("  Non-sandboxed execution: allowed");
        }

        // Apply host tuning — guard restores settings on drop (no-op on non-Linux).
        // This must happen before CPU layout detection so that SMT changes
        // are reflected in the core count.
        let _tuning_guard = crate::tuning::apply(&self.config.tuning);

        // Re-detect CPU layout after tuning (SMT may have changed core count).
        // Linux-only: CpuLayout::detect() reads /sys/devices which only exists on Linux.
        #[cfg(target_os = "linux")]
        {
            self.config.cpu_layout = Some(CpuLayout::detect());
            if let Some(cpu_layout) = &self.config.cpu_layout {
                if cpu_layout.has_isolation() {
                    println!(
                        "  CPU isolation: housekeeping={}, benchmark={}",
                        cpu_layout.housekeeping_cpuset(),
                        cpu_layout.benchmark_cpuset()
                    );
                } else {
                    println!("  CPU isolation: disabled (insufficient cores)");
                }
            }
        }

        let client = RunnerApiClient::new(
            self.config.host.clone(),
            String::from(self.config.key.clone()),
            self.config.runner.clone(),
        )?;

        let channel_url = client.channel_url()?;

        println!("Connecting to channel...");

        run_driver(&self.config, &channel_url, client.key())
    }
}

/// Effect-driven protocol loop. The state machine decides what to do; this
/// function executes effects and feeds I/O results back.
#[expect(clippy::print_stdout)]
fn run_driver(config: &UpConfig, channel_url: &Url, key: &str) -> Result<(), UpError> {
    let runner_metadata = if config.no_auto_update {
        None
    } else {
        bencher_valid::OperatingSystem::from_host()
            .ok()
            .zip(bencher_valid::Architecture::from_host().ok())
            .map(|(os, arch)| bencher_json::runner::JsonRunnerMetadata {
                os,
                arch,
                version: env!("CARGO_PKG_VERSION").to_owned(),
            })
    };
    let mut sm = ChannelStateMachine::new(config.poll_timeout_secs, runner_metadata);
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

        match execute_effect(effect, config, channel_url, key, &mut ws) {
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

enum EffectResult {
    /// Effect produced no input; continue to next effect.
    Continue,
    /// Effect produced an input; feed it to the state machine.
    Input(Input),
    /// Exit the protocol loop.
    Exit,
}

#[expect(clippy::print_stdout, clippy::print_stderr)]
fn execute_effect(
    effect: Effect,
    config: &UpConfig,
    channel_url: &Url,
    key: &str,
    ws: &mut Option<Arc<Mutex<JobChannel>>>,
) -> EffectResult {
    match effect {
        Effect::Connect => match JobChannel::connect(channel_url, key) {
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
        Effect::SleepBeforeReconnect(reason) => {
            let delay = transient_retry_delay();
            println!("Reconnecting in {} seconds ({reason})...", delay.as_secs());
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
        Effect::SelfUpdate {
            version,
            url,
            checksum,
        } => match self_update(&version, &url, &checksum, config.max_download_size) {
            Ok(()) => EffectResult::Exit,
            Err(e) => {
                eprintln!("Self-update failed: {e}");
                EffectResult::Continue
            },
        },
    }
}

fn try_send(
    ws: Option<&Arc<Mutex<JobChannel>>>,
    msg: &RunnerMessage,
) -> Result<(), WebSocketError> {
    let ws_ref = ws.ok_or(WebSocketError::NotConnected)?;
    let mut ws_guard = ws_ref
        .lock()
        .map_err(|_poison| WebSocketError::LockPoisoned)?;
    ws_guard.send_message(msg)
}

#[expect(clippy::print_stderr)]
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
        Err(e) => {
            eprintln!("Connection lost: {e}");
            Input::ConnectionFailed
        },
    }
}

#[expect(clippy::print_stderr)]
fn wait_for_job_input(ws: Option<&Arc<Mutex<JobChannel>>>, timeout: Duration) -> Input {
    let Some(ws_ref) = ws else {
        return Input::ConnectionFailed;
    };
    let Ok(mut ws_guard) = ws_ref.lock() else {
        return Input::ConnectionFailed;
    };
    match ws_guard.wait_for_job(timeout) {
        Ok(websocket::WaitResult::Job(job)) => Input::Message(ServerMessage::Job(job)),
        Ok(websocket::WaitResult::NoJob) => Input::Message(ServerMessage::NoJob),
        Ok(websocket::WaitResult::Update(msg)) => Input::Message(msg),
        // wait_for_job conflates timeout and connection errors in Err;
        // ConnectionFailed is the safer default — the state machine will
        // reconnect either way, and Close on a dead connection is a no-op.
        Err(e) => {
            eprintln!("Connection lost: {e}");
            Input::ConnectionFailed
        },
    }
}

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

/// Install signal handlers for SIGINT and SIGTERM (non-Linux POSIX).
///
/// Uses `libc::signal()` directly since `nix` is not available on macOS.
#[cfg(not(target_os = "linux"))]
fn install_signal_handlers() {
    // SAFETY: `signal_handler` only performs `AtomicBool::store` with
    // `Ordering::SeqCst`, which is async-signal-safe per POSIX.
    #[expect(unsafe_code, clippy::fn_to_numeric_cast_any)]
    unsafe {
        libc::signal(libc::SIGINT, signal_handler as libc::sighandler_t);
        libc::signal(libc::SIGTERM, signal_handler as libc::sighandler_t);
    }
}

extern "C" fn signal_handler(_sig: libc::c_int) {
    SHUTDOWN.store(true, Ordering::SeqCst);
}

const DEFAULT_MAX_DOWNLOAD_SIZE: u64 = 500 * 1024 * 1024;

#[expect(clippy::print_stdout)]
fn self_update(
    version: &str,
    url: &Url,
    checksum: &bencher_valid::Sha256,
    max_download_size: Option<u64>,
) -> Result<(), SelfUpdateError> {
    #[cfg(not(unix))]
    {
        return Err(SelfUpdateError::UnsupportedPlatform);
    }

    #[cfg(unix)]
    {
        use sha2::Digest as _;
        use std::io::{BufWriter, Read as _, Write as _};
        use std::os::unix::fs::PermissionsExt as _;
        use std::os::unix::process::CommandExt as _;

        println!("Updating to version {version}...");
        println!("  Downloading: {url}");

        let current_exe = std::env::current_exe().map_err(SelfUpdateError::CurrentExe)?;
        let new_path = current_exe.with_extension("new");
        let old_path = current_exe.with_extension("old");

        let response = ureq::get(url.as_str())
            .call()
            .map_err(SelfUpdateError::Http)?;

        let limit = max_download_size.unwrap_or(DEFAULT_MAX_DOWNLOAD_SIZE);
        let mut reader = response.into_body().into_reader();
        let mut file =
            BufWriter::new(std::fs::File::create(&new_path).map_err(SelfUpdateError::FileOp)?);
        let mut hasher = sha2::Sha256::new();
        let mut downloaded: u64 = 0;
        let mut buf = [0u8; 8192];
        loop {
            let n = reader.read(&mut buf).map_err(SelfUpdateError::Download)?;
            if n == 0 {
                break;
            }
            downloaded += n as u64;
            if downloaded > limit {
                drop(file);
                let _remove_new = std::fs::remove_file(&new_path);
                return Err(SelfUpdateError::DownloadTooLarge { limit, downloaded });
            }
            let chunk = buf.get(..n).ok_or_else(|| {
                SelfUpdateError::Download(std::io::Error::other(
                    "read returned byte count exceeding buffer",
                ))
            })?;
            hasher.update(chunk);
            file.write_all(chunk).map_err(SelfUpdateError::FileOp)?;
        }
        file.flush().map_err(SelfUpdateError::FileOp)?;
        drop(file);

        let digest = hasher.finalize();
        let actual_hex = format!("{digest:x}");
        let actual: bencher_valid::Sha256 =
            actual_hex.parse().map_err(SelfUpdateError::ChecksumParse)?;
        if actual != *checksum {
            let _remove_new = std::fs::remove_file(&new_path);
            return Err(SelfUpdateError::Checksum {
                expected: checksum.clone(),
                actual,
            });
        }
        println!("  Checksum verified: {checksum}");

        std::fs::set_permissions(&new_path, std::fs::Permissions::from_mode(0o755)).map_err(
            |e| {
                let _remove_new = std::fs::remove_file(&new_path);
                SelfUpdateError::FileOp(e)
            },
        )?;

        if old_path.exists() {
            let _remove_old = std::fs::remove_file(&old_path);
        }
        std::fs::rename(&current_exe, &old_path).map_err(|e| {
            let _remove_new = std::fs::remove_file(&new_path);
            SelfUpdateError::FileOp(e)
        })?;
        std::fs::rename(&new_path, &current_exe).map_err(|e| {
            let _restore_current = std::fs::rename(&old_path, &current_exe);
            SelfUpdateError::FileOp(e)
        })?;

        println!("  Binary updated. Restarting...");

        let args: Vec<String> = std::env::args().skip(1).collect();
        let err = std::process::Command::new(&current_exe).args(&args).exec();
        let _restore_current = std::fs::rename(&old_path, &current_exe);
        Err(SelfUpdateError::Exec(err))
    }
}
