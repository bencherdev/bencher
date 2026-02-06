// All daemon code is used on Linux; suppress dead-code warnings on other platforms
// where `Daemon::run()` is a stub.
#![cfg_attr(not(target_os = "linux"), allow(dead_code))]

#[cfg(not(target_os = "linux"))]
use std::sync::atomic::AtomicBool;
#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicBool, Ordering};

use url::Url;

use crate::cpu::CpuLayout;
use crate::tuning::TuningConfig;

mod api_client;
mod error;
mod job;
mod websocket;

pub use error::DaemonError;

#[cfg(target_os = "linux")]
use api_client::{ClaimRequest, RunnerApiClient};
#[cfg(target_os = "linux")]
use error::ApiClientError;
#[cfg(target_os = "linux")]
use job::{JobOutcome, execute_job};
#[cfg(target_os = "linux")]
use std::time::Duration;

#[cfg(target_os = "linux")]
const TRANSIENT_RETRY_DELAY: Duration = Duration::from_secs(5);

/// Global shutdown flag set by signal handler.
/// Async-signal-safe: only uses `AtomicBool::store`.
static SHUTDOWN: AtomicBool = AtomicBool::new(false);

pub struct DaemonConfig {
    pub host: Url,
    pub token: String,
    pub runner: String,
    pub labels: Vec<String>,
    pub poll_timeout_secs: u32,
    pub tuning: TuningConfig,
    /// CPU layout for isolating benchmark cores from housekeeping tasks.
    pub cpu_layout: CpuLayout,
}

pub struct Daemon {
    config: DaemonConfig,
}

impl Daemon {
    pub fn new(config: DaemonConfig) -> Self {
        Self { config }
    }

    #[cfg(target_os = "linux")]
    #[expect(clippy::print_stdout)]
    pub fn run(self) -> Result<(), DaemonError> {
        install_signal_handlers();

        println!("Bencher Runner Daemon starting...");
        println!("  Host: {}", self.config.host);
        println!("  Runner: {}", self.config.runner);
        if !self.config.labels.is_empty() {
            println!("  Labels: {}", self.config.labels.join(", "));
        }
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
            self.config.token.clone(),
            self.config.runner.clone(),
        )?;

        let claim_request = ClaimRequest {
            labels: self.config.labels.clone(),
            poll_timeout_seconds: self.config.poll_timeout_secs,
        };

        println!("Polling for jobs...");

        loop {
            // Check shutdown flag
            if SHUTDOWN.load(Ordering::SeqCst) {
                println!("Shutdown signal received, exiting...");
                return Err(DaemonError::Shutdown);
            }

            // Claim a job (long-poll, blocks up to poll_timeout_secs)
            match client.claim_job(&claim_request) {
                Ok(Some(job)) => {
                    let job_uuid = job.uuid;
                    println!("Claimed job: {job_uuid}");

                    let ws_url = client.websocket_url(&job_uuid)?;
                    match execute_job(&self.config, &job, &ws_url) {
                        Ok(JobOutcome::Completed { .. }) => {
                            println!("Job {job_uuid} completed successfully");
                        },
                        Ok(JobOutcome::Failed { error, .. }) => {
                            println!("Job {job_uuid} failed: {error}");
                        },
                        Ok(JobOutcome::Canceled) => {
                            println!("Job {job_uuid} was canceled");
                        },
                        Err(e) => {
                            println!("Job {job_uuid} error: {e}");
                        },
                    }

                    println!("Polling for jobs...");
                },
                Ok(None) => {
                    // No job available, loop back to poll
                },
                Err(ApiClientError::Unauthorized | ApiClientError::InvalidToken) => {
                    println!("Authentication failed. Check runner token.");
                    return Err(ApiClientError::Unauthorized.into());
                },
                Err(e) => {
                    println!("Error claiming job: {e}");
                    println!("Retrying in {} seconds...", TRANSIENT_RETRY_DELAY.as_secs());
                    std::thread::sleep(TRANSIENT_RETRY_DELAY);
                },
            }
        }
    }

    #[cfg(not(target_os = "linux"))]
    pub fn run(self) -> Result<(), DaemonError> {
        Err(DaemonError::Config("Daemon requires Linux".to_owned()))
    }
}

/// Install signal handlers for SIGINT and SIGTERM.
///
/// The handler sets the global `SHUTDOWN` flag. `AtomicBool::store` is
/// async-signal-safe, so this is safe to call from a signal handler context.
#[cfg(target_os = "linux")]
fn install_signal_handlers() {
    // SAFETY: `signal_handler` only performs `AtomicBool::store` with
    // `Ordering::SeqCst`, which is async-signal-safe per POSIX.
    unsafe {
        libc::signal(libc::SIGINT, signal_handler as libc::sighandler_t);
        libc::signal(libc::SIGTERM, signal_handler as libc::sighandler_t);
    }
}

#[cfg(target_os = "linux")]
extern "C" fn signal_handler(_sig: libc::c_int) {
    SHUTDOWN.store(true, Ordering::SeqCst);
}
