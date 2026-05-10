use std::collections::{BTreeMap, HashMap};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use bencher_json::JsonClaimedJob;
use bencher_json::runner::{JsonIterationOutput, RunnerMessage, ServerMessage};
use camino::Utf8PathBuf;

use super::UpConfig;
use super::state_machine::JobFinishResult;
use super::websocket::JobChannel;

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

#[expect(clippy::print_stdout, clippy::print_stderr, clippy::use_debug)]
pub fn execute_job(
    config: &UpConfig,
    job: &JsonClaimedJob,
    ws: &Arc<Mutex<JobChannel>>,
) -> JobFinishResult {
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
    let iter_count = job.config.iter.map_or(1, bencher_json::Iteration::as_usize);
    let allow_failure = job.config.allow_failure.unwrap_or_default();

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::new(AtomicBool::new(false));
    let heartbeat = spawn_heartbeat_thread(config, ws, &cancel_flag, &stop_flag);

    // Execute benchmark iterations — pass cancel_flag so the vsock poll loop
    // can abort early when the server sends a cancellation message.
    let mut results = Vec::with_capacity(iter_count);
    let mut last_exit_code = 0;
    let mut last_stdout_preview = None;
    let mut failed_error = None;

    let build_time = job_config.build_time;
    let file_size = job_config.file_size;
    let benchmark_name = if build_time {
        match build_benchmark_name(job) {
            Ok(name) => Some(name),
            Err(error) => {
                return JobFinishResult::Failed {
                    error,
                    results: Vec::new(),
                };
            },
        }
    } else {
        None
    };

    for iteration in 0..iter_count {
        if cancel_flag.load(Ordering::SeqCst) {
            break;
        }
        println!(
            "Starting iteration {}/{iter_count} for job {}",
            iteration + 1,
            job.uuid
        );
        let start = build_time.then(std::time::Instant::now);
        let result = crate::execute(&job_config, Some(&cancel_flag));
        let elapsed = start.map(|s| s.elapsed());
        match result {
            Ok(output) => {
                last_exit_code = output.exit_code;
                if !output.stdout.is_empty() {
                    last_stdout_preview = Some(output.stdout.clone());
                }
                let failed = output.exit_code != 0 && !allow_failure;
                results.push(output_to_iteration(
                    output,
                    elapsed,
                    file_size,
                    benchmark_name.as_ref(),
                ));
                if failed {
                    failed_error = Some(format!(
                        "Benchmark exited with non-zero exit code: {last_exit_code}"
                    ));
                    break;
                }
            },
            Err(e) if allow_failure => {
                eprintln!(
                    "Iteration {}/{iter_count} failed (allow_failure=true, skipping): {e}",
                    iteration + 1
                );
            },
            Err(e) => {
                failed_error = Some(e.to_string());
                break;
            },
        }
    }

    // Stop heartbeat thread
    stop_flag.store(true, Ordering::SeqCst);
    if let Err(panic) = heartbeat.join() {
        eprintln!("Warning: heartbeat thread panicked: {panic:?}");
    }

    // Failure takes priority over cancellation (matches original behavior:
    // if the benchmark failed *and* a cancel arrived, we report failure).
    if let Some(error) = failed_error {
        return JobFinishResult::Failed { error, results };
    }

    // Check if canceled
    if cancel_flag.load(Ordering::SeqCst) {
        println!("Job {} was canceled by server", job.uuid);
        return JobFinishResult::Canceled;
    }

    JobFinishResult::Completed {
        exit_code: last_exit_code,
        output: last_stdout_preview,
        results,
    }
}

/// Derive the benchmark name for build-time tracking from the job config.
///
/// Precedence: entrypoint+cmd joined string (matches CLI behavior),
/// then image reference, then project@digest as last resort.
fn build_benchmark_name(job: &JsonClaimedJob) -> Result<bencher_json::BenchmarkName, String> {
    let name: String = job
        .config
        .entrypoint
        .iter()
        .flatten()
        .chain(job.config.cmd.iter().flatten())
        .cloned()
        .collect::<Vec<_>>()
        .join(" ");
    if !name.is_empty() {
        return name
            .parse()
            .map_err(|e| format!("Invalid benchmark name for build time: {e}"));
    }
    if let Some(image) = &job.config.image {
        return image
            .to_string()
            .parse()
            .map_err(|e| format!("Invalid benchmark name for build time: {e}"));
    }
    format!("{}@{}", job.config.project, job.config.digest)
        .parse()
        .map_err(|e| format!("Invalid benchmark name for build time: {e}"))
}

fn output_to_iteration(
    output: crate::RunOutput,
    build_time: Option<Duration>,
    file_size: bool,
    benchmark_name: Option<&bencher_json::BenchmarkName>,
) -> JsonIterationOutput {
    let file_output =
        build_metric_output(build_time, file_size, output.output_files, benchmark_name);
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
    build_time: Option<Duration>,
    file_size: bool,
    output_files: Option<HashMap<Utf8PathBuf, Vec<u8>>>,
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

    if file_size && let Some(files) = &output_files {
        for (path, bytes) in files {
            if let Ok(name) = path.file_name().unwrap_or(path.as_str()).parse() {
                #[expect(clippy::cast_precision_loss)]
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
            }
        }
    }

    if metric_results.is_empty() {
        return None;
    }

    let results = JsonNewMetric::results(metric_results);
    let bmf_json = serde_json::to_string(&results).unwrap_or_default();
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
fn build_config_from_job(
    up_config: &UpConfig,
    job: &JsonClaimedJob,
) -> Result<crate::Config, crate::error::ConfigError> {
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

    // Pass through CPU layout for core isolation
    if let Some(cpu_layout) = &up_config.cpu_layout
        && cpu_layout.has_isolation()
    {
        runner_config = runner_config.with_cpu_layout(cpu_layout.clone());
    }

    // Pass through max output size if configured
    if let Some(max_output_size) = up_config.max_output_size {
        runner_config = runner_config.with_max_output_size(max_output_size);
    }

    // Pass through max file count if configured
    if let Some(max_file_count) = up_config.max_file_count {
        runner_config = runner_config.with_max_file_count(max_file_count);
    }

    // Pass through max symlinks if configured
    if let Some(max_symlinks) = up_config.max_symlinks {
        runner_config = runner_config.with_max_symlinks(max_symlinks);
    }

    // Pass through grace period if configured
    if let Some(grace_period) = up_config.grace_period {
        runner_config = runner_config.with_grace_period(grace_period);
    }

    // Pass through sandbox log level
    runner_config.sandbox_log_level = up_config.sandbox_log_level;

    // Pass through sandbox mode from the job spec
    runner_config = runner_config.with_sandbox(spec.sandbox);

    Ok(runner_config)
}

#[expect(clippy::print_stderr)]
fn spawn_heartbeat_thread(
    config: &UpConfig,
    ws: &Arc<Mutex<JobChannel>>,
    cancel_flag: &Arc<AtomicBool>,
    stop_flag: &Arc<AtomicBool>,
) -> std::thread::JoinHandle<()> {
    let ws_heartbeat = Arc::clone(ws);
    let cancel_heartbeat = Arc::clone(cancel_flag);
    let stop_heartbeat = Arc::clone(stop_flag);
    let housekeeping_cores = config
        .cpu_layout
        .as_ref()
        .map(|l| l.housekeeping.clone())
        .unwrap_or_default();
    std::thread::spawn(move || {
        if let Err(e) = crate::cpu::pin_current_thread(&housekeeping_cores) {
            eprintln!("Warning: failed to pin heartbeat thread to housekeeping cores: {e}");
        }
        heartbeat_loop(&ws_heartbeat, &cancel_heartbeat, &stop_heartbeat);
    })
}

#[expect(clippy::print_stderr, clippy::use_debug)]
fn heartbeat_loop(ws: &Arc<Mutex<JobChannel>>, cancel_flag: &AtomicBool, stop_flag: &AtomicBool) {
    loop {
        std::thread::sleep(Duration::from_secs(1));

        if stop_flag.load(Ordering::SeqCst) {
            break;
        }

        let mut ws_guard = match ws.lock() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("ERROR: heartbeat lock poisoned: {e}");
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
                eprintln!("Warning: unexpected {msg:?} during job execution heartbeat");
            },
            Err(_) => break,
        }
    }
}

#[cfg(test)]
#[expect(clippy::indexing_slicing, clippy::get_unwrap)]
mod tests {
    use super::*;
    use camino::Utf8PathBuf;

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

    #[expect(clippy::too_many_arguments, clippy::needless_pass_by_value)]
    fn test_job_with_options(
        cpu: u32,
        memory_bytes: u64,
        disk_bytes: u64,
        timeout: u32,
        network: bool,
        entrypoint: Option<Vec<String>>,
        cmd: Option<Vec<String>>,
        env: Option<std::collections::HashMap<String, String>>,
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
            "oci_token": "eyJhbGciOiJIUzI1NiIsInR5cCI6IkpXVCJ9.eyJzdWIiOiIxMjM0NTY3ODkwIiwibmFtZSI6IkpvaG4gRG9lIiwiYWRtaW4iOnRydWV9.TJVA95OrM7E2cBab30RMHrHDcEfxjoYZgeFONFh7HgQ",
            "timeout": timeout,
            "created": "2025-01-01T00:00:00Z"
        });
        serde_json::from_value(json).expect("Failed to construct test JsonClaimedJob")
    }

    // --- build_config_from_job ---

    fn test_up_config() -> UpConfig {
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
            grace_period: None,
            sandbox_log_level: crate::SandboxLogLevel::default(),
            allow_no_sandbox: false,
            no_auto_update: false,
            max_download_size: None,
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
                "registry": "http://localhost:61016",
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
            "localhost:61016/11111111-2222-3333-4444-555555555555@sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"
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
        let mut env = std::collections::HashMap::new();
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
        assert_eq!(result_env.get("RUST_LOG").unwrap(), "debug");
        assert_eq!(result_env.get("CI").unwrap(), "true");
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
        let mut env = std::collections::HashMap::new();
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
        assert!(check_sandbox_allowed(Some(bencher_json::Sandbox::Firecracker), false).is_ok());
        assert!(check_sandbox_allowed(Some(bencher_json::Sandbox::Firecracker), true).is_ok());
    }

    #[test]
    fn sandbox_none_rejected_without_flag() {
        assert!(check_sandbox_allowed(None, false).is_err());
    }

    #[test]
    fn sandbox_none_allowed_with_flag() {
        assert!(check_sandbox_allowed(None, true).is_ok());
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
        }
    }

    #[test]
    fn output_no_metrics_no_files() {
        let output = test_output("hello stdout", None);
        let result = output_to_iteration(output, None, false, None);
        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout.as_deref(), Some("hello stdout"));
        assert_eq!(result.stderr.as_deref(), Some("some stderr"));
        assert!(result.output.is_none());
    }

    #[test]
    fn output_no_metrics_with_files() {
        let output = test_output("hello", Some(vec![("/tmp/out.json", b"{\"data\":1}")]));
        let result = output_to_iteration(output, None, false, None);
        assert_eq!(result.stdout.as_deref(), Some("hello"));
        let files = result.output.unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(
            files
                .get(Utf8PathBuf::from("/tmp/out.json").as_path())
                .unwrap(),
            "{\"data\":1}"
        );
    }

    #[test]
    fn output_build_time_only_no_files() {
        let name = default_benchmark_name();
        let duration = Duration::from_millis(3140);
        let output = test_output("hello", None);
        let result = output_to_iteration(output, Some(duration), false, Some(&name));
        assert_eq!(result.stdout.as_deref(), Some("hello"));
        let files = result.output.unwrap();
        assert_eq!(files.len(), 1);
        let bmf_json = files.values().next().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(bmf_json).unwrap();
        let benchmark = parsed.get("benchmark").unwrap();
        let build_time = benchmark.get("build-time").unwrap();
        let value = build_time.get("value").unwrap().as_f64().unwrap();
        assert!((value - 3.14).abs() < 0.01);
    }

    #[test]
    fn output_build_time_replaces_file_contents() {
        let name = default_benchmark_name();
        let duration = Duration::from_secs(1);
        let output = test_output("hello", Some(vec![("/tmp/out.json", b"file data")]));
        let result = output_to_iteration(output, Some(duration), false, Some(&name));
        assert_eq!(result.stdout.as_deref(), Some("hello"));
        let files = result.output.unwrap();
        assert!(files.values().any(|v| v.contains("build-time")));
        assert!(!files.values().any(|v| v.contains("file data")));
    }

    #[test]
    fn output_file_size_with_files() {
        let output = test_output("hello", Some(vec![("/tmp/out.bin", &[0u8; 1024])]));
        let result = output_to_iteration(output, None, true, None);
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
    fn output_file_size_no_files() {
        let output = test_output("hello", None);
        let result = output_to_iteration(output, None, true, None);
        assert_eq!(result.stdout.as_deref(), Some("hello"));
        assert!(result.output.is_none());
    }

    #[test]
    fn output_build_time_and_file_size() {
        let name: bencher_json::BenchmarkName = "-c cargo bench".parse().unwrap();
        let duration = Duration::from_millis(2500);
        let output = test_output("hello", Some(vec![("/tmp/result.bin", &[0u8; 512])]));
        let result = output_to_iteration(output, Some(duration), true, Some(&name));
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
    fn output_build_time_and_file_size_no_files() {
        let name = default_benchmark_name();
        let duration = Duration::from_secs(5);
        let output = test_output("hello", None);
        let result = output_to_iteration(output, Some(duration), true, Some(&name));
        assert_eq!(result.stdout.as_deref(), Some("hello"));
        let files = result.output.unwrap();
        let bmf_json = files.values().next().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(bmf_json).unwrap();
        assert!(parsed.get("benchmark").unwrap().get("build-time").is_some());
    }

    #[test]
    fn output_preserves_exit_code() {
        let name = default_benchmark_name();
        let mut output = test_output("hello", None);
        output.exit_code = 42;
        let result = output_to_iteration(output, Some(Duration::from_secs(1)), true, Some(&name));
        assert_eq!(result.exit_code, 42);
    }

    #[test]
    fn output_preserves_stderr() {
        let name = default_benchmark_name();
        let output = test_output("hello", None);
        let result = output_to_iteration(output, Some(Duration::from_secs(1)), false, Some(&name));
        assert_eq!(result.stderr.as_deref(), Some("some stderr"));
    }

    #[test]
    fn output_build_time_uses_command_name() {
        let name: bencher_json::BenchmarkName = "/bin/sh -c cargo build".parse().unwrap();
        let duration = Duration::from_millis(1500);
        let output = test_output("hello", None);
        let result = output_to_iteration(output, Some(duration), false, Some(&name));
        let files = result.output.unwrap();
        let bmf_json = files.values().next().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(bmf_json).unwrap();
        assert!(parsed.get("/bin/sh -c cargo build").is_some());
        assert!(parsed.get("benchmark").is_none());
    }

    #[test]
    fn output_build_time_entrypoint_and_cmd_combined() {
        let name: bencher_json::BenchmarkName = "/bin/sh -c cargo bench".parse().unwrap();
        let duration = Duration::from_millis(1500);
        let output = test_output("hello", None);
        let result = output_to_iteration(output, Some(duration), false, Some(&name));
        let files = result.output.unwrap();
        let bmf_json = files.values().next().unwrap();
        let parsed: serde_json::Value = serde_json::from_str(bmf_json).unwrap();
        assert!(parsed.get("/bin/sh -c cargo bench").is_some());
    }

    #[test]
    fn output_no_build_time_no_benchmark_name() {
        let duration = Duration::from_millis(1500);
        let output = test_output("hello", None);
        let result = output_to_iteration(output, Some(duration), false, None);
        // build_time duration is set but no benchmark_name → no build-time metric
        assert!(result.output.is_none());
    }
}
