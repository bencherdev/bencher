#[cfg(target_os = "linux")]
#[expect(clippy::print_stdout)]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "linux")]
use std::sync::{Arc, Mutex};
#[cfg(target_os = "linux")]
use std::time::Duration;

use super::api_client::ClaimedJob;
#[cfg(target_os = "linux")]
use super::error::{DaemonError, WebSocketError};
#[cfg(target_os = "linux")]
use super::websocket::{JobChannel, RunnerMessage, ServerMessage};
use super::DaemonConfig;

pub enum JobOutcome {
    Completed { exit_code: i32, output: Option<String> },
    Failed { exit_code: Option<i32>, error: String },
    Canceled,
}

#[cfg(target_os = "linux")]
#[expect(clippy::print_stdout)]
pub fn execute_job(
    config: &DaemonConfig,
    job: &ClaimedJob,
    ws_url: &url::Url,
) -> Result<JobOutcome, DaemonError> {
    println!("Connecting WebSocket for job {}...", job.uuid);
    let ws = JobChannel::connect(ws_url, &config.token)?;
    let ws = Arc::new(Mutex::new(ws));

    // Build runner Config from claimed job spec
    let job_config = build_config_from_job(config, job);

    // Send Running status
    {
        let mut ws_guard = ws.lock().map_err(|e| {
            WebSocketError::Send(format!("Failed to lock WebSocket: {e}"))
        })?;
        ws_guard.send_message(&RunnerMessage::Running)?;
    }

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let stop_flag = Arc::new(AtomicBool::new(false));

    // Spawn heartbeat thread
    let ws_heartbeat = Arc::clone(&ws);
    let cancel_heartbeat = Arc::clone(&cancel_flag);
    let stop_heartbeat = Arc::clone(&stop_flag);
    let heartbeat = std::thread::spawn(move || {
        heartbeat_loop(&ws_heartbeat, &cancel_heartbeat, &stop_heartbeat);
    });

    // Execute benchmark (blocking)
    let result = crate::execute(&job_config);

    // Stop heartbeat thread
    stop_flag.store(true, Ordering::SeqCst);
    drop(heartbeat.join());

    // Check if canceled
    if cancel_flag.load(Ordering::SeqCst) {
        println!("Job {} was canceled by server", job.uuid);
        let mut ws_guard = ws.lock().map_err(|e| {
            WebSocketError::Send(format!("Failed to lock WebSocket: {e}"))
        })?;
        ws_guard.close();
        return Ok(JobOutcome::Canceled);
    }

    // Send result
    let outcome = match result {
        Ok(output) => {
            let msg = RunnerMessage::Completed {
                exit_code: 0,
                output: Some(output),
            };
            let mut ws_guard = ws.lock().map_err(|e| {
                WebSocketError::Send(format!("Failed to lock WebSocket: {e}"))
            })?;
            ws_guard.send_message(&msg)?;
            // Wait for Ack with 5s timeout
            drop(ws_guard.read_message_timeout(Duration::from_secs(5)));
            ws_guard.close();
            JobOutcome::Completed {
                exit_code: 0,
                output: None,
            }
        }
        Err(e) => {
            let error_msg = e.to_string();
            let msg = RunnerMessage::Failed {
                exit_code: None,
                error: error_msg.clone(),
            };
            let mut ws_guard = ws.lock().map_err(|e| {
                WebSocketError::Send(format!("Failed to lock WebSocket: {e}"))
            })?;
            ws_guard.send_message(&msg)?;
            // Wait for Ack with 5s timeout
            drop(ws_guard.read_message_timeout(Duration::from_secs(5)));
            ws_guard.close();
            JobOutcome::Failed {
                exit_code: None,
                error: error_msg,
            }
        }
    };

    Ok(outcome)
}

fn build_config_from_job(
    daemon_config: &DaemonConfig,
    job: &ClaimedJob,
) -> crate::Config {
    let vcpus = job.spec.vcpus.unwrap_or(daemon_config.default_vcpus);
    let memory_mib = job.spec.memory_mib.unwrap_or(daemon_config.default_memory_mib);
    let timeout_secs = u64::from(job.spec.timeout_seconds);

    // Use the repository URL as the OCI image reference
    let oci_image = job.spec.repository.to_string();

    crate::Config::new(oci_image)
        .with_vcpus(vcpus)
        .with_memory_mib(memory_mib)
        .with_timeout_secs(timeout_secs)
}

#[cfg(target_os = "linux")]
fn heartbeat_loop(
    ws: &Arc<Mutex<JobChannel>>,
    cancel_flag: &AtomicBool,
    stop_flag: &AtomicBool,
) {
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
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuning::TuningConfig;

    fn test_daemon_config(vcpus: u8, memory_mib: u32) -> DaemonConfig {
        DaemonConfig {
            host: url::Url::parse("http://localhost:61016/").unwrap(),
            token: "bencher_runner_test".to_owned(),
            runner: "test-runner".to_owned(),
            labels: vec![],
            poll_timeout_secs: 55,
            tuning: TuningConfig::disabled(),
            default_vcpus: vcpus,
            default_memory_mib: memory_mib,
        }
    }

    fn test_job(vcpus: Option<u8>, memory_mib: Option<u32>, timeout: u32) -> ClaimedJob {
        let spec_json = serde_json::json!({
            "repository": "https://github.com/org/repo",
            "benchmark_command": "cargo bench",
            "timeout_seconds": timeout,
            "vcpus": vcpus,
            "memory_mib": memory_mib,
        });
        let spec: super::super::api_client::JobSpec =
            serde_json::from_value(spec_json).unwrap();

        ClaimedJob {
            uuid: uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            spec,
            timeout_seconds: timeout,
        }
    }

    // --- build_config_from_job ---

    #[test]
    fn uses_job_spec_vcpus_when_present() {
        let config = test_daemon_config(1, 512);
        let job = test_job(Some(4), None, 300);
        let result = build_config_from_job(&config, &job);
        assert_eq!(result.vcpus, 4);
    }

    #[test]
    fn falls_back_to_daemon_default_vcpus() {
        let config = test_daemon_config(2, 512);
        let job = test_job(None, None, 300);
        let result = build_config_from_job(&config, &job);
        assert_eq!(result.vcpus, 2);
    }

    #[test]
    fn uses_job_spec_memory_when_present() {
        let config = test_daemon_config(1, 512);
        let job = test_job(None, Some(2048), 300);
        let result = build_config_from_job(&config, &job);
        assert_eq!(result.memory_mib, 2048);
    }

    #[test]
    fn falls_back_to_daemon_default_memory() {
        let config = test_daemon_config(1, 1024);
        let job = test_job(None, None, 300);
        let result = build_config_from_job(&config, &job);
        assert_eq!(result.memory_mib, 1024);
    }

    #[test]
    fn timeout_converts_u32_to_u64() {
        let config = test_daemon_config(1, 512);
        let job = test_job(None, None, 600);
        let result = build_config_from_job(&config, &job);
        assert_eq!(result.timeout_secs, 600);
    }

    #[test]
    fn repository_url_becomes_oci_image() {
        let config = test_daemon_config(1, 512);
        let job = test_job(None, None, 300);
        let result = build_config_from_job(&config, &job);
        assert_eq!(result.oci_image, "https://github.com/org/repo");
    }

    #[test]
    fn both_overrides_present() {
        let config = test_daemon_config(1, 512);
        let job = test_job(Some(8), Some(4096), 900);
        let result = build_config_from_job(&config, &job);
        assert_eq!(result.vcpus, 8);
        assert_eq!(result.memory_mib, 4096);
        assert_eq!(result.timeout_secs, 900);
    }
}
