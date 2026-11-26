use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bencher_json::JsonClaimedJob;
use bencher_json::runner::{JsonIterationOutput, RunnerMessage, ServerMessage};
use camino::Utf8PathBuf;
use slog::{Logger, error, info, o, warn};

use super::UpConfig;
use super::state_machine::JobFinishResult;
use super::websocket::JobChannel;
use crate::run::PreparedImage;
use crate::{JobDeadline, RunnerError};

// NUL bytes are invalid in file paths on all OSes (POSIX and Windows),
// so this key can never collide with a real file collected from the VM.
// The server ignores keys (`output.into_values()`), but a collision in
// the BTreeMap would silently drop one entry.
const METRIC_OUTPUT_KEY: &str = "\0bencher";

/// Check whether the sandbox configuration is allowed by the runner.
///
/// Returns `Ok(())` if the job may proceed, or `Err` with a human-readable
/// reason when the runner is not configured to accept the requested sandbox.
fn check_sandbox_allowed(
    sandbox: Option<bencher_json::Sandbox>,
    allow_no_sandbox: bool,
) -> Result<(), &'static str> {
    match sandbox {
        Some(bencher_json::Sandbox::Firecracker) => Ok(()),
        None if allow_no_sandbox => Ok(()),
        None => Err(
            "Job requires non-sandboxed execution but runner was not started with --danger-allow-no-sandbox",
        ),
    }
}

/// Every record of the Job carries its `job`.
pub fn execute_job(
    log: &Logger,
    config: &UpConfig,
    job: &JsonClaimedJob,
    ws: &Arc<Mutex<JobChannel>>,
    host: &mut crate::jail::HostPreparation,
) -> JobFinishResult {
    let log = &log.new(o!("job" => job.uuid.to_string()));
    // The Runner has just sent `Running`, which starts the server's clock too.
    let deadline = JobDeadline::start(Duration::from_secs(job.config.timeout.as_secs()));

    // Only allow jobs with a known sandbox type or explicit opt-in for non-sandboxed.
    if let Err(reason) = check_sandbox_allowed(job.spec.sandbox, config.allow_no_sandbox) {
        return JobFinishResult::Failed {
            error: reason.to_owned(),
            results: Vec::new(),
        };
    }

    // Build runner Config from claimed job spec (all values from job spec, no defaults)
    let job_config = match build_config_from_job(config, job) {
        Ok(config) => config,
        Err(e) => {
            return JobFinishResult::Failed {
                error: e.to_string(),
                results: Vec::new(),
            };
        },
    };
    let cancel_flag = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::new(AtomicBool::new(false));
    let heartbeat = spawn_heartbeat_thread(log, config, ws, &cancel_flag, &stop_flag);
    let stop_watcher = spawn_stop_watcher(log, config, &cancel_flag, &stop_flag);

    // Passing cancel_flag so the wait for the VM can abort early when the
    // server cancels or the runner stops.
    let run = run_job(log, job, &job_config, host, deadline, &cancel_flag);

    // Stop the heartbeat and stop watcher threads
    stop_flag.store(true, Ordering::SeqCst);
    if heartbeat.join().is_err() {
        warn!(log, "Thread panicked"; "thread" => "heartbeat");
    }
    let stopped = stop_watcher.join().unwrap_or_else(|_panic| {
        warn!(log, "Thread panicked"; "thread" => "stop watcher");
        false
    });
    if stopped {
        info!(log, "Job stopped with the runner");
        return run.stopped();
    }

    let canceled = cancel_flag.load(Ordering::SeqCst);
    if canceled {
        info!(log, "Job canceled by the server");
    }
    run.finish(canceled)
}

/// Pulls the Job's image and runs it once, since the server sends each of a
/// Job's iterations as a Job of its own.
fn run_job(
    log: &Logger,
    job: &JsonClaimedJob,
    job_config: &crate::Config,
    host: &mut crate::jail::HostPreparation,
    deadline: JobDeadline,
    cancel_flag: &AtomicBool,
) -> Run {
    let allow_failure = job.config.allow_failure.unwrap_or_default();
    let benchmark_name = if job_config.build_time {
        match build_benchmark_name(job) {
            Ok(name) => Some(name),
            Err(e) => {
                warn!(log, "No benchmark name for the build time"; "error" => bencher_logger::capped(&e));
                None
            },
        }
    } else {
        None
    };

    let image = match PreparedImage::prepare(log, job_config, host, Some(cancel_flag), deadline) {
        Ok(image) => image,
        Err(e) => return Run::failed(e.to_string()),
    };
    run_once(log, allow_failure, deadline, cancel_flag, |deadline| {
        let output = image.execute(log, job_config, host, Some(cancel_flag), deadline)?;
        Ok(output_to_iteration(
            log,
            output,
            job_config.build_time,
            job_config.file_size,
            benchmark_name.as_ref(),
        ))
    })
}

/// Runs the Job with what is left of `deadline`, unless the server has
/// canceled it or its time has run out.
fn run_once<F>(
    log: &Logger,
    allow_failure: bool,
    deadline: JobDeadline,
    cancel_flag: &AtomicBool,
    run: F,
) -> Run
where
    F: FnOnce(JobDeadline) -> Result<JsonIterationOutput, RunnerError>,
{
    if cancel_flag.load(Ordering::SeqCst) {
        return Run::default();
    }
    if deadline.remaining().is_zero() {
        return Run::failed("Job timed out: no time was left for its run".to_owned());
    }
    info!(log, "Starting run");
    match run(deadline) {
        Ok(output) => {
            let failure = (output.exit_code != 0 && !allow_failure).then(|| {
                format!(
                    "Benchmark exited with non-zero exit code: {}",
                    output.exit_code
                )
            });
            Run {
                output: Some(output),
                failure,
            }
        },
        // A run with no result, which the server drops under allow_failure.
        // A run out of time fails the Job, since the Job is out of time too.
        Err(e) if allow_failure && !deadline.remaining().is_zero() => {
            warn!(log, "Run failed, skipped under allow_failure"; "error" => bencher_logger::capped(&e));
            Run::default()
        },
        Err(e) => Run::failed(e.to_string()),
    }
}

/// What the Job's run leaves behind for its outcome.
#[derive(Default)]
struct Run {
    output: Option<JsonIterationOutput>,
    /// Why the run failed.
    failure: Option<String>,
}

impl Run {
    fn failed(error: String) -> Self {
        Self {
            output: None,
            failure: Some(error),
        }
    }

    /// A cancel outranks a failure: the server sends one only for a Job it has
    /// already ended, and discards any other outcome as late.
    fn finish(self, canceled: bool) -> JobFinishResult {
        let Self { output, failure } = self;
        if canceled {
            return JobFinishResult::Canceled;
        }
        let exit_code = output.as_ref().map_or(0, |output| output.exit_code);
        let results = output.into_iter().collect();
        if let Some(error) = failure {
            return JobFinishResult::Failed { error, results };
        }
        JobFinishResult::Completed { exit_code, results }
    }

    /// The runner's stop is why the Job failed, whatever ended its run.
    fn stopped(self) -> JobFinishResult {
        JobFinishResult::Failed {
            error: "the runner stopped".to_owned(),
            results: self.output.into_iter().collect(),
        }
    }
}

/// Derive the benchmark name for build-time tracking from the job config.
///
/// Precedence: entrypoint+cmd joined string (matches CLI behavior),
/// then image reference, then project@digest as last resort.
fn build_benchmark_name(
    job: &JsonClaimedJob,
) -> Result<bencher_json::BenchmarkName, bencher_json::ValidError> {
    let args = job
        .config
        .entrypoint
        .iter()
        .flatten()
        .chain(job.config.cmd.iter().flatten())
        .map(String::as_str)
        .collect::<Vec<_>>();
    // No arguments, or one empty one, join to an empty name.
    if !matches!(args.as_slice(), [] | [""]) {
        return bencher_json::BenchmarkName::from_args(&args);
    }
    if let Some(image) = &job.config.image {
        return image.to_string().parse();
    }
    format!("{}@{}", job.config.project, job.config.digest).parse()
}

fn output_to_iteration(
    log: &Logger,
    output: crate::RunOutput,
    build_time: bool,
    file_size: bool,
    benchmark_name: Option<&bencher_json::BenchmarkName>,
) -> JsonIterationOutput {
    let file_output = build_metric_output(
        log,
        build_time.then_some(output.duration),
        file_size,
        output.output_files,
        // As in `bencher run`, a command that failed has no build time.
        benchmark_name.filter(|_| output.exit_code == 0),
    );
    JsonIterationOutput {
        exit_code: output.exit_code,
        stdout: if output.stdout.is_empty() {
            None
        } else {
            Some(output.stdout)
        },
        stderr: if output.stderr.is_empty() {
            None
        } else {
            Some(output.stderr)
        },
        output: file_output,
    }
}

fn build_metric_output(
    log: &Logger,
    build_time: Option<Duration>,
    file_size: bool,
    output_files: Option<Vec<(Utf8PathBuf, Vec<u8>)>>,
    benchmark_name: Option<&bencher_json::BenchmarkName>,
) -> Option<BTreeMap<Utf8PathBuf, String>> {
    use bencher_json::{
        JsonNewMetric,
        project::measure::built_in::{self, BuiltInMeasure as _},
        project::metric::MetricResults,
    };

    let has_metrics = build_time.is_some() || file_size;
    if !has_metrics {
        return output_files.map(|files| {
            files
                .into_iter()
                .map(|(path, bytes)| (path, String::from_utf8_lossy(&bytes).into_owned()))
                .collect()
        });
    }

    let mut metric_results: MetricResults = Vec::new();

    if file_size && let Some(files) = &output_files {
        // In declared order, so the last file with a basename wins, as in `bencher run --file-size`.
        for (path, bytes) in files {
            let file_name = path.file_name().unwrap_or(path.as_str());
            match file_name.parse() {
                Ok(name) => {
                    #[expect(clippy::cast_precision_loss, reason = "file size as f64 metric value")]
                    let size = bytes.len() as f64;
                    metric_results.push((
                        name,
                        vec![(
                            built_in::json::FileSize::name_id(),
                            JsonNewMetric {
                                value: size.into(),
                                ..Default::default()
                            },
                        )],
                    ));
                },
                Err(e) => {
                    warn!(log, "File size metric skipped";
                        "path" => bencher_logger::capped(path),
                        "error" => bencher_logger::capped(&e),
                    );
                },
            }
        }
    }

    // After the file sizes, so a build time named like a file wins, as in `bencher run`.
    if let Some(duration) = build_time
        && let Some(name) = benchmark_name
    {
        let seconds = (duration.as_secs_f64() * 100.0).round() / 100.0;
        metric_results.push((
            name.clone(),
            vec![(
                built_in::json::BuildTime::name_id(),
                JsonNewMetric {
                    value: seconds.into(),
                    ..Default::default()
                },
            )],
        ));
    }

    // Empty rather than none for a Job that asked for a build time, since the
    // server parses the stdout of an iteration with no output instead.
    if metric_results.is_empty() {
        return build_time.map(|_| BTreeMap::new());
    }

    let results = JsonNewMetric::results(metric_results);
    let bmf_json = match serde_json::to_string(&results) {
        Ok(json) => json,
        Err(e) => {
            warn!(log, "Metrics not serialized"; "error" => bencher_logger::capped(&e));
            return None;
        },
    };
    let mut output = BTreeMap::new();
    output.insert(Utf8PathBuf::from(METRIC_OUTPUT_KEY), bmf_json);
    Some(output)
}

/// Build a runner Config from the claimed job and up config.
///
/// Resource requirements (cpu, memory, disk, network) come from the spec as
/// strong types and are passed through directly.
/// Execution details (registry, project, digest, entrypoint, cmd, env, timeout,
/// `file_paths`) come from the config. The OCI token is passed through for
/// authenticated image pulls.
///
/// CPU layout from the up config is passed through for core isolation.
///
/// The up config is destructured so a new runner-level setting is a build error
/// until it is passed through here, rather than silently ignored at runtime.
fn build_config_from_job(
    up_config: &UpConfig,
    job: &JsonClaimedJob,
) -> Result<crate::Config, crate::error::ConfigError> {
    let UpConfig {
        // Protocol and identity settings, which do not describe a run.
        host: _,
        key: _,
        runner: _,
        poll_timeout_secs: _,
        tuning: _,
        no_auto_update: _,
        update_channel: _,
        max_download_size: _,
        allow_no_sandbox: _,
        raid_pause: _,
        cpu_layout,
        max_output_size,
        max_file_count,
        max_symlinks,
        sandbox_log_level,
        image_cache_size,
        state_dir,
        jail_user,
    } = up_config;

    let spec = &job.spec;
    let config = &job.config;

    // Build OCI image reference: host:port/project@digest
    // The API's OCI registry routes use the project UUID/slug as the repository
    // name (e.g., /v2/{project}/manifests/{ref}), so no extra path segments needed.
    // ImageReference::parse() expects Docker-style references (host:port/repo@digest),
    // not full URLs with schemes, so strip the scheme from the registry URL.
    let registry_str = config.registry.as_ref().trim_end_matches('/');
    let (registry_scheme, registry_authority) =
        if let Some(authority) = registry_str.strip_prefix("http://") {
            (bencher_oci::RegistryScheme::Http, authority)
        } else if let Some(authority) = registry_str.strip_prefix("https://") {
            (bencher_oci::RegistryScheme::Https, authority)
        } else {
            (bencher_oci::RegistryScheme::Https, registry_str)
        };
    let oci_image = format!("{registry_authority}/{}@{}", config.project, config.digest);

    let mut runner_config = crate::Config::new(oci_image)
        .with_registry_scheme(registry_scheme)
        .with_token(job.oci_token.to_string())?
        .with_vcpus(spec.cpu)
        .with_memory(spec.memory)
        .with_disk(spec.disk)
        .with_timeout_secs(u64::from(u32::from(config.timeout)))
        .with_network(spec.network)
        .with_build_time(config.build_time.unwrap_or_default())
        .with_file_size(config.file_size.unwrap_or_default())
        .with_entrypoint_opt(config.entrypoint.clone())
        .with_cmd_opt(config.cmd.clone())
        .with_env_opt(config.env.clone());

    // Pass all file paths through for multi-file output extraction
    runner_config = runner_config.with_file_paths_opt(config.file_paths.clone());

    runner_config = runner_config.with_state_dir(state_dir.clone());
    runner_config = runner_config.with_jail_user(*jail_user);
    runner_config.blob_store = image_cache_size.map(|_| crate::jail::blob_store(state_dir));

    // Pass through CPU layout for core isolation
    if let Some(cpu_layout) = cpu_layout
        && cpu_layout.has_isolation()
    {
        runner_config = runner_config.with_cpu_layout(cpu_layout.clone());
    }

    // Pass through max output size if configured
    if let Some(max_output_size) = *max_output_size {
        runner_config = runner_config.with_max_output_size(max_output_size);
    }

    // Pass through max file count if configured
    if let Some(max_file_count) = *max_file_count {
        runner_config = runner_config.with_max_file_count(max_file_count);
    }

    // Pass through max symlinks if configured
    if let Some(max_symlinks) = *max_symlinks {
        runner_config = runner_config.with_max_symlinks(max_symlinks);
    }

    // Pass through sandbox log level
    runner_config.sandbox_log_level = *sandbox_log_level;

    // Pass through sandbox mode from the job spec
    runner_config = runner_config.with_sandbox(spec.sandbox);

    Ok(runner_config)
}

fn spawn_heartbeat_thread(
    log: &Logger,
    config: &UpConfig,
    ws: &Arc<Mutex<JobChannel>>,
    cancel_flag: &Arc<AtomicBool>,
    stop_flag: &Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    let ws_heartbeat = Arc::clone(ws);
    let cancel_heartbeat = Arc::clone(cancel_flag);
    let stop_heartbeat = Arc::clone(stop_flag);
    let housekeeping_cores = housekeeping_cores(config);
    let log = log.clone();
    std::thread::spawn(move || {
        if let Err(e) = crate::cpu::pin_current_thread(&housekeeping_cores) {
            warn!(log, "Thread not pinned to housekeeping cores"; "thread" => "heartbeat", "error" => %e);
        }
        heartbeat_loop(&log, &ws_heartbeat, &cancel_heartbeat, &stop_heartbeat);
    })
}

fn heartbeat_loop(
    log: &Logger,
    ws: &Arc<Mutex<JobChannel>>,
    cancel_flag: &AtomicBool,
    stop_flag: &AtomicBool,
) {
    loop {
        std::thread::sleep(Duration::from_secs(1));

        if stop_flag.load(Ordering::SeqCst) {
            break;
        }

        let mut ws_guard = match ws.lock() {
            Ok(guard) => guard,
            Err(e) => {
                error!(log, "Heartbeat lock poisoned"; "error" => %e);
                break;
            },
        };

        // Send heartbeat, ignoring errors (main thread handles fatal WS errors)
        if ws_guard.send_message(&RunnerMessage::Heartbeat).is_err() {
            break;
        }

        // Check for cancel
        match ws_guard.try_read_message() {
            Ok(Some(ServerMessage::Cancel)) => {
                cancel_flag.store(true, Ordering::SeqCst);
                break;
            },
            Ok(None | Some(ServerMessage::Ack { .. })) => {},
            Ok(Some(
                msg @ (ServerMessage::Job(_) | ServerMessage::NoJob | ServerMessage::Update { .. }),
            )) => {
                warn!(log, "Unexpected server message during the Job";
                    "detail" => bencher_logger::capped(format!("{msg:?}")),
                );
            },
            Err(_) => break,
        }
    }
}

/// How often the stop watcher looks for the runner's stop.
const STOP_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// Turns the runner's stop into the Job's cancel, so the teardown kills the
/// VMM before the service manager's stop timeout kills the runner.
fn spawn_stop_watcher(
    log: &Logger,
    config: &UpConfig,
    cancel_flag: &Arc<AtomicBool>,
    done_flag: &Arc<AtomicBool>,
) -> std::thread::JoinHandle<bool> {
    let cancel = Arc::clone(cancel_flag);
    let done = Arc::clone(done_flag);
    let housekeeping_cores = housekeeping_cores(config);
    let log = log.clone();
    std::thread::spawn(move || {
        if let Err(e) = crate::cpu::pin_current_thread(&housekeeping_cores) {
            warn!(log, "Thread not pinned to housekeeping cores"; "thread" => "stop watcher", "error" => %e);
        }
        cancel_on_stop(crate::signal::stop_flag(), &cancel, &done)
    })
}

/// Sets `cancel_flag` once `runner_stop` is set, until the Job is `done`.
/// True only if the stop ended the Job, not a cancel the server sent first.
fn cancel_on_stop(runner_stop: &AtomicBool, cancel_flag: &AtomicBool, done: &AtomicBool) -> bool {
    loop {
        if done.load(Ordering::SeqCst) {
            return false;
        }
        if runner_stop.load(Ordering::SeqCst) {
            return !cancel_flag.swap(true, Ordering::SeqCst);
        }
        std::thread::sleep(STOP_POLL_INTERVAL);
    }
}

fn housekeeping_cores(config: &UpConfig) -> Vec<usize> {
    config
        .cpu_layout
        .as_ref()
        .map(|l| l.housekeeping.clone())
        .unwrap_or_default()
}

#[cfg(test)]
pub(crate) mod tests {
    use std::collections::HashMap;
    use std::io::{ErrorKind, Read as _, Write as _};
    use std::net::{SocketAddr, TcpListener, TcpStream};
    use std::num::NonZeroU64;
    use std::sync::mpsc::{self, TryRecvError};
    use std::thread::JoinHandle;

    use super::*;
    use camino::{Utf8Path, Utf8PathBuf};

    use crate::ExecutionError;
    use crate::log::discard;
    use crate::units::mib_to_bytes;
    use bencher_json::{Cpu, Disk, Memory};

    /// Construct a `JsonClaimedJob` for testing by building the JSON
    /// with the proper nested structure and deserializing.
    fn test_job(
        cpu: u32,
        memory_bytes: u64,
        disk_bytes: u64,
        timeout: u32,
        network: bool,
    ) -> JsonClaimedJob {
        test_job_with_options(
            cpu,
            memory_bytes,
            disk_bytes,
            timeout,
            network,
            None,
            None,
            None,
            None,
        )
    }

    #[expect(
        clippy::too_many_arguments,
        clippy::needless_pass_by_value,
        reason = "test helper with many config fields"
    )]
    fn test_job_with_options(
        cpu: u32,
        memory_bytes: u64,
        disk_bytes: u64,
        timeout: u32,
        network: bool,
        entrypoint: Option<Vec<String>>,
        cmd: Option<Vec<String>>,
        env: Option<HashMap<String, String>>,
        file_paths: Option<Vec<String>>,
    ) -> JsonClaimedJob {
        let json = serde_json::json!({
            "uuid": "550e8400-e29b-41d4-a716-446655440000",
            "spec": {
                "uuid": "00000000-0000-0000-0000-000000000001",
                "name": "test-spec",
                "slug": "test-spec",
                "os": "linux",
                "architecture": "x86_64",
                "cpu": cpu,
                "memory": memory_bytes,
                "disk": disk_bytes,
                "network": network,
                "created": "2025-01-01T00:00:00Z",
                "modified": "2025-01-01T00:00:00Z"
            },
            "config": {
                "registry": "https://registry.bencher.dev",
                "project": "11111111-2222-3333-4444-555555555555",
                "digest": "sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3",
                "entrypoint": entrypoint,
                "cmd": cmd,
                "env": env,
                "timeout": timeout,
                "file_paths": file_paths,
            },
            "oci_token": OCI_TOKEN,
            "timeout": timeout,
            "created": "2025-01-01T00:00:00Z"
        });
        serde_json::from_value(json).expect("Failed to construct test JsonClaimedJob")
    }

    const OCI_TOKEN: &str = "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiYWRtaW4iOnRydWV9.TJVA95OrM7E2cBab30RMHrHDcEfxjoYZgeFONFh7HgQ";

    // --- build_config_from_job ---

    pub(crate) fn test_up_config() -> UpConfig {
        UpConfig {
            host: url::Url::parse("https://api.bencher.dev").unwrap(),
            key: "bencher_runner_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh"
                .parse()
                .unwrap(),
            runner: "test-runner".parse().unwrap(),
            poll_timeout_secs: 30,
            tuning: crate::TuningConfig::disabled(),
            cpu_layout: Some(crate::cpu::CpuLayout::with_core_count(4)),
            max_output_size: None,
            max_file_count: None,
            max_symlinks: None,
            sandbox_log_level: crate::SandboxLogLevel::default(),
            allow_no_sandbox: false,
            raid_pause: true,
            no_auto_update: false,
            update_channel: bencher_valid::UpdateChannel::default(),
            max_download_size: None,
            image_cache_size: None,
            state_dir: Utf8PathBuf::from(crate::jail::DEFAULT_STATE_DIR),
            jail_user: crate::jail::JailUser::default(),
        }
    }

    #[test]
    fn uses_job_spec_vcpus() {
        let up_config = test_up_config();
        let job = test_job(4, mib_to_bytes(512), mib_to_bytes(1024), 300, false);
        let result = build_config_from_job(&up_config, &job).unwrap();
        assert_eq!(result.vcpus, Cpu::try_from(4).unwrap());
    }

    #[test]
    fn converts_memory_from_job() {
        let up_config = test_up_config();
        let job = test_job(1, mib_to_bytes(2048), mib_to_bytes(1024), 300, false);
        let result = build_config_from_job(&up_config, &job).unwrap();
        assert_eq!(result.memory, Memory::from_mib(2048).unwrap());
    }

    #[test]
    fn converts_disk_from_job() {
        let up_config = test_up_config();
        let job = test_job(1, mib_to_bytes(512), mib_to_bytes(10240), 300, false);
        let result = build_config_from_job(&up_config, &job).unwrap();
        assert_eq!(result.disk, Disk::from_mib(10240).unwrap());
    }

    #[test]
    fn memory_preserves_bytes() {
        let up_config = test_up_config();
        // 512 MiB + 1 byte - strong type preserves exact byte value
        let job = test_job(1, mib_to_bytes(512) + 1, mib_to_bytes(1024), 300, false);
        let result = build_config_from_job(&up_config, &job).unwrap();
        assert_eq!(result.memory.to_mib(), 513);
    }

    #[test]
    fn timeout_converts_u32_to_u64() {
        let up_config = test_up_config();
        let job = test_job(1, mib_to_bytes(512), mib_to_bytes(1024), 600, false);
        let result = build_config_from_job(&up_config, &job).unwrap();
        assert_eq!(result.timeout_secs, 600);
    }

    #[test]
    fn builds_oci_image_url() {
        let up_config = test_up_config();
        let job = test_job(1, mib_to_bytes(512), mib_to_bytes(1024), 300, false);
        let result = build_config_from_job(&up_config, &job).unwrap();
        assert_eq!(
            result.oci_image,
            "registry.bencher.dev/11111111-2222-3333-4444-555555555555@sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"
        );
    }

    #[test]
    fn builds_oci_image_url_http_scheme() {
        let up_config = test_up_config();
        let json = serde_json::json!({
            "uuid": "550e8400-e29b-41d4-a716-446655440000",
            "spec": {
                "uuid": "00000000-0000-0000-0000-000000000001",
                "name": "test-spec",
                "slug": "test-spec",
                "os": "linux",
                "architecture": "x86_64",
                "cpu": 1,
                "memory": mib_to_bytes(512),
                "disk": mib_to_bytes(1024),
                "network": false,
                "created": "2025-01-01T00:00:00Z",
                "modified": "2025-01-01T00:00:00Z"
            },
            "config": {
                "registry": "http://localhost:6610",
                "project": "11111111-2222-3333-4444-555555555555",
                "digest": "sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3",
                "timeout": 300,
            },
            "oci_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiYWRtaW4iOnRydWV9.TJVA95OrM7E2cBab30RMHrHDcEfxjoYZgeFONFh7HgQ",
            "timeout": 300,
            "created": "2025-01-01T00:00:00Z"
        });
        let job: JsonClaimedJob =
            serde_json::from_value(json).expect("Failed to construct test JsonClaimedJob");
        let result = build_config_from_job(&up_config, &job).unwrap();
        assert_eq!(
            result.oci_image,
            "localhost:6610/11111111-2222-3333-4444-555555555555@sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"
        );
    }

    #[test]
    fn oci_token_passed_through() {
        let up_config = test_up_config();
        let job = test_job(1, mib_to_bytes(512), mib_to_bytes(1024), 300, false);
        let result = build_config_from_job(&up_config, &job).unwrap();
        assert!(
            result.token.is_some(),
            "OCI token should be passed to config"
        );
    }

    #[test]
    fn network_enabled() {
        let up_config = test_up_config();
        let job = test_job(1, mib_to_bytes(512), mib_to_bytes(1024), 300, true);
        let result = build_config_from_job(&up_config, &job).unwrap();
        assert!(result.network);
    }

    #[test]
    fn network_disabled() {
        let up_config = test_up_config();
        let job = test_job(1, mib_to_bytes(512), mib_to_bytes(1024), 300, false);
        let result = build_config_from_job(&up_config, &job).unwrap();
        assert!(!result.network);
    }

    #[test]
    fn entrypoint_and_cmd() {
        let up_config = test_up_config();
        let job = test_job_with_options(
            1,
            mib_to_bytes(512),
            mib_to_bytes(1024),
            300,
            false,
            Some(vec!["/bin/sh".to_owned()]),
            Some(vec!["-c".to_owned(), "cargo bench".to_owned()]),
            None,
            None,
        );
        let result = build_config_from_job(&up_config, &job).unwrap();
        assert_eq!(result.entrypoint.unwrap(), vec!["/bin/sh"]);
        assert_eq!(result.cmd.unwrap(), vec!["-c", "cargo bench"]);
    }

    #[test]
    fn env_vars() {
        let up_config = test_up_config();
        let mut env = HashMap::new();
        env.insert("RUST_LOG".to_owned(), "debug".to_owned());
        env.insert("CI".to_owned(), "true".to_owned());

        let job = test_job_with_options(
            1,
            mib_to_bytes(512),
            mib_to_bytes(1024),
            300,
            false,
            None,
            None,
            Some(env.clone()),
            None,
        );
        let result = build_config_from_job(&up_config, &job).unwrap();
        let result_env = result.env.unwrap();
        assert_eq!(&result_env["RUST_LOG"], "debug");
        assert_eq!(&result_env["CI"], "true");
    }

    #[test]
    fn file_paths_passed_through() {
        let up_config = test_up_config();
        let job = test_job_with_options(
            1,
            mib_to_bytes(512),
            mib_to_bytes(1024),
            300,
            false,
            None,
            None,
            None,
            Some(vec!["/tmp/results.json".to_owned()]),
        );
        let result = build_config_from_job(&up_config, &job).unwrap();
        assert_eq!(
            result.file_paths.as_deref(),
            Some([Utf8PathBuf::from("/tmp/results.json")].as_slice())
        );
    }

    #[test]
    fn multiple_file_paths_passed_through() {
        let up_config = test_up_config();
        let job = test_job_with_options(
            1,
            mib_to_bytes(512),
            mib_to_bytes(1024),
            300,
            false,
            None,
            None,
            None,
            Some(vec![
                "/tmp/results.json".to_owned(),
                "/tmp/metrics.csv".to_owned(),
            ]),
        );
        let result = build_config_from_job(&up_config, &job).unwrap();
        let paths = result.file_paths.unwrap();
        assert_eq!(paths.len(), 2);
        assert_eq!(paths[0], Utf8PathBuf::from("/tmp/results.json"));
        assert_eq!(paths[1], Utf8PathBuf::from("/tmp/metrics.csv"));
    }

    #[test]
    fn all_options() {
        let up_config = test_up_config();
        let mut env = HashMap::new();
        env.insert("KEY".to_owned(), "value".to_owned());

        let job = test_job_with_options(
            8,
            mib_to_bytes(4096),
            mib_to_bytes(20480),
            900,
            true,
            Some(vec!["/bin/bash".to_owned()]),
            Some(vec!["-c".to_owned(), "make bench".to_owned()]),
            Some(env),
            Some(vec!["/output/bench.txt".to_owned()]),
        );
        let result = build_config_from_job(&up_config, &job).unwrap();
        assert_eq!(result.vcpus, Cpu::try_from(8).unwrap());
        assert_eq!(result.memory, Memory::from_mib(4096).unwrap());
        assert_eq!(result.disk, Disk::from_mib(20480).unwrap());
        assert_eq!(result.timeout_secs, 900);
        assert!(result.network);
        assert!(result.entrypoint.is_some());
        assert!(result.cmd.is_some());
        assert!(result.env.is_some());
        assert!(result.token.is_some());
        assert_eq!(
            result.file_paths.as_deref(),
            Some([Utf8PathBuf::from("/output/bench.txt")].as_slice())
        );
    }

    #[test]
    fn state_dir_passed_through() {
        // A job that builds its jail outside the daemon's swept state directory
        // leaks a chroot on every unclean exit.
        let mut up_config = test_up_config();
        up_config.state_dir = Utf8PathBuf::from("/mnt/fast/runner-state");
        let job = test_job(1, mib_to_bytes(512), mib_to_bytes(1024), 300, false);

        let result = build_config_from_job(&up_config, &job).unwrap();

        assert_eq!(
            result.state_dir,
            Utf8PathBuf::from("/mnt/fast/runner-state")
        );
    }

    #[test]
    fn state_dir_defaults_when_the_daemon_uses_the_default() {
        let up_config = test_up_config();
        let job = test_job(1, mib_to_bytes(512), mib_to_bytes(1024), 300, false);

        let result = build_config_from_job(&up_config, &job).unwrap();

        assert_eq!(result.state_dir, crate::jail::DEFAULT_STATE_DIR);
    }

    #[test]
    fn jail_user_passed_through() {
        // Without the override, the VMM can share a uid with a local account
        // that can signal it.
        let mut up_config = test_up_config();
        up_config.jail_user = crate::jail::JailUser::new(4242, 4243).unwrap();
        let job = test_job(1, mib_to_bytes(512), mib_to_bytes(1024), 300, false);

        let result = build_config_from_job(&up_config, &job).unwrap();

        assert_eq!(result.jail_user.uid(), 4242);
        assert_eq!(result.jail_user.gid(), 4243);
    }

    #[test]
    fn cpu_layout_passed_through() {
        let up_config = test_up_config();
        let job = test_job(4, mib_to_bytes(512), mib_to_bytes(1024), 300, false);
        let result = build_config_from_job(&up_config, &job).unwrap();
        // CPU layout should be passed through from up config
        assert!(result.cpu_layout.is_some());
        let layout = result.cpu_layout.unwrap();
        assert_eq!(layout.housekeeping, vec![0]);
        assert_eq!(layout.benchmark, vec![1, 2, 3]);
    }

    #[test]
    fn cpu_layout_not_passed_when_no_isolation() {
        let mut up_config = test_up_config();
        // Single core - no isolation possible
        up_config.cpu_layout = Some(crate::cpu::CpuLayout::with_core_count(1));
        let job = test_job(1, mib_to_bytes(512), mib_to_bytes(1024), 300, false);
        let result = build_config_from_job(&up_config, &job).unwrap();
        // CPU layout should not be passed through when no isolation is possible
        assert!(result.cpu_layout.is_none());
    }

    // --- check_sandbox_allowed ---

    #[test]
    fn sandbox_firecracker_always_allowed() {
        check_sandbox_allowed(Some(bencher_json::Sandbox::Firecracker), false).unwrap();
        check_sandbox_allowed(Some(bencher_json::Sandbox::Firecracker), true).unwrap();
    }

    #[test]
    fn sandbox_none_rejected_without_flag() {
        check_sandbox_allowed(None, false).unwrap_err();
    }

    #[test]
    fn sandbox_none_allowed_with_flag() {
        check_sandbox_allowed(None, true).unwrap();
    }

    // --- Run::finish ---

    #[test]
    fn a_failure_without_a_cancel_ends_the_job_failed() {
        // Prevents the cancel check passing every Job, which reports a Job
        // that failed on its own as canceled.
        let run = Run::failed("Benchmark exited with non-zero exit code: 1".to_owned());

        let outcome = run.finish(false);

        assert!(
            matches!(&outcome, JobFinishResult::Failed { error, .. } if error.ends_with("exit code: 1")),
            "a Job no one canceled ends with its own failure, got: {outcome:?}"
        );
    }

    #[test]
    fn a_cancel_that_ends_the_run_ends_the_job_canceled() {
        // Prevents the error a server cancel leaves in result collection
        // outranking the cancel, which reports a canceled Job as failed.
        let run = Run::failed(
            RunnerError::from(ExecutionError::Canceled("job was canceled".to_owned())).to_string(),
        );

        let outcome = run.finish(true);

        assert!(
            matches!(outcome, JobFinishResult::Canceled),
            "a Job the server canceled ends canceled, got: {outcome:?}"
        );
    }

    // --- run_once ---

    #[test]
    fn a_non_zero_exit_fails_the_job_without_allow_failure() {
        // Kills a non-zero exit reported as completed, which gives the server
        // a Job to keep that the user asked to fail.
        let run = run_once(
            &discard(),
            false,
            JobDeadline::start(Duration::from_mins(1)),
            &AtomicBool::new(false),
            |_deadline| Ok(exit_output(3)),
        );

        let outcome = run.finish(false);
        assert!(
            matches!(&outcome, JobFinishResult::Failed { error, results } if error.ends_with("exit code: 3") && results.len() == 1),
            "the run's failure and its result are kept: {outcome:?}"
        );
    }

    #[test]
    fn a_non_zero_exit_completes_the_job_under_allow_failure() {
        // Kills allow_failure ignored for a non-zero exit, which fails a Job
        // whose user allowed the benchmark to fail.
        let run = run_once(
            &discard(),
            true,
            JobDeadline::start(Duration::from_mins(1)),
            &AtomicBool::new(false),
            |_deadline| Ok(exit_output(3)),
        );

        let outcome = run.finish(false);
        assert!(
            matches!(&outcome, JobFinishResult::Completed { exit_code: 3, results } if results.len() == 1),
            "the run completes with its exit code: {outcome:?}"
        );
    }

    #[test]
    fn no_run_starts_once_the_jobs_timeout_has_run_out() {
        // Prevents a VM booting with no time left, after the server's limit.
        let mut started = false;

        let run = run_once(
            &discard(),
            true,
            JobDeadline::start(Duration::ZERO),
            &AtomicBool::new(false),
            |_deadline| {
                started = true;
                Ok(iteration_output())
            },
        );

        assert!(!started, "the run had no time to start");
        let outcome = run.finish(false);
        assert!(
            matches!(&outcome, JobFinishResult::Failed { error, results } if error.contains("timed out") && results.is_empty()),
            "a Job out of time ends failed: {outcome:?}"
        );
    }

    #[test]
    fn no_run_starts_once_the_server_has_canceled() {
        // Kills a run started for a cancel that landed after the pull, which
        // spawns the benchmark of a Job the server has ended.
        let mut started = false;

        let run = run_once(
            &discard(),
            false,
            JobDeadline::start(Duration::from_mins(1)),
            &AtomicBool::new(true),
            |_deadline| {
                started = true;
                Ok(iteration_output())
            },
        );

        assert!(!started, "the canceled Job never runs");
        assert!(matches!(run.finish(true), JobFinishResult::Canceled));
    }

    #[test]
    fn running_out_of_time_ends_the_job_whatever_allow_failure_says() {
        // Prevents allow_failure skipping a timeout, which reports a Job that
        // ran out of time as completed.
        let run = run_once(
            &discard(),
            true,
            JobDeadline::start(Duration::from_secs(1)),
            &AtomicBool::new(false),
            |deadline| {
                spend(deadline);
                Err(ExecutionError::Timeout("the guest was still running".to_owned()).into())
            },
        );

        let outcome = run.finish(false);
        assert!(
            matches!(&outcome, JobFinishResult::Failed { error, .. } if error.contains("the guest was still running")),
            "the run's own error is kept: {outcome:?}"
        );
    }

    #[test]
    fn allow_failure_still_skips_a_failure_with_time_left() {
        // Prevents the timeout check swallowing allow_failure for every failure.
        let run = run_once(
            &discard(),
            true,
            JobDeadline::start(Duration::from_mins(1)),
            &AtomicBool::new(false),
            |_deadline| Err(ExecutionError::Setup("the image would not unpack".to_owned()).into()),
        );

        let outcome = run.finish(false);
        assert!(
            matches!(&outcome, JobFinishResult::Completed { results, .. } if results.is_empty()),
            "a failure with time left is skipped, got: {outcome:?}"
        );
    }

    // --- the runner's stop ---

    #[test]
    fn a_runner_stop_cancels_the_running_job() {
        // Prevents a stop that leaves the Job running, so the service
        // manager's stop timeout kills the runner and strands the VMM.
        let runner_stop = AtomicBool::new(false);
        let cancel_flag = AtomicBool::new(false);

        let stopped = std::thread::scope(|scope| {
            scope.spawn(|| {
                std::thread::sleep(STOP_POLL_INTERVAL * 2);
                runner_stop.store(true, Ordering::SeqCst);
            });
            cancel_on_stop(&runner_stop, &cancel_flag, &AtomicBool::new(false))
        });

        assert!(
            cancel_flag.load(Ordering::SeqCst),
            "the stop cancels the Job"
        );
        assert!(stopped, "the stop is why the Job ended");
    }

    #[test]
    fn a_cancel_the_server_sent_first_stays_a_cancel() {
        // Prevents the stop claiming a Job the server already canceled, which
        // reports it failed.
        let stopped = cancel_on_stop(
            &AtomicBool::new(true),
            &AtomicBool::new(true),
            &AtomicBool::new(false),
        );

        assert!(!stopped, "the server's cancel came first");
    }

    #[test]
    fn a_stop_once_the_job_is_done_leaves_its_outcome() {
        // Prevents a stop after the run throwing away a Job that finished.
        let cancel_flag = AtomicBool::new(false);

        let stopped = cancel_on_stop(&AtomicBool::new(true), &cancel_flag, &AtomicBool::new(true));

        assert!(
            !stopped && !cancel_flag.load(Ordering::SeqCst),
            "the Job ended on its own"
        );
    }

    #[test]
    fn a_job_the_runner_stopped_fails_with_why() {
        // Prevents a stopped Job reporting the cancel its teardown left, or
        // losing the result of its run.
        let run = Run {
            output: Some(iteration_output()),
            failure: Some(
                RunnerError::from(ExecutionError::Canceled("job was canceled".to_owned()))
                    .to_string(),
            ),
        };

        let outcome = run.stopped();

        let JobFinishResult::Failed { error, results } = outcome else {
            panic!("a stopped Job ends failed, got: {outcome:?}");
        };
        assert_eq!(error, "the runner stopped", "the user sees why");
        assert_eq!(results.len(), 1, "the run's result is kept");
    }

    fn iteration_output() -> JsonIterationOutput {
        exit_output(0)
    }

    fn exit_output(exit_code: i32) -> JsonIterationOutput {
        JsonIterationOutput {
            exit_code,
            stdout: Some("ran".to_owned()),
            stderr: None,
            output: None,
        }
    }

    /// Stands in for a run that lasts until the Job is out of time.
    fn spend(deadline: JobDeadline) {
        while !deadline.remaining().is_zero() {
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    // --- build_config_from_job: build_time / file_size ---

    #[test]
    fn build_time_passed_through() {
        let up_config = test_up_config();
        let json = serde_json::json!({
            "uuid": "550e8400-e29b-41d4-a716-446655440000",
            "spec": {
                "uuid": "00000000-0000-0000-0000-000000000001",
                "name": "test-spec", "slug": "test-spec",
                "os": "linux", "architecture": "x86_64",
                "cpu": 1, "memory": mib_to_bytes(512), "disk": mib_to_bytes(1024),
                "network": false,
                "created": "2025-01-01T00:00:00Z", "modified": "2025-01-01T00:00:00Z"
            },
            "config": {
                "registry": "https://registry.bencher.dev",
                "project": "11111111-2222-3333-4444-555555555555",
                "digest": "sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3",
                "timeout": 300,
                "build_time": true,
            },
            "oci_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiYWRtaW4iOnRydWV9.TJVA95OrM7E2cBab30RMHrHDcEfxjoYZgeFONFh7HgQ",
            "timeout": 300,
            "created": "2025-01-01T00:00:00Z"
        });
        let job: JsonClaimedJob = serde_json::from_value(json).unwrap();
        let result = build_config_from_job(&up_config, &job).unwrap();
        assert!(result.build_time);
    }

    #[test]
    fn file_size_passed_through() {
        let up_config = test_up_config();
        let json = serde_json::json!({
            "uuid": "550e8400-e29b-41d4-a716-446655440000",
            "spec": {
                "uuid": "00000000-0000-0000-0000-000000000001",
                "name": "test-spec", "slug": "test-spec",
                "os": "linux", "architecture": "x86_64",
                "cpu": 1, "memory": mib_to_bytes(512), "disk": mib_to_bytes(1024),
                "network": false,
                "created": "2025-01-01T00:00:00Z", "modified": "2025-01-01T00:00:00Z"
            },
            "config": {
                "registry": "https://registry.bencher.dev",
                "project": "11111111-2222-3333-4444-555555555555",
                "digest": "sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3",
                "timeout": 300,
                "file_size": true,
            },
            "oci_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiYWRtaW4iOnRydWV9.TJVA95OrM7E2cBab30RMHrHDcEfxjoYZgeFONFh7HgQ",
            "timeout": 300,
            "created": "2025-01-01T00:00:00Z"
        });
        let job: JsonClaimedJob = serde_json::from_value(json).unwrap();
        let result = build_config_from_job(&up_config, &job).unwrap();
        assert!(result.file_size);
    }

    #[test]
    fn build_time_default_false() {
        let up_config = test_up_config();
        let job = test_job(1, mib_to_bytes(512), mib_to_bytes(1024), 300, false);
        let result = build_config_from_job(&up_config, &job).unwrap();
        assert!(!result.build_time);
    }

    #[test]
    fn file_size_default_false() {
        let up_config = test_up_config();
        let job = test_job(1, mib_to_bytes(512), mib_to_bytes(1024), 300, false);
        let result = build_config_from_job(&up_config, &job).unwrap();
        assert!(!result.file_size);
    }

    // --- build_benchmark_name ---

    #[test]
    fn build_benchmark_name_joins_the_entrypoint_and_cmd() {
        let job = command_job(Some(&["/usr/bin/env"]), Some(&["make"]));
        assert_eq!(
            build_benchmark_name(&job).unwrap().as_ref(),
            "/usr/bin/env make"
        );
    }

    #[test]
    fn build_benchmark_name_is_the_cmd_without_an_entrypoint() {
        let job = command_job(None, Some(&["make"]));
        assert_eq!(build_benchmark_name(&job).unwrap().as_ref(), "make");
    }

    #[test]
    fn build_benchmark_name_falls_back_to_the_image() {
        const IMAGE: &str = "alpine:3.18";
        // A lone empty argument joins to an empty name, as no command does.
        for cmd in [None, Some(&[""][..])] {
            let mut job = command_job(None, cmd);
            job.config.image = Some(IMAGE.parse().unwrap());
            assert_eq!(
                build_benchmark_name(&job).unwrap().as_ref(),
                IMAGE,
                "{cmd:?}"
            );
        }
        // A command too long to name is an error, not a fallback.
        let long = "a".repeat(bencher_json::BenchmarkName::MAX_LEN - "make".len());
        let mut job = command_job(None, Some(&["make", long.as_str()]));
        job.config.image = Some(IMAGE.parse().unwrap());
        build_benchmark_name(&job).unwrap_err();
    }

    fn command_job(entrypoint: Option<&[&str]>, cmd: Option<&[&str]>) -> JsonClaimedJob {
        let to_owned = |args: &[&str]| args.iter().map(|&arg| arg.to_owned()).collect();
        test_job_with_options(
            1,
            mib_to_bytes(512),
            mib_to_bytes(1024),
            300,
            false,
            entrypoint.map(to_owned),
            cmd.map(to_owned),
            None,
            None,
        )
    }

    // --- output_to_iteration ---

    fn default_benchmark_name() -> bencher_json::BenchmarkName {
        "benchmark".parse().unwrap()
    }

    fn test_output(stdout: &str, output_files: Option<Vec<(&str, &[u8])>>) -> crate::RunOutput {
        crate::RunOutput {
            exit_code: 0,
            stdout: stdout.to_owned(),
            stderr: "some stderr".to_owned(),
            output_files: output_files.map(|files| {
                files
                    .into_iter()
                    .map(|(k, v)| (Utf8PathBuf::from(k), v.to_vec()))
                    .collect()
            }),
            duration: Duration::ZERO,
        }
    }

    #[test]
    fn output_no_metrics_no_files() {
        let output = test_output("hello stdout", None);
        let result = output_to_iteration(&discard(), output, false, false, None);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout.as_deref(), Some("hello stdout"));
        assert_eq!(result.stderr.as_deref(), Some("some stderr"));
        assert!(result.output.is_none());
    }

    #[test]
    fn output_no_metrics_with_files() {
        let output = test_output("hello", Some(vec![("/tmp/out.json", b"{\"data\":1}")]));
        let result = output_to_iteration(&discard(), output, false, false, None);
        assert_eq!(result.stdout.as_deref(), Some("hello"));
        let files = result.output.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(
            &files[Utf8PathBuf::from("/tmp/out.json").as_path()],
            "{\"data\":1}"
        );
    }

    #[test]
    fn output_build_time_only_no_files() {
        let name = default_benchmark_name();
        let output = crate::RunOutput {
            duration: Duration::from_millis(3140),
            ..test_output("hello", None)
        };
        let result = output_to_iteration(&discard(), output, true, false, Some(&name));
        assert_eq!(result.stdout.as_deref(), Some("hello"));
        let files = result.output.unwrap();
        assert_eq!(files.len(), 1);
        let bmf_json = files.values().next().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(bmf_json).unwrap();
        let benchmark = parsed.get("benchmark").unwrap();
        let build_time = benchmark.get("build-time").unwrap();
        let value = build_time.get("value").unwrap().as_f64().unwrap();
        #[expect(
            clippy::approx_constant,
            reason = "testing a specific rounded value, not pi"
        )]
        let expected = 3.14;
        assert!((value - expected).abs() < 0.01);
    }

    #[test]
    fn output_build_time_replaces_file_contents() {
        let name = default_benchmark_name();
        let output = crate::RunOutput {
            duration: Duration::from_secs(1),
            ..test_output("hello", Some(vec![("/tmp/out.json", b"file data")]))
        };
        let result = output_to_iteration(&discard(), output, true, false, Some(&name));
        assert_eq!(result.stdout.as_deref(), Some("hello"));
        let files = result.output.unwrap();
        assert!(files.values().any(|v| v.contains("build-time")));
        assert!(!files.values().any(|v| v.contains("file data")));
    }

    #[test]
    fn output_file_size_with_files() {
        let output = test_output("hello", Some(vec![("/tmp/out.bin", &[0u8; 1024])]));
        let result = output_to_iteration(&discard(), output, false, true, None);
        assert_eq!(result.stdout.as_deref(), Some("hello"));
        let files = result.output.unwrap();
        let bmf_json = files.values().next().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(bmf_json).unwrap();
        let file_entry = parsed.get("out.bin").unwrap();
        let file_size = file_entry.get("file-size").unwrap();
        let value = file_size.get("value").unwrap().as_f64().unwrap();
        assert!((value - 1024.0).abs() < f64::EPSILON);
    }

    #[test]
    fn output_file_size_of_a_shared_basename_is_the_last_declared_file() {
        // Each name is declared twice, the last 1 byte longer and sorting first, so a
        // sorted order picks the first for every name and a hash map's for about half.
        let names: Vec<String> = (0..32).map(|n| format!("{n}.bin")).collect();
        let first: Vec<String> = names.iter().map(|name| format!("/z/{name}")).collect();
        let last: Vec<String> = names.iter().map(|name| format!("/a/{name}")).collect();
        let files = first
            .iter()
            .map(|path| (path.as_str(), b"x".as_slice()))
            .chain(last.iter().map(|path| (path.as_str(), b"xx".as_slice())))
            .collect();
        let result = output_to_iteration(
            &discard(),
            test_output("hello", Some(files)),
            false,
            true,
            None,
        );
        let files = result.output.unwrap();
        let bmf_json = files.values().next().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(bmf_json).unwrap();
        let sizes: BTreeMap<String, Option<f64>> = parsed
            .as_object()
            .unwrap()
            .iter()
            .map(|(name, measures)| {
                let size = measures
                    .get("file-size")
                    .and_then(|metric| metric.get("value"))
                    .and_then(serde_json::Value::as_f64);
                (name.clone(), size)
            })
            .collect();
        let expected: BTreeMap<String, Option<f64>> =
            names.into_iter().map(|name| (name, Some(2.0))).collect();
        assert_eq!(sizes, expected);
    }

    #[test]
    fn output_file_size_no_files() {
        let output = test_output("hello", None);
        let result = output_to_iteration(&discard(), output, false, true, None);
        assert_eq!(result.stdout.as_deref(), Some("hello"));
        assert!(result.output.is_none());
    }

    #[test]
    fn output_build_time_and_file_size() {
        let name: bencher_json::BenchmarkName = "-c cargo bench".parse().unwrap();
        let output = crate::RunOutput {
            duration: Duration::from_millis(2500),
            ..test_output("hello", Some(vec![("/tmp/result.bin", &[0u8; 512])]))
        };
        let result = output_to_iteration(&discard(), output, true, true, Some(&name));
        assert_eq!(result.stdout.as_deref(), Some("hello"));
        let files = result.output.unwrap();
        let bmf_json = files.values().next().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(bmf_json).unwrap();
        assert!(
            parsed
                .get("-c cargo bench")
                .unwrap()
                .get("build-time")
                .is_some()
        );
        assert!(parsed.get("result.bin").unwrap().get("file-size").is_some());
    }

    #[test]
    fn output_build_time_wins_over_a_file_of_the_same_name() {
        let name: bencher_json::BenchmarkName = "make".parse().unwrap();
        let output = crate::RunOutput {
            duration: Duration::from_secs(2),
            ..test_output("hello", Some(vec![("/out/make", b"binary")]))
        };
        let result = output_to_iteration(&discard(), output, true, true, Some(&name));
        let files = result.output.unwrap();
        let bmf_json = files.values().next().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(bmf_json).unwrap();
        let measures: Vec<&str> = parsed
            .get("make")
            .unwrap()
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        assert_eq!(measures, ["build-time"]);
    }

    #[test]
    fn output_build_time_and_file_size_no_files() {
        let name = default_benchmark_name();
        let output = crate::RunOutput {
            duration: Duration::from_secs(5),
            ..test_output("hello", None)
        };
        let result = output_to_iteration(&discard(), output, true, true, Some(&name));
        assert_eq!(result.stdout.as_deref(), Some("hello"));
        let files = result.output.unwrap();
        let bmf_json = files.values().next().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(bmf_json).unwrap();
        assert!(parsed.get("benchmark").unwrap().get("build-time").is_some());
    }

    #[test]
    fn output_preserves_exit_code() {
        let name = default_benchmark_name();
        let output = crate::RunOutput {
            exit_code: 42,
            duration: Duration::from_secs(1),
            ..test_output("hello", None)
        };
        let result = output_to_iteration(&discard(), output, true, true, Some(&name));
        assert_eq!(result.exit_code, 42);
    }

    #[test]
    fn a_failed_command_reports_no_build_time() {
        // Kills a build time for a command that exited non-zero, which under
        // allow_failure lands in the series as the time the command took to
        // fail, handing on the files a build time Job replaces, and no metrics
        // map at all, which has the server parse the stdout and fail the Job.
        let name = default_benchmark_name();
        let output = crate::RunOutput {
            exit_code: 1,
            duration: Duration::from_secs(1),
            ..test_output("hello", Some(vec![("/tmp/out.json", b"file data")]))
        };
        let result = output_to_iteration(&discard(), output, true, false, Some(&name));
        assert_eq!(result.exit_code, 1);
        assert_eq!(result.output, Some(BTreeMap::new()));
    }

    #[test]
    fn output_preserves_stderr() {
        let name = default_benchmark_name();
        let output = crate::RunOutput {
            duration: Duration::from_secs(1),
            ..test_output("hello", None)
        };
        let result = output_to_iteration(&discard(), output, true, false, Some(&name));
        assert_eq!(result.stderr.as_deref(), Some("some stderr"));
    }

    #[test]
    fn output_build_time_uses_command_name() {
        let name: bencher_json::BenchmarkName = "/bin/sh -c cargo build".parse().unwrap();
        let output = crate::RunOutput {
            duration: Duration::from_millis(1500),
            ..test_output("hello", None)
        };
        let result = output_to_iteration(&discard(), output, true, false, Some(&name));
        let files = result.output.unwrap();
        let bmf_json = files.values().next().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(bmf_json).unwrap();
        assert!(parsed.get("/bin/sh -c cargo build").is_some());
        assert!(parsed.get("benchmark").is_none());
    }

    #[test]
    fn output_build_time_entrypoint_and_cmd_combined() {
        let name: bencher_json::BenchmarkName = "/bin/sh -c cargo bench".parse().unwrap();
        let output = crate::RunOutput {
            duration: Duration::from_millis(1500),
            ..test_output("hello", None)
        };
        let result = output_to_iteration(&discard(), output, true, false, Some(&name));
        let files = result.output.unwrap();
        let bmf_json = files.values().next().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(bmf_json).unwrap();
        assert!(parsed.get("/bin/sh -c cargo bench").is_some());
    }

    #[test]
    fn output_no_build_time_no_benchmark_name() {
        let output = crate::RunOutput {
            duration: Duration::from_millis(1500),
            ..test_output("hello", None)
        };
        let result = output_to_iteration(&discard(), output, true, false, None);
        // No build time without a name, and an empty map, so the stdout is not parsed.
        assert_eq!(result.output, Some(BTreeMap::new()));
    }

    // --- run_job ---

    #[test]
    fn a_job_runs_its_image_once() {
        // Kills a Job that runs its image more than once, or not at all, for
        // the 1 iteration the server sends.
        let registry = TestRegistry::start(None);
        let job = registry.job(false);
        let messages = crate::log::Messages::default();

        let outcome = run_test_job(&messages.logger(), &job, &AtomicBool::new(false)).finish(false);

        assert_eq!(
            registry.stop(),
            Requests {
                manifests: 1,
                blobs: 2
            }
        );
        assert_eq!(messages.count("Command finished"), 1, "the image ran once");
        let JobFinishResult::Completed { exit_code, results } = outcome else {
            panic!("a clean run completes, got: {outcome:?}");
        };
        let stdout: Vec<_> = results
            .iter()
            .map(|output| output.stdout.as_deref())
            .collect();
        assert_eq!((exit_code, stdout), (0, vec![Some("ran\n")]));
    }

    #[test]
    fn the_run_has_only_the_time_the_pull_left() {
        // Kills a run given a deadline of its own, which runs a benchmark past
        // the time the server gave the Job.
        let registry = TestRegistry::serve(
            None,
            b"#!/bin/sh\n/bin/sleep 2\necho ran\n",
            Duration::from_secs(2),
        );
        let job = registry.job(false);

        let outcome = run_job(
            &discard(),
            &job,
            &host_config(&job),
            &mut crate::jail::HostPreparation::new(),
            JobDeadline::start(Duration::from_secs(3)),
            &AtomicBool::new(false),
        )
        .finish(false);

        registry.stop();
        assert!(
            matches!(&outcome, JobFinishResult::Failed { error, results } if results.is_empty() && (error.contains("Benchmark timeout") || error.contains("timed out"))),
            "the 2s run outlasts the 1s the pull left: {outcome:?}"
        );
    }

    #[test]
    fn a_pull_that_fails_ends_the_job_before_its_run() {
        // Kills a failed pull skipped like a failed run under allow_failure,
        // which completes a Job that never ran.
        let registry = TestRegistry::start(None);
        let mut job = registry.job(true);
        job.config.digest = sha256(b"not in the registry").parse().unwrap();

        let run = run_test_job(&discard(), &job, &AtomicBool::new(false));

        assert_eq!(
            registry.stop(),
            Requests {
                manifests: 1,
                blobs: 0
            }
        );
        let outcome = run.finish(false);
        assert!(
            matches!(&outcome, JobFinishResult::Failed { error, results } if results.is_empty() && error.contains("404")),
            "the pull's failure ends the Job: {outcome:?}"
        );
    }

    #[test]
    fn a_cancel_during_the_pull_ends_the_job_before_its_run() {
        // Kills a run started once the server's cancel has landed during the
        // pull, which unpacks and spawns the benchmark of a Job the server has
        // ended.
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let registry = TestRegistry::start(Some(Arc::clone(&cancel_flag)));
        let job = registry.job(false);
        let messages = crate::log::Messages::default();

        let run = run_test_job(&messages.logger(), &job, &cancel_flag);

        assert_eq!(
            registry.stop().manifests,
            1,
            "the cancel landed in the pull"
        );
        assert!(
            !messages.contains("Starting run") && run.output.is_none(),
            "no run started: {:?}",
            run.output
        );
    }

    #[test]
    fn a_cancel_before_the_pull_pulls_nothing() {
        // Kills a pull for a Job canceled before its preparation starts, which
        // downloads the whole image for a Job the server has ended.
        let registry = TestRegistry::start(None);
        let job = registry.job(false);

        let run = run_test_job(&discard(), &job, &AtomicBool::new(true));

        assert_eq!(registry.stop(), Requests::default());
        assert!(run.output.is_none(), "nothing ran: {:?}", run.output);
    }

    #[test]
    fn no_tree_outlives_its_run_and_no_pull_its_image() {
        // Kills a tree a run leaves behind, and a pull's directory kept
        // once its image is dropped, either of which fills the runner's disk
        // Job after Job.
        let registry = TestRegistry::start(None);
        let config = host_config(&registry.job(false));
        let deadline = JobDeadline::start(Duration::from_mins(1));
        let image = crate::run::PulledImage::pull(&discard(), &config, deadline).unwrap();

        for _ in 0..2 {
            crate::local::local_execute(&discard(), &config, &image, None, deadline).unwrap();
            let left: Vec<String> = std::fs::read_dir(image.work_dir())
                .unwrap()
                .map(|entry| entry.unwrap().file_name().into_string().unwrap())
                .collect();
            assert_eq!(left, ["oci-image"], "only the pull outlives a run");
        }
        let work_dir = image.work_dir().to_owned();
        drop(image);
        registry.stop();
        assert!(!work_dir.exists(), "{work_dir} outlived its image");
    }

    #[test]
    fn a_second_job_downloads_no_blob_the_store_holds() {
        // Kills a Job that pulls past the runner's store, which downloads every
        // blob of an image the runner has run before.
        let state_dir = tempfile::tempdir().unwrap();
        let state_dir = Utf8Path::from_path(state_dir.path()).unwrap();
        fill_store(state_dir);
        let registry = TestRegistry::start(None);

        let run = run_stored_job(&registry.job(false), state_dir);

        assert_eq!(
            registry.stop(),
            Requests {
                manifests: 1,
                blobs: 0
            }
        );
        assert_eq!(
            run.output.and_then(|output| output.stdout).as_deref(),
            Some("ran\n"),
            "{:?}",
            run.failure
        );
    }

    #[test]
    fn a_job_whose_token_is_refused_fails_with_every_blob_in_the_store() {
        // Kills serving the manifest from the store, or skipping its request
        // when the store holds every blob, either of which runs the image of a
        // Job whose token the registry refuses.
        let state_dir = tempfile::tempdir().unwrap();
        let state_dir = Utf8Path::from_path(state_dir.path()).unwrap();
        fill_store(state_dir);
        let registry = TestRegistry::start(None);
        let mut job = registry.job(false);
        let (claims, _signature) = OCI_TOKEN.rsplit_once('.').unwrap();
        job.oci_token = format!("{claims}.c2lnbmVk").parse().unwrap();

        let outcome = run_stored_job(&job, state_dir).finish(false);

        assert_eq!(
            registry.stop(),
            Requests {
                manifests: 1,
                blobs: 0
            }
        );
        assert!(
            matches!(&outcome, JobFinishResult::Failed { error, results } if results.is_empty() && error.contains("401")),
            "the refused manifest ends the Job: {outcome:?}"
        );
    }

    #[test]
    fn a_job_keeps_no_blob_in_a_state_directory_not_its_own() {
        // Kills writing the store without taking the state directory first,
        // which fills a directory somebody else owns with blobs every later Job
        // trusts.
        let foreign = tempfile::tempdir().unwrap();
        let foreign = Utf8Path::from_path(foreign.path()).unwrap();
        std::fs::create_dir(foreign.join("dpkg")).unwrap();
        let registry = TestRegistry::start(None);

        let outcome = run_stored_job(&registry.job(false), foreign).finish(false);

        assert_eq!(registry.stop(), Requests::default());
        assert!(
            matches!(&outcome, JobFinishResult::Failed { error, results } if results.is_empty() && error.contains("dpkg")),
            "the foreign state directory ends the Job: {outcome:?}"
        );
        assert!(!foreign.join("blobs").exists());
    }

    /// Runs the image once, so the store in `state_dir` holds its every blob.
    fn fill_store(state_dir: &Utf8Path) {
        let registry = TestRegistry::start(None);
        let run = run_stored_job(&registry.job(false), state_dir);
        assert_eq!(
            registry.stop(),
            Requests {
                manifests: 1,
                blobs: 2
            }
        );
        assert!(run.failure.is_none(), "{:?}", run.failure);
    }

    /// Runs `job` as the runner does with its blob store in `state_dir`.
    fn run_stored_job(job: &JsonClaimedJob, state_dir: &Utf8Path) -> Run {
        let up_config = UpConfig {
            cpu_layout: None,
            image_cache_size: NonZeroU64::new(1 << 30),
            state_dir: state_dir.to_owned(),
            ..test_up_config()
        };
        run_job(
            &discard(),
            job,
            &build_config_from_job(&up_config, job).unwrap(),
            &mut crate::jail::HostPreparation::new(),
            JobDeadline::start(Duration::from_mins(1)),
            &AtomicBool::new(false),
        )
    }

    /// Runs `job` on the host, with no CPU isolation.
    fn run_test_job(log: &Logger, job: &JsonClaimedJob, cancel_flag: &AtomicBool) -> Run {
        run_job(
            log,
            job,
            &host_config(job),
            &mut crate::jail::HostPreparation::new(),
            JobDeadline::start(Duration::from_mins(1)),
            cancel_flag,
        )
    }

    fn host_config(job: &JsonClaimedJob) -> crate::Config {
        let up_config = UpConfig {
            cpu_layout: None,
            ..test_up_config()
        };
        build_config_from_job(&up_config, job).unwrap()
    }

    /// A registry on localhost serving one image, by default one whose command
    /// prints `ran`, to the test Job's token alone, that counts what it is
    /// asked for.
    struct TestRegistry {
        address: SocketAddr,
        digest: String,
        release: mpsc::SyncSender<()>,
        server: JoinHandle<Requests>,
    }

    #[derive(Debug, Default, PartialEq, Eq)]
    struct Requests {
        manifests: usize,
        blobs: usize,
    }

    impl TestRegistry {
        /// Sets `cancel_flag`, if any, as it serves the manifest, as a cancel
        /// that lands during the pull does.
        fn start(cancel_flag: Option<Arc<AtomicBool>>) -> Self {
            Self::serve(cancel_flag, b"#!/bin/sh\necho ran\n", Duration::ZERO)
        }

        /// Serves an image whose `/bench` is `script`, with each manifest
        /// `manifest_delay` late.
        fn serve(
            cancel_flag: Option<Arc<AtomicBool>>,
            script: &[u8],
            manifest_delay: Duration,
        ) -> Self {
            let layer = bench_layer(script);
            let config = serde_json::to_vec(&serde_json::json!({
                "architecture": "amd64",
                "os": "linux",
                "config": { "Cmd": ["/bench"] },
                "rootfs": { "type": "layers", "diff_ids": [sha256(&layer)] },
            }))
            .unwrap();
            let manifest = serde_json::to_vec(&serde_json::json!({
                "schemaVersion": 2,
                "mediaType": "application/vnd.oci.image.manifest.v1+json",
                "config": {
                    "mediaType": "application/vnd.oci.image.config.v1+json",
                    "digest": sha256(&config),
                    "size": config.len(),
                },
                "layers": [{
                    "mediaType": "application/vnd.oci.image.layer.v1.tar",
                    "digest": sha256(&layer),
                    "size": layer.len(),
                }],
            }))
            .unwrap();
            let digest = sha256(&manifest);
            let by_digest: HashMap<String, Vec<u8>> = [manifest, config, layer]
                .into_iter()
                .map(|bytes| (sha256(&bytes), bytes))
                .collect();
            let bearer = bencher_json::bearer_header(OCI_TOKEN);
            let listener = TcpListener::bind("127.0.0.1:0").unwrap();
            listener.set_nonblocking(true).unwrap();
            let address = listener.local_addr().unwrap();
            let (release, released) = mpsc::sync_channel(0);
            let server = std::thread::spawn(move || {
                let mut requests = Requests::default();
                while let Err(TryRecvError::Empty) = released.try_recv() {
                    let accepted = listener.accept();
                    if let Err(e) = &accepted
                        && e.kind() == ErrorKind::WouldBlock
                    {
                        std::thread::sleep(Duration::from_millis(10));
                        continue;
                    }
                    let (mut stream, _) = accepted.unwrap();
                    stream.set_nonblocking(false).unwrap();
                    let head = request_head(&mut stream);
                    let path = head.split(' ').nth(1).unwrap();
                    let (kind, digest) = path.rsplit_once('/').unwrap();
                    if kind.ends_with("/manifests") {
                        requests.manifests += 1;
                        if let Some(flag) = &cancel_flag {
                            flag.store(true, Ordering::SeqCst);
                        }
                        std::thread::sleep(manifest_delay);
                    } else {
                        requests.blobs += 1;
                    }
                    let authorized = head.lines().any(|line| {
                        line.split_once(':').is_some_and(|(name, value)| {
                            name.eq_ignore_ascii_case("authorization") && value.trim() == bearer
                        })
                    });
                    let (status, body) = match (authorized, by_digest.get(digest)) {
                        (false, _) => ("401 Unauthorized", [].as_slice()),
                        (true, None) => ("404 Not Found", [].as_slice()),
                        (true, Some(body)) => ("200 OK", body.as_slice()),
                    };
                    respond(&mut stream, status, body);
                }
                requests
            });
            Self {
                address,
                digest,
                release,
                server,
            }
        }

        /// A Job that runs the image.
        fn job(&self, allow_failure: bool) -> JsonClaimedJob {
            let mut job = test_job(1, mib_to_bytes(512), mib_to_bytes(1024), 60, false);
            job.config.registry = format!("http://{}", self.address).parse().unwrap();
            job.config.digest = self.digest.parse().unwrap();
            job.config.allow_failure = Some(allow_failure);
            job
        }

        /// Shuts the registry down and returns what it was asked for.
        fn stop(self) -> Requests {
            let Self {
                address: _,
                digest: _,
                release,
                server,
            } = self;
            drop(release);
            server.join().unwrap()
        }
    }

    /// A layer holding `/bench`, the executable `script`.
    fn bench_layer(script: &[u8]) -> Vec<u8> {
        let mut header = tar::Header::new_gnu();
        header.set_size(u64::try_from(script.len()).unwrap());
        header.set_mode(0o755);
        header.set_cksum();
        let mut layer = tar::Builder::new(Vec::new());
        layer.append_data(&mut header, "bench", script).unwrap();
        layer.into_inner().unwrap()
    }

    fn sha256(bytes: &[u8]) -> String {
        use sha2::Digest as _;

        format!("sha256:{}", hex::encode(sha2::Sha256::digest(bytes)))
    }

    fn request_head(stream: &mut TcpStream) -> String {
        stream
            .set_read_timeout(Some(Duration::from_secs(10)))
            .unwrap();
        let mut head = Vec::new();
        let mut byte = [0];
        while !head.ends_with(b"\r\n\r\n") {
            stream.read_exact(&mut byte).unwrap();
            head.extend_from_slice(&byte);
        }
        String::from_utf8(head).unwrap()
    }

    fn respond(stream: &mut TcpStream, status: &str, body: &[u8]) {
        let mut response = format!(
            "HTTP/1.1 {status}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .into_bytes();
        response.extend_from_slice(body);
        stream.write_all(&response).unwrap();
    }
}
