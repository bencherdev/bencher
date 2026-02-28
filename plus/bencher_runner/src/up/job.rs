#[cfg(target_os = "linux")]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "linux")]
use std::sync::{Arc, Mutex};
#[cfg(target_os = "linux")]
use std::time::Duration;

#[cfg(target_os = "linux")]
use bencher_json::JsonClaimedJob;

#[cfg(target_os = "linux")]
use super::UpConfig;
#[cfg(target_os = "linux")]
use super::error::{UpError, WebSocketError};
#[cfg(target_os = "linux")]
use super::websocket::JobChannel;
#[cfg(target_os = "linux")]
use bencher_json::runner::{JsonIterationOutput, RunnerMessage, ServerMessage};

pub enum JobOutcome {
    Completed {
        exit_code: i32,
        output: Option<String>,
    },
    Failed {
        error: String,
    },
    Canceled,
}

#[cfg(target_os = "linux")]
#[expect(clippy::print_stdout, clippy::print_stderr, clippy::use_debug)]
pub fn execute_job(
    config: &UpConfig,
    job: &JsonClaimedJob,
    ws_url: &url::Url,
) -> Result<JobOutcome, UpError> {
    println!("Connecting WebSocket for job {}...", job.uuid);
    let ws = JobChannel::connect(ws_url, config.token.as_ref())?;
    let ws = Arc::new(Mutex::new(ws));

    // Build runner Config from claimed job spec (all values from job spec, no defaults)
    let job_config = build_config_from_job(config, job);

    // Send Running status
    {
        let mut ws_guard = ws
            .lock()
            .map_err(|e| WebSocketError::Send(format!("Failed to lock WebSocket: {e}")))?;
        ws_guard.send_message(&RunnerMessage::Running)?;
    }

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::new(AtomicBool::new(false));

    // Spawn heartbeat thread, pinned to housekeeping cores
    let ws_heartbeat = Arc::clone(&ws);
    let cancel_heartbeat = Arc::clone(&cancel_flag);
    let stop_heartbeat = Arc::clone(&stop_flag);
    let housekeeping_cores = config.cpu_layout.housekeeping.clone();
    let heartbeat = std::thread::spawn(move || {
        // Pin this thread to housekeeping cores to avoid interfering with benchmarks
        if let Err(e) = crate::cpu::pin_current_thread(&housekeeping_cores) {
            eprintln!("Warning: failed to pin heartbeat thread to housekeeping cores: {e}");
        }
        heartbeat_loop(&ws_heartbeat, &cancel_heartbeat, &stop_heartbeat);
    });

    // Execute benchmark (blocking) — pass cancel_flag so the vsock poll loop
    // can abort early when the server sends a cancellation message.
    let result = crate::execute(&job_config, Some(&cancel_flag));

    // Stop heartbeat thread
    stop_flag.store(true, Ordering::SeqCst);
    if let Err(panic) = heartbeat.join() {
        eprintln!("Warning: heartbeat thread panicked: {panic:?}");
    }

    // Check if canceled
    if cancel_flag.load(Ordering::SeqCst) {
        println!("Job {} was canceled by server", job.uuid);
        let mut ws_guard = ws
            .lock()
            .map_err(|e| WebSocketError::Send(format!("Failed to lock WebSocket: {e}")))?;
        // Send Canceled message to notify server
        drop(ws_guard.send_message(&RunnerMessage::Canceled));
        ws_guard.close();
        return Ok(JobOutcome::Canceled);
    }

    // Send result
    let outcome = match result {
        Ok(output) => {
            // Convert output files from HashMap<Utf8PathBuf, Vec<u8>> to BTreeMap<Utf8PathBuf, String>
            let file_output = output.output_files.map(|files| {
                files
                    .into_iter()
                    .map(|(path, bytes)| (path, String::from_utf8_lossy(&bytes).into_owned()))
                    .collect::<std::collections::BTreeMap<_, _>>()
            });
            let stdout_preview = if output.stdout.is_empty() {
                None
            } else {
                Some(output.stdout.clone())
            };
            let iteration = JsonIterationOutput {
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
            };
            let msg = RunnerMessage::Completed {
                results: vec![iteration],
            };
            let mut ws_guard = ws
                .lock()
                .map_err(|e| WebSocketError::Send(format!("Failed to lock WebSocket: {e}")))?;
            ws_guard.send_message(&msg)?;
            // Wait for Ack with 5s timeout
            drop(ws_guard.read_message_timeout(Duration::from_secs(5)));
            ws_guard.close();
            JobOutcome::Completed {
                exit_code: output.exit_code,
                output: stdout_preview,
            }
        },
        Err(e) => {
            let error_msg = e.to_string();
            let msg = RunnerMessage::Failed {
                results: Vec::new(),
                error: error_msg.clone(),
            };
            let mut ws_guard = ws
                .lock()
                .map_err(|e| WebSocketError::Send(format!("Failed to lock WebSocket: {e}")))?;
            ws_guard.send_message(&msg)?;
            // Wait for Ack with 5s timeout
            drop(ws_guard.read_message_timeout(Duration::from_secs(5)));
            ws_guard.close();
            JobOutcome::Failed { error: error_msg }
        },
    };

    Ok(outcome)
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
#[cfg(target_os = "linux")]
fn build_config_from_job(up_config: &UpConfig, job: &JsonClaimedJob) -> crate::Config {
    let spec = &job.spec;
    let config = &job.config;

    // Build OCI image URL: registry/project/images@digest
    let registry_str = config.registry.as_ref().trim_end_matches('/');
    let oci_image = format!("{registry_str}/{}/images@{}", config.project, config.digest);

    let mut runner_config = crate::Config::new(oci_image)
        .with_token(job.oci_token.to_string())
        .with_vcpus(spec.cpu)
        .with_memory(spec.memory)
        .with_disk(spec.disk)
        .with_timeout_secs(u64::from(u32::from(config.timeout)))
        .with_network(spec.network)
        .with_entrypoint_opt(config.entrypoint.clone())
        .with_cmd_opt(config.cmd.clone())
        .with_env_opt(config.env.clone());

    // Pass all file paths through for multi-file output extraction
    runner_config = runner_config.with_file_paths_opt(config.file_paths.clone());

    // Pass through CPU layout for core isolation
    if up_config.cpu_layout.has_isolation() {
        runner_config = runner_config.with_cpu_layout(up_config.cpu_layout.clone());
    }

    // Pass through max output size if configured
    if let Some(max_output_size) = up_config.max_output_size {
        runner_config = runner_config.with_max_output_size(max_output_size);
    }

    // Pass through max file count if configured
    if let Some(max_file_count) = up_config.max_file_count {
        runner_config = runner_config.with_max_file_count(max_file_count);
    }

    // Pass through grace period if configured
    if let Some(grace_period) = up_config.grace_period {
        runner_config = runner_config.with_grace_period(grace_period);
    }

    // Pass through Firecracker log level
    runner_config.firecracker_log_level = up_config.firecracker_log_level;

    runner_config
}

#[cfg(target_os = "linux")]
fn heartbeat_loop(ws: &Arc<Mutex<JobChannel>>, cancel_flag: &AtomicBool, stop_flag: &AtomicBool) {
    loop {
        std::thread::sleep(Duration::from_secs(1));

        if stop_flag.load(Ordering::SeqCst) {
            break;
        }

        let Ok(mut ws_guard) = ws.lock() else { break };

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
            Ok(_) => {},
            Err(_) => break,
        }
    }
}

#[cfg(test)]
#[cfg(target_os = "linux")]
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
            token: "bencher_runner_test".parse().unwrap(),
            runner: "test-runner".to_owned(),
            poll_timeout_secs: 30,
            tuning: crate::TuningConfig::disabled(),
            cpu_layout: crate::cpu::CpuLayout::with_core_count(4),
            max_output_size: None,
            max_file_count: None,
            grace_period: None,
            firecracker_log_level: crate::firecracker::FirecrackerLogLevel::default(),
        }
    }

    #[test]
    fn uses_job_spec_vcpus() {
        let up_config = test_up_config();
        let job = test_job(4, mib_to_bytes(512), mib_to_bytes(1024), 300, false);
        let result = build_config_from_job(&up_config, &job);
        assert_eq!(result.vcpus, Cpu::try_from(4).unwrap());
    }

    #[test]
    fn converts_memory_from_job() {
        let up_config = test_up_config();
        let job = test_job(1, mib_to_bytes(2048), mib_to_bytes(1024), 300, false);
        let result = build_config_from_job(&up_config, &job);
        assert_eq!(result.memory, Memory::from_mib(2048).unwrap());
    }

    #[test]
    fn converts_disk_from_job() {
        let up_config = test_up_config();
        let job = test_job(1, mib_to_bytes(512), mib_to_bytes(10240), 300, false);
        let result = build_config_from_job(&up_config, &job);
        assert_eq!(result.disk, Disk::from_mib(10240).unwrap());
    }

    #[test]
    fn memory_preserves_bytes() {
        let up_config = test_up_config();
        // 512 MiB + 1 byte - strong type preserves exact byte value
        let job = test_job(1, mib_to_bytes(512) + 1, mib_to_bytes(1024), 300, false);
        let result = build_config_from_job(&up_config, &job);
        assert_eq!(result.memory.to_mib(), 513);
    }

    #[test]
    fn timeout_converts_u32_to_u64() {
        let up_config = test_up_config();
        let job = test_job(1, mib_to_bytes(512), mib_to_bytes(1024), 600, false);
        let result = build_config_from_job(&up_config, &job);
        assert_eq!(result.timeout_secs, 600);
    }

    #[test]
    fn builds_oci_image_url() {
        let up_config = test_up_config();
        let job = test_job(1, mib_to_bytes(512), mib_to_bytes(1024), 300, false);
        let result = build_config_from_job(&up_config, &job);
        assert_eq!(
            result.oci_image,
            "https://registry.bencher.dev/11111111-2222-3333-4444-555555555555/images@sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"
        );
    }

    #[test]
    fn oci_token_passed_through() {
        let up_config = test_up_config();
        let job = test_job(1, mib_to_bytes(512), mib_to_bytes(1024), 300, false);
        let result = build_config_from_job(&up_config, &job);
        assert!(
            result.token.is_some(),
            "OCI token should be passed to config"
        );
    }

    #[test]
    fn network_enabled() {
        let up_config = test_up_config();
        let job = test_job(1, mib_to_bytes(512), mib_to_bytes(1024), 300, true);
        let result = build_config_from_job(&up_config, &job);
        assert!(result.network);
    }

    #[test]
    fn network_disabled() {
        let up_config = test_up_config();
        let job = test_job(1, mib_to_bytes(512), mib_to_bytes(1024), 300, false);
        let result = build_config_from_job(&up_config, &job);
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
        let result = build_config_from_job(&up_config, &job);
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
        let result = build_config_from_job(&up_config, &job);
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
        let result = build_config_from_job(&up_config, &job);
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
        let result = build_config_from_job(&up_config, &job);
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
        let result = build_config_from_job(&up_config, &job);
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
        let result = build_config_from_job(&up_config, &job);
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
        up_config.cpu_layout = crate::cpu::CpuLayout::with_core_count(1);
        let job = test_job(1, mib_to_bytes(512), mib_to_bytes(1024), 300, false);
        let result = build_config_from_job(&up_config, &job);
        // CPU layout should not be passed through when no isolation is possible
        assert!(result.cpu_layout.is_none());
    }
}
