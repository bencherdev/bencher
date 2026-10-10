use bencher_json::RunnerResourceId;
use bencher_json::runner::PauseReason;
use camino::Utf8Path;
use slog::{Logger, error, info, warn};
use url::Url;

use crate::cpu::CpuLayout;
use crate::host::md;
use crate::log_level::SandboxLogLevel;
use crate::tuning::{TuningConfig, preflight};

mod api_client;
mod error;
mod job;
mod state_machine;
mod websocket;

pub use error::UpError;

use api_client::RunnerApiClient;
use bencher_json::runner::{RunnerMessage, ServerMessage};
use error::{SelfChecksumError, SelfUpdateError, WebSocketError};
use job::execute_job;
use state_machine::{ChannelStateMachine, Effect, Input, LogLevel, Probe};

use std::collections::VecDeque;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use websocket::JobChannel;

const TRANSIENT_RETRY_BASE: Duration = Duration::from_secs(5);
const TRANSIENT_RETRY_JITTER: u64 = 5;

fn transient_retry_delay() -> Duration {
    use rand::RngExt as _;
    let jitter = rand::rng().random_range(0..=TRANSIENT_RETRY_JITTER);
    TRANSIENT_RETRY_BASE + Duration::from_secs(jitter)
}

#[derive(Debug)]
pub struct UpConfig {
    pub host: Url,
    pub key: bencher_valid::RunnerKey,
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
    /// Sandbox process log level.
    pub sandbox_log_level: SandboxLogLevel,
    /// Whether to allow non-sandboxed execution.
    pub allow_no_sandbox: bool,
    /// Take no Job while an md array syncs, rebuilds, or scrubs.
    pub raid_pause: bool,
    /// Disable auto-update: do not send runner metadata to the server.
    pub no_auto_update: bool,
    /// Update channel for automatic updates.
    pub update_channel: bencher_valid::UpdateChannel,
    /// Maximum download size in bytes for self-update binaries.
    pub max_download_size: Option<u64>,
    pub state_dir: camino::Utf8PathBuf,
    pub jail_user: crate::jail::JailUser,
}

pub struct Up {
    config: UpConfig,
}

impl Up {
    pub fn new(config: UpConfig) -> Self {
        Self { config }
    }

    #[cfg_attr(
        not(target_os = "linux"),
        expect(unused_mut, reason = "mut needed on Linux for CPU layout detection")
    )]
    pub fn run(mut self, log: &Logger) -> Result<(), UpError> {
        crate::signal::install_handlers();

        // The host is prepared on demand, not here, because a runner serving
        // only non-sandboxed specs must come up without root.
        info!(log, "Runner starting";
            "version" => bencher_json::BENCHER_API_VERSION,
            "host" => self.config.host.as_str(),
            "runner" => %self.config.runner,
            "poll_timeout_secs" => self.config.poll_timeout_secs,
            "state_dir" => self.config.state_dir.as_str(),
            "allow_no_sandbox" => self.config.allow_no_sandbox,
            "raid_pause" => self.config.raid_pause,
        );

        #[cfg(target_os = "linux")]
        let runner_lock =
            crate::runner_lock::RunnerLock::acquire(log).map_err(crate::RunnerError::from)?;
        #[cfg(target_os = "linux")]
        // A runner that may run Jobs with no sandbox still serves those on a
        // kernel without `cgroup.kill`.
        crate::jail::prepare_at_startup(log, &runner_lock, !self.config.allow_no_sandbox)
            .map_err(crate::RunnerError::from)?;

        // Warn about host conditions that limit benchmark accuracy (Linux only)
        preflight::log_host_warnings(log);

        // Apply host tuning, which persists until the host reboots (no-op on
        // non-Linux). This must happen before CPU layout detection so that SMT
        // changes are reflected in the core count.
        let _tuning_guard = crate::tuning::apply(log, &self.config.tuning);

        // Re-detect CPU layout after tuning (SMT may have changed core count).
        // Linux-only: CpuLayout::detect() reads /sys/devices which only exists on Linux.
        #[cfg(target_os = "linux")]
        {
            let cpu_layout = CpuLayout::detect(log);
            crate::tuning::apply_cpu_scoped(log, &self.config.tuning, &cpu_layout);
            cpu_layout.log_isolation(log);
            self.config.cpu_layout = Some(cpu_layout);
        }

        let client = RunnerApiClient::new(
            self.config.host.clone(),
            self.config.key.clone(),
            self.config.runner.clone(),
        );

        let channel_url = client.channel_url()?;

        info!(log, "Connecting to channel");

        run_driver(log, &self.config, &channel_url, client.key())
    }
}

/// Effect-driven protocol loop. The state machine decides what to do; this
/// function executes effects and feeds I/O results back.
fn run_driver(
    log: &Logger,
    config: &UpConfig,
    channel_url: &Url,
    key: &str,
) -> Result<(), UpError> {
    // Only the canary channel converges by checksum, so stable runners skip
    // the full-binary read and keep their wire format checksum-free.
    let checksum =
        if config.no_auto_update || config.update_channel != bencher_valid::UpdateChannel::Canary {
            None
        } else {
            match self_checksum() {
                Ok(checksum) => Some(checksum),
                Err(e) => {
                    warn!(log, "Runner checksum failed"; "error" => %e);
                    None
                },
            }
        };
    let runner_metadata =
        build_runner_metadata(config.no_auto_update, config.update_channel, checksum);
    if let Some(channel) = runner_metadata
        .as_ref()
        .and_then(|metadata| metadata.channel)
    {
        info!(log, "Update channel"; "channel" => %channel);
    }
    let mut host = crate::jail::HostPreparation::new();
    let mut sm = ChannelStateMachine::new(config.poll_timeout_secs, runner_metadata);
    let mut ws: Option<Arc<Mutex<JobChannel>>> = None;

    drive(log, &mut sm, crate::signal::stop_requested, |effect| {
        execute_effect(
            log,
            effect,
            config,
            channel_url,
            key,
            &mut ws,
            &mut host,
            crate::signal::stop_requested,
        )
    })
}

/// Executes the machine's effects and feeds their inputs back until it exits.
///
/// A stop reaches the machine once, ahead of the next effect, which still runs
/// unless the machine exits first, so a running Job's result can go out.
fn drive<S, F>(
    log: &Logger,
    sm: &mut ChannelStateMachine,
    stopped: S,
    mut execute: F,
) -> Result<(), UpError>
where
    S: Fn() -> bool,
    F: FnMut(Effect) -> EffectResult,
{
    let mut effects: VecDeque<Effect> =
        ChannelStateMachine::initial_effects().into_iter().collect();
    let mut stop_fed = false;
    while let Some(effect) = effects.pop_front() {
        if !stop_fed && stopped() {
            stop_fed = true;
            info!(log, "Shutdown signal received");
            effects.push_front(effect);
            for shutdown in sm.step(Input::Shutdown).into_iter().rev() {
                effects.push_front(shutdown);
            }
            continue;
        }

        match execute(effect) {
            EffectResult::Continue => {},
            EffectResult::Input(input) => {
                effects.clear();
                effects.extend(sm.step(input));
            },
            EffectResult::Exit => {
                return if stopped() {
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

#[expect(
    clippy::too_many_arguments,
    reason = "the driver's state, borrowed for one effect"
)]
fn execute_effect(
    log: &Logger,
    effect: Effect,
    config: &UpConfig,
    channel_url: &Url,
    key: &str,
    ws: &mut Option<Arc<Mutex<JobChannel>>>,
    host: &mut crate::jail::HostPreparation,
    stopped: impl Fn() -> bool,
) -> EffectResult {
    match effect {
        Effect::Connect => match JobChannel::connect(channel_url, key) {
            Ok(new_ws) => {
                *ws = Some(Arc::new(Mutex::new(new_ws)));
                EffectResult::Input(Input::Connected)
            },
            Err(e) => {
                warn!(log, "Channel connection failed"; "error" => bencher_logger::capped(&e));
                EffectResult::Input(Input::ConnectionFailed)
            },
        },
        Effect::Send(msg) => match try_send(ws.as_ref(), &msg) {
            Ok(()) => EffectResult::Continue,
            Err(e) => {
                warn!(log, "Channel send failed"; "error" => bencher_logger::capped(&e));
                EffectResult::Input(Input::ConnectionFailed)
            },
        },
        Effect::Probe => EffectResult::Input(Input::Probed(probe(
            Utf8Path::new(crate::host::SYSFS),
            config.raid_pause,
        ))),
        Effect::PauseBegan(reasons) => {
            log_pause_began(log, &reasons);
            EffectResult::Continue
        },
        Effect::PauseEnded => {
            info!(log, "Pause ended");
            EffectResult::Continue
        },
        Effect::HoldOff(hold) => {
            info!(log, "Holding off the reconnect while paused"; "hold_secs" => hold.as_secs());
            sleep_unless_stopped(hold, stopped);
            EffectResult::Continue
        },
        Effect::Receive(timeout) => EffectResult::Input(receive_input(log, ws.as_ref(), timeout)),
        Effect::WaitForJob(timeout) => {
            EffectResult::Input(wait_for_job_input(log, ws.as_ref(), timeout))
        },
        Effect::ExecuteJob(job) => {
            let Some(ws_ref) = ws.as_ref() else {
                error!(log, "Channel not connected for the Job"; "job" => %job.uuid);
                return EffectResult::Input(Input::ConnectionFailed);
            };
            let result = execute_job(log, config, &job, ws_ref, host);
            EffectResult::Input(Input::JobFinished(result))
        },
        Effect::SleepBeforeReconnect(reason) => {
            let delay = transient_retry_delay();
            info!(log, "Reconnecting"; "delay_secs" => delay.as_secs(), "reason" => %reason);
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
            report_outcome(log, &outcome);
            EffectResult::Continue
        },
        Effect::Log(level, msg) => {
            log_message(log, level, &msg);
            EffectResult::Continue
        },
        Effect::SelfUpdate {
            version,
            url,
            checksum,
        } => match self_update(log, &version, &url, &checksum, config.max_download_size) {
            Ok(()) => EffectResult::Exit,
            Err(e) => {
                warn!(log, "Self-update failed"; "error" => bencher_logger::capped(&e));
                EffectResult::Input(Input::SelfUpdateFailed)
            },
        },
        Effect::Exit => EffectResult::Exit,
    }
}

/// What the host says about taking a Job, read at the idle decision point.
fn probe(sysfs: &Utf8Path, raid_pause: bool) -> Probe {
    let reasons = if raid_pause {
        md::raid_reasons(&md::arrays(sysfs))
    } else {
        Vec::new()
    };
    Probe {
        reasons,
        now: bencher_json::DateTime::now(),
    }
}

fn log_pause_began(log: &Logger, reasons: &[PauseReason]) {
    for reason in reasons {
        match reason {
            PauseReason::Raid {
                array,
                action,
                done,
                total,
                speed,
            } => {
                let percent = done
                    .zip(*total)
                    .and_then(|(done, total)| done.saturating_mul(100).checked_div(total));
                info!(log, "Paused for a RAID sync";
                    "array" => array,
                    "action" => ?action,
                    "percent" => percent,
                    "speed_kib" => speed,
                );
            },
            PauseReason::Maintenance { marker, lock } => {
                info!(log, "Paused for host maintenance"; "marker" => marker, "lock" => lock);
            },
            PauseReason::Other => {},
        }
    }
}

/// A stop ends the wait within a second, since a paused hold lasts a whole poll.
fn sleep_unless_stopped(duration: Duration, stopped: impl Fn() -> bool) {
    let deadline = Instant::now() + duration;
    while !stopped() {
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            break;
        }
        std::thread::sleep(left.min(Duration::from_secs(1)));
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

fn receive_input(log: &Logger, ws: Option<&Arc<Mutex<JobChannel>>>, timeout: Duration) -> Input {
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
            warn!(log, "Channel connection lost"; "error" => bencher_logger::capped(&e));
            Input::ConnectionFailed
        },
    }
}

fn wait_for_job_input(
    log: &Logger,
    ws: Option<&Arc<Mutex<JobChannel>>>,
    timeout: Duration,
) -> Input {
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
            warn!(log, "Channel connection lost"; "error" => bencher_logger::capped(&e));
            Input::ConnectionFailed
        },
    }
}

fn report_outcome(log: &Logger, outcome: &state_machine::JobOutcome) {
    let state_machine::JobOutcome { job, kind, acked } = outcome;
    match kind {
        state_machine::TerminalKind::Completed {
            exit_code,
            stdout_bytes,
            stderr_bytes,
        } => {
            info!(log, "Job completed";
                "job" => %job,
                "exit_code" => *exit_code,
                "stdout_bytes" => *stdout_bytes,
                "stderr_bytes" => *stderr_bytes,
            );
        },
        state_machine::TerminalKind::Failed { error } => {
            info!(log, "Job failed"; "job" => %job, "error" => bencher_logger::capped(error));
        },
        state_machine::TerminalKind::Canceled => {
            info!(log, "Job canceled"; "job" => %job);
        },
    }
    if !acked {
        warn!(log, "No server ACK for the Job"; "job" => %job);
    }
    info!(log, "Polling for jobs");
}

/// The state machine's text, which can quote a server message, goes in a
/// capped field under one fixed `msg`.
fn log_message(log: &Logger, level: LogLevel, msg: &str) {
    let detail = bencher_logger::capped(msg);
    match level {
        LogLevel::Info => info!(log, "Channel"; "detail" => detail),
        LogLevel::Warn => warn!(log, "Channel"; "detail" => detail),
        LogLevel::Error => error!(log, "Channel"; "detail" => detail),
    }
}

const DEFAULT_MAX_DOWNLOAD_SIZE: u64 = 500 * 1024 * 1024;

struct CleanupGuard {
    path: std::path::PathBuf,
    armed: bool,
}

impl CleanupGuard {
    fn new(path: std::path::PathBuf) -> Self {
        Self { path, armed: true }
    }

    fn defuse(&mut self) {
        self.armed = false;
    }
}

impl Drop for CleanupGuard {
    fn drop(&mut self) {
        if self.armed {
            let _remove = std::fs::remove_file(&self.path);
        }
    }
}

fn self_update(
    log: &Logger,
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

        info!(log, "Updating";
            "version" => bencher_logger::capped(version),
            "url" => bencher_logger::capped(url),
        );

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
        let mut cleanup = CleanupGuard::new(new_path.clone());
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
        let actual_hex = hex::encode(digest);
        let actual: bencher_valid::Sha256 =
            actual_hex.parse().map_err(SelfUpdateError::ChecksumParse)?;
        if actual != *checksum {
            return Err(SelfUpdateError::Checksum {
                expected: checksum.clone(),
                actual,
            });
        }
        info!(log, "Update checksum verified"; "checksum" => %checksum);

        std::fs::set_permissions(&new_path, std::fs::Permissions::from_mode(0o755))
            .map_err(SelfUpdateError::FileOp)?;

        cleanup.defuse();

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

        info!(log, "Update installed, restarting");

        let args: Vec<String> = std::env::args().skip(1).collect();
        let err = std::process::Command::new(&current_exe).args(&args).exec();
        let _restore_current = std::fs::rename(&old_path, &current_exe);
        Err(SelfUpdateError::Exec(err))
    }
}

/// Build the runner metadata sent with `Ready` messages, or `None` when
/// auto-update is disabled. The stable channel is sent as absent, with no
/// checksum, so the wire format matches pre-channel runners; only the canary
/// channel reports a checksum, since only it converges by content.
fn build_runner_metadata(
    no_auto_update: bool,
    update_channel: bencher_valid::UpdateChannel,
    checksum: Option<bencher_valid::Sha256>,
) -> Option<bencher_json::runner::JsonRunnerMetadata> {
    if no_auto_update {
        return None;
    }
    let (channel, checksum) = match update_channel {
        bencher_valid::UpdateChannel::Stable => (None, None),
        channel @ bencher_valid::UpdateChannel::Canary => (Some(channel), checksum),
    };
    bencher_valid::OperatingSystem::from_host()
        .ok()
        .zip(bencher_valid::Architecture::from_host().ok())
        .map(|(os, arch)| bencher_json::runner::JsonRunnerMetadata {
            os,
            arch,
            version: bencher_json::BENCHER_API_VERSION.to_owned(),
            channel,
            checksum,
        })
}

/// Compute the SHA-256 checksum of the currently running binary.
///
/// Reported to the server so canary channel runners converge on the published
/// build by content rather than by version number.
fn self_checksum() -> Result<bencher_valid::Sha256, SelfChecksumError> {
    let current_exe = std::env::current_exe().map_err(SelfChecksumError::CurrentExe)?;
    file_checksum(&current_exe)
}

/// Compute the SHA-256 checksum of a file by streaming its contents.
fn file_checksum(path: &std::path::Path) -> Result<bencher_valid::Sha256, SelfChecksumError> {
    use sha2::Digest as _;
    use std::io::Read as _;

    let mut file = std::fs::File::open(path).map_err(SelfChecksumError::Read)?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = [0u8; 8192];
    loop {
        let n = file.read(&mut buf).map_err(SelfChecksumError::Read)?;
        if n == 0 {
            break;
        }
        let chunk = buf.get(..n).ok_or_else(|| {
            SelfChecksumError::Read(std::io::Error::other(
                "read returned byte count exceeding buffer",
            ))
        })?;
        hasher.update(chunk);
    }
    hex::encode(hasher.finalize())
        .parse()
        .map_err(SelfChecksumError::Parse)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicBool, Ordering};

    use super::*;

    #[test]
    fn file_checksum_known_contents() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("checksum-test");
        std::fs::write(&path, b"hello world").unwrap();

        let checksum = file_checksum(&path).unwrap();
        // Precomputed: sha256("hello world")
        let expected: bencher_valid::Sha256 =
            "b94d27b9934d3e08a52e52d7da7dabfac484efe37a5380ee9088f7ace2efcde9"
                .parse()
                .unwrap();
        assert_eq!(checksum, expected);
    }

    #[test]
    fn self_checksum_matches_current_exe() {
        let current_exe = std::env::current_exe().unwrap();
        assert_eq!(
            self_checksum().unwrap(),
            file_checksum(&current_exe).unwrap()
        );
    }

    #[test]
    fn metadata_none_when_no_auto_update() {
        assert!(build_runner_metadata(true, bencher_valid::UpdateChannel::Canary, None).is_none());
    }

    #[test]
    fn metadata_stable_channel_is_absent() {
        let metadata =
            build_runner_metadata(false, bencher_valid::UpdateChannel::Stable, None).unwrap();
        assert!(metadata.channel.is_none());
        assert!(metadata.checksum.is_none());
        assert_eq!(metadata.version, bencher_json::BENCHER_API_VERSION);
    }

    #[test]
    fn metadata_stable_channel_drops_checksum() {
        // Stable runners must stay checksum-free on the wire, even if a
        // checksum was computed.
        let checksum: bencher_valid::Sha256 =
            "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"
                .parse()
                .unwrap();
        let metadata =
            build_runner_metadata(false, bencher_valid::UpdateChannel::Stable, Some(checksum))
                .unwrap();
        assert!(metadata.channel.is_none());
        assert!(metadata.checksum.is_none());
    }

    #[test]
    fn metadata_canary_channel_with_checksum() {
        let checksum: bencher_valid::Sha256 =
            "a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"
                .parse()
                .unwrap();
        let metadata = build_runner_metadata(
            false,
            bencher_valid::UpdateChannel::Canary,
            Some(checksum.clone()),
        )
        .unwrap();
        assert_eq!(metadata.channel, Some(bencher_valid::UpdateChannel::Canary));
        assert_eq!(metadata.checksum, Some(checksum));
    }

    #[test]
    fn no_raid_pause_takes_jobs_through_a_sync() {
        // Kills a probe that pauses for a sync despite `--no-raid-pause`, or
        // never pauses for one.
        let (_dir, sysfs) = md::tests::sysfs(&[("md0", "check", "1 / 2", "100")]);
        assert_eq!(probe(&sysfs, true).reasons.len(), 1);
        assert_eq!(probe(&sysfs, false).reasons, []);
    }

    #[test]
    fn a_paused_reconnect_holds_off_for_the_whole_hold() {
        // Kills a hold that does not sleep, which reconnects a paused runner
        // every few seconds.
        let hold = Duration::from_millis(300);
        let started = Instant::now();
        execute(Effect::HoldOff(hold), &mut host(), || false);
        assert!(started.elapsed() >= hold, "{:?}", started.elapsed());
    }

    #[test]
    fn a_stop_ends_a_hold_within_one_slice() {
        // Kills a hold that a stop cannot end, which keeps a stopped runner up
        // for a whole poll.
        let asked = std::cell::Cell::new(false);
        let started = Instant::now();
        execute(
            Effect::HoldOff(Duration::from_secs(10)),
            &mut host(),
            || asked.replace(true),
        );
        let elapsed = started.elapsed();
        assert!(
            (Duration::from_secs(1)..Duration::from_secs(3)).contains(&elapsed),
            "{elapsed:?}"
        );
    }

    /// Runs one effect as the driver does, with no channel.
    fn execute(
        effect: Effect,
        host: &mut crate::jail::HostPreparation,
        stopped: impl Fn() -> bool,
    ) -> EffectResult {
        execute_effect(
            &crate::log::discard(),
            effect,
            &job::tests::test_up_config(),
            &Url::parse("ws://localhost/").unwrap(),
            "",
            &mut None,
            host,
            stopped,
        )
    }

    fn host() -> crate::jail::HostPreparation {
        crate::jail::HostPreparation::new()
    }

    // --- drive ---

    #[test]
    fn a_stop_during_a_job_still_sends_its_result() {
        // Prevents the stop replacing the Job's result with an exit, which
        // leaves the server to find out only when the Job's heartbeat times out.
        let stop = AtomicBool::new(false);
        let mut sm = ChannelStateMachine::new(30, None);
        let mut job = Some(state_machine::test_claimed_job());
        let job_uuid = state_machine::test_claimed_job().uuid;
        let mut sent = Vec::new();
        let mut steps = 0;
        let checks = std::cell::Cell::new(0);

        let stopped = || {
            // Bounded too, so a driver that loops without running an effect fails here.
            checks.set(checks.get() + 1);
            assert!(
                checks.get() < 100,
                "the driver never exited, checking for a stop"
            );
            stop.load(Ordering::SeqCst)
        };
        let result = drive(&crate::log::discard(), &mut sm, stopped, |effect| {
            steps += 1;
            assert!(steps < 100, "the driver never exited, sent: {sent:?}");
            match effect {
                Effect::Connect => EffectResult::Input(Input::Connected),
                Effect::WaitForJob(_) => EffectResult::Input(Input::Message(
                    job.take().map_or(ServerMessage::NoJob, ServerMessage::Job),
                )),
                Effect::ExecuteJob(_) => {
                    // The runner is stopped mid-Job, which ends the Job.
                    stop.store(true, Ordering::SeqCst);
                    EffectResult::Input(Input::JobFinished(
                        state_machine::JobFinishResult::Failed {
                            error: "the runner stopped".to_owned(),
                            results: Vec::new(),
                        },
                    ))
                },
                Effect::Send(msg) => {
                    sent.push(msg);
                    EffectResult::Continue
                },
                Effect::Receive(_) => EffectResult::Input(Input::Message(ServerMessage::Ack {
                    job: Some(job_uuid),
                })),
                Effect::Probe => EffectResult::Input(Input::Probed(Probe {
                    reasons: Vec::new(),
                    now: bencher_json::DateTime::now(),
                })),
                Effect::Exit => EffectResult::Exit,
                Effect::SleepBeforeReconnect(_)
                | Effect::Close
                | Effect::ReportOutcome(_)
                | Effect::Log(..)
                | Effect::SelfUpdate { .. }
                | Effect::PauseBegan(_)
                | Effect::PauseEnded
                | Effect::HoldOff(_) => EffectResult::Continue,
            }
        });

        assert!(matches!(result, Err(UpError::Shutdown)), "{result:?}");
        assert!(
            matches!(sent.last(), Some(RunnerMessage::Failed { error, .. }) if error == "the runner stopped"),
            "the Job's result is the last message sent: {sent:?}"
        );
    }
}
