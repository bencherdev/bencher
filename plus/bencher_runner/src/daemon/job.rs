#[cfg(target_os = "linux")]
#[expect(clippy::print_stdout)]
use std::sync::atomic::{AtomicBool, Ordering};
#[cfg(target_os = "linux")]
use std::sync::{Arc, Mutex};
#[cfg(target_os = "linux")]
use std::time::Duration;

#[cfg(target_os = "linux")]
use super::DaemonConfig;
use super::api_client::ClaimedJob;
#[cfg(target_os = "linux")]
use super::error::{DaemonError, WebSocketError};
#[cfg(target_os = "linux")]
use super::websocket::{JobChannel, RunnerMessage, ServerMessage};
use crate::units::bytes_to_mib;

pub enum JobOutcome {
    Completed {
        exit_code: i32,
        output: Option<String>,
    },
    Failed {
        exit_code: Option<i32>,
        error: String,
    },
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

    // Build runner Config from claimed job spec (all values from job spec, no defaults)
    let job_config = build_config_from_job(job);

    // Send Running status
    {
        let mut ws_guard = ws
            .lock()
            .map_err(|e| WebSocketError::Send(format!("Failed to lock WebSocket: {e}")))?;
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
        let mut ws_guard = ws
            .lock()
            .map_err(|e| WebSocketError::Send(format!("Failed to lock WebSocket: {e}")))?;
        // Send Cancelled message to notify server
        drop(ws_guard.send_message(&RunnerMessage::Cancelled));
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
            let mut ws_guard = ws
                .lock()
                .map_err(|e| WebSocketError::Send(format!("Failed to lock WebSocket: {e}")))?;
            ws_guard.send_message(&msg)?;
            // Wait for Ack with 5s timeout
            drop(ws_guard.read_message_timeout(Duration::from_secs(5)));
            ws_guard.close();
            JobOutcome::Completed {
                exit_code: 0,
                output: None,
            }
        },
        Err(e) => {
            let error_msg = e.to_string();
            let msg = RunnerMessage::Failed {
                exit_code: None,
                error: error_msg.clone(),
            };
            let mut ws_guard = ws
                .lock()
                .map_err(|e| WebSocketError::Send(format!("Failed to lock WebSocket: {e}")))?;
            ws_guard.send_message(&msg)?;
            // Wait for Ack with 5s timeout
            drop(ws_guard.read_message_timeout(Duration::from_secs(5)));
            ws_guard.close();
            JobOutcome::Failed {
                exit_code: None,
                error: error_msg,
            }
        },
    };

    Ok(outcome)
}

/// Build a runner Config from the claimed job spec.
///
/// All values come directly from the job spec - there are no daemon defaults.
/// Memory and disk are converted from bytes (API) to MiB (Firecracker).
fn build_config_from_job(job: &ClaimedJob) -> crate::Config {
    let spec = &job.spec;

    // Convert bytes to MiB for Firecracker (rounds up)
    let memory_mib = bytes_to_mib(spec.memory);
    let disk_mib = bytes_to_mib(spec.disk);

    // Build OCI image URL: registry/project/images@digest
    let registry_str = spec.registry.as_str().trim_end_matches('/');
    let oci_image = format!("{registry_str}/{}/images@{}", spec.project, spec.digest);

    // vcpu is u32 in the spec, but Config expects u8
    // Clamp to u8::MAX if larger (unlikely in practice)
    let vcpus = if spec.vcpu > u32::from(u8::MAX) {
        u8::MAX
    } else {
        // This cast is safe because we just checked that vcpu <= u8::MAX
        #[expect(
            clippy::cast_possible_truncation,
            reason = "Checked that value fits in u8 above"
        )]
        let result = spec.vcpu as u8;
        result
    };

    crate::Config::new(oci_image)
        .with_vcpus(vcpus)
        .with_memory_mib(memory_mib)
        .with_disk_mib(disk_mib)
        .with_timeout_secs(u64::from(spec.timeout))
        .with_network(spec.network)
        .with_entrypoint_opt(spec.entrypoint.clone())
        .with_cmd_opt(spec.cmd.clone())
        .with_env_opt(spec.env.clone())
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
mod tests {
    use super::*;
    use crate::units::mib_to_bytes;

    fn test_job(
        vcpu: u32,
        memory_bytes: u64,
        disk_bytes: u64,
        timeout: u32,
        network: bool,
    ) -> ClaimedJob {
        let spec_json = serde_json::json!({
            "registry": "https://registry.bencher.dev",
            "project": "11111111-2222-3333-4444-555555555555",
            "digest": "sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3",
            "vcpu": vcpu,
            "memory": memory_bytes,
            "disk": disk_bytes,
            "timeout": timeout,
            "network": network,
        });
        let spec: super::super::api_client::JobSpec = serde_json::from_value(spec_json).unwrap();

        ClaimedJob {
            uuid: uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            spec,
            timeout_seconds: timeout,
        }
    }

    fn test_job_with_options(
        vcpu: u32,
        memory_bytes: u64,
        disk_bytes: u64,
        timeout: u32,
        network: bool,
        entrypoint: Option<Vec<String>>,
        cmd: Option<Vec<String>>,
        env: Option<std::collections::HashMap<String, String>>,
    ) -> ClaimedJob {
        let spec_json = serde_json::json!({
            "registry": "https://registry.bencher.dev",
            "project": "11111111-2222-3333-4444-555555555555",
            "digest": "sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3",
            "vcpu": vcpu,
            "memory": memory_bytes,
            "disk": disk_bytes,
            "timeout": timeout,
            "network": network,
            "entrypoint": entrypoint,
            "cmd": cmd,
            "env": env,
        });
        let spec: super::super::api_client::JobSpec = serde_json::from_value(spec_json).unwrap();

        ClaimedJob {
            uuid: uuid::Uuid::parse_str("550e8400-e29b-41d4-a716-446655440000").unwrap(),
            spec,
            timeout_seconds: timeout,
        }
    }

    // --- build_config_from_job ---

    #[test]
    fn uses_job_spec_vcpus() {
        let job = test_job(4, mib_to_bytes(512), mib_to_bytes(1024), 300, false);
        let result = build_config_from_job(&job);
        assert_eq!(result.vcpus, 4);
    }

    #[test]
    fn converts_memory_bytes_to_mib() {
        // 1 GiB in bytes = 1073741824
        let job = test_job(1, mib_to_bytes(2048), mib_to_bytes(1024), 300, false);
        let result = build_config_from_job(&job);
        assert_eq!(result.memory_mib, 2048);
    }

    #[test]
    fn converts_disk_bytes_to_mib() {
        // 10 GiB in bytes
        let job = test_job(1, mib_to_bytes(512), mib_to_bytes(10240), 300, false);
        let result = build_config_from_job(&job);
        assert_eq!(result.disk_mib, 10240);
    }

    #[test]
    fn memory_rounds_up() {
        // 512 MiB + 1 byte should round up to 513 MiB
        let job = test_job(1, mib_to_bytes(512) + 1, mib_to_bytes(1024), 300, false);
        let result = build_config_from_job(&job);
        assert_eq!(result.memory_mib, 513);
    }

    #[test]
    fn timeout_converts_u32_to_u64() {
        let job = test_job(1, mib_to_bytes(512), mib_to_bytes(1024), 600, false);
        let result = build_config_from_job(&job);
        assert_eq!(result.timeout_secs, 600);
    }

    #[test]
    fn builds_oci_image_url() {
        let job = test_job(1, mib_to_bytes(512), mib_to_bytes(1024), 300, false);
        let result = build_config_from_job(&job);
        assert_eq!(
            result.oci_image,
            "https://registry.bencher.dev/11111111-2222-3333-4444-555555555555/images@sha256:a665a45920422f9d417e4867efdc4fb8a04a1f3fff1fa07e998e86f7f7a27ae3"
        );
    }

    #[test]
    fn network_enabled() {
        let job = test_job(1, mib_to_bytes(512), mib_to_bytes(1024), 300, true);
        let result = build_config_from_job(&job);
        assert!(result.network);
    }

    #[test]
    fn network_disabled() {
        let job = test_job(1, mib_to_bytes(512), mib_to_bytes(1024), 300, false);
        let result = build_config_from_job(&job);
        assert!(!result.network);
    }

    #[test]
    fn entrypoint_and_cmd() {
        let job = test_job_with_options(
            1,
            mib_to_bytes(512),
            mib_to_bytes(1024),
            300,
            false,
            Some(vec!["/bin/sh".to_owned()]),
            Some(vec!["-c".to_owned(), "cargo bench".to_owned()]),
            None,
        );
        let result = build_config_from_job(&job);
        assert_eq!(result.entrypoint.unwrap(), vec!["/bin/sh"]);
        assert_eq!(result.cmd.unwrap(), vec!["-c", "cargo bench"]);
    }

    #[test]
    fn env_vars() {
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
        );
        let result = build_config_from_job(&job);
        let result_env = result.env.unwrap();
        assert_eq!(result_env.get("RUST_LOG").unwrap(), "debug");
        assert_eq!(result_env.get("CI").unwrap(), "true");
    }

    #[test]
    fn all_options() {
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
        );
        let result = build_config_from_job(&job);
        assert_eq!(result.vcpus, 8);
        assert_eq!(result.memory_mib, 4096);
        assert_eq!(result.disk_mib, 20480);
        assert_eq!(result.timeout_secs, 900);
        assert!(result.network);
        assert!(result.entrypoint.is_some());
        assert!(result.cmd.is_some());
        assert!(result.env.is_some());
    }
}
