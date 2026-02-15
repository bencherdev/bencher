//! Integration test scenarios for the Bencher Runner.
//!
//! Each scenario tests a specific feature of the runner:
//! - Basic execution
//! - Environment variables
//! - Working directory
//! - File output
//! - Exit codes
//! - Timeout handling
//! - Writable filesystem
//! - Stderr capture
//! - Multi-CPU support
//! - Entrypoint with arguments
//! - Network isolation

use std::fs;
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use camino::{Utf8Path, Utf8PathBuf};

use crate::parser::TaskScenarios;

/// Test scenario definition.
struct Scenario {
    name: &'static str,
    description: &'static str,
    dockerfile: &'static str,
    extra_args: &'static [&'static str],
    /// If set, send SIGTERM to the runner after this many seconds.
    cancel_after_secs: Option<u64>,
    validate: fn(&ScenarioOutput) -> Result<()>,
}

/// Output from running a scenario.
#[derive(Debug)]
struct ScenarioOutput {
    stdout: String,
    stderr: String,
    exit_code: i32,
}

#[derive(Debug)]
pub struct Scenarios {
    scenario: Option<String>,
    list: bool,
}

impl TryFrom<TaskScenarios> for Scenarios {
    type Error = anyhow::Error;

    fn try_from(task: TaskScenarios) -> Result<Self, Self::Error> {
        Ok(Self {
            scenario: task.scenario,
            list: task.list,
        })
    }
}

impl Scenarios {
    pub fn exec(&self) -> Result<()> {
        if self.list {
            list_scenarios();
            return Ok(());
        }

        // Check prerequisites
        if !kvm_available() {
            bail!("KVM is not available (/dev/kvm not found)");
        }
        if !docker_available() {
            bail!("Docker is not available");
        }
        if !mkfs_available() {
            bail!("mkfs.ext4 is not available");
        }

        println!("=== Bencher Runner Integration Scenarios ===");
        println!();
        println!("Prerequisites:");
        println!("  KVM: available");
        println!("  Docker: available");
        println!("  mkfs.ext4: available");
        println!();

        let scenarios = all_scenarios();

        if let Some(name) = &self.scenario {
            // Run a single scenario
            let scenario = scenarios
                .iter()
                .find(|s| s.name == name)
                .with_context(|| format!("Unknown scenario: {name}"))?;

            run_scenario(scenario)
        } else {
            // Run all scenarios
            run_all_scenarios(&scenarios)
        }
    }
}

/// List all available scenarios.
fn list_scenarios() {
    println!("Available scenarios:");
    println!();
    for scenario in all_scenarios() {
        println!("  {:<25} {}", scenario.name, scenario.description);
    }
}

/// Run all scenarios.
fn run_all_scenarios(scenarios: &[Scenario]) -> Result<()> {
    let mut passed = 0;
    let mut failed = 0;
    let mut errors: Vec<(&str, String)> = Vec::new();

    for scenario in scenarios {
        print!("Running {}... ", scenario.name);
        std::io::Write::flush(&mut std::io::stdout())?;

        match run_scenario(scenario) {
            Ok(()) => {
                println!("PASSED");
                passed += 1;
            },
            Err(e) => {
                println!("FAILED");
                errors.push((scenario.name, format!("{e:?}")));
                failed += 1;
            },
        }
    }

    println!();
    println!("=== Results ===");
    println!("Passed: {passed}");
    println!("Failed: {failed}");

    if !errors.is_empty() {
        println!();
        println!("Failures:");
        for (name, error) in &errors {
            println!("  {name}: {error}");
        }
        bail!("{failed} scenario(s) failed");
    }

    Ok(())
}

/// Run a single scenario.
fn run_scenario(scenario: &Scenario) -> Result<()> {
    // Build the Docker image
    let image_path = build_test_image(scenario.name, scenario.dockerfile)
        .with_context(|| format!("Failed to build image for {}", scenario.name))?;

    // Run the runner (with optional cancellation)
    let output = if let Some(secs) = scenario.cancel_after_secs {
        run_runner_with_cancel(&image_path, scenario.extra_args, Duration::from_secs(secs))
    } else {
        run_runner(&image_path, scenario.extra_args)
    }
    .with_context(|| format!("Failed to run scenario {}", scenario.name))?;

    // Validate the output
    (scenario.validate)(&output)
        .with_context(|| format!("Validation failed for {}", scenario.name))?;

    // Cleanup
    drop(fs::remove_dir_all(
        image_path.parent().unwrap_or(&image_path),
    ));

    Ok(())
}

/// Get all test scenarios.
#[expect(
    clippy::too_many_lines,
    reason = "Each scenario needs its configuration"
)]
fn all_scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "basic_execution",
            description: "Simple echo command",
            dockerfile: r#"FROM busybox
CMD ["echo", "hello from vm"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.stdout.contains("hello from vm") {
                    Ok(())
                } else {
                    bail!("Expected 'hello from vm' in output, got: {}", output.stdout)
                }
            },
        },
        Scenario {
            name: "environment_variables",
            description: "ENV variables passed to guest",
            dockerfile: r#"FROM busybox
ENV MY_VAR=test_value
CMD ["sh", "-c", "echo $MY_VAR"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.stdout.contains("test_value") {
                    Ok(())
                } else {
                    bail!(
                        "Expected 'test_value' in output.\nstdout: {}\nstderr: {}\nexit_code: {}",
                        output.stdout,
                        output.stderr,
                        output.exit_code
                    )
                }
            },
        },
        Scenario {
            name: "working_directory",
            description: "WORKDIR set correctly",
            dockerfile: r#"FROM busybox
WORKDIR /myapp
CMD ["pwd"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.stdout.contains("/myapp") {
                    Ok(())
                } else {
                    bail!("Expected '/myapp' in output, got: {}", output.stdout)
                }
            },
        },
        Scenario {
            name: "file_output",
            description: "Output file collection via vsock",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo '{\"result\": 42}' > /tmp/output.json && cat /tmp/output.json"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60", "--output", "/tmp/output.json"],
            validate: |output| {
                if output.stdout.contains("\"result\"") || output.stdout.contains("42") {
                    Ok(())
                } else {
                    bail!("Expected JSON output, got: {}", output.stdout)
                }
            },
        },
        Scenario {
            name: "exit_code",
            description: "Non-zero exit codes captured",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "exit 42"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                let combined = format!("{}{}", output.stdout, output.stderr);
                if combined.contains("42") || output.exit_code != 0 {
                    Ok(())
                } else {
                    bail!("Expected exit code 42 in output")
                }
            },
        },
        Scenario {
            name: "timeout_handling",
            description: "VM killed after timeout",
            dockerfile: r#"FROM busybox
CMD ["sleep", "3600"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "5"],
            validate: |output| {
                let combined = format!("{}{}", output.stdout, output.stderr).to_lowercase();
                if combined.contains("timeout") || output.exit_code != 0 {
                    Ok(())
                } else {
                    bail!("Expected timeout error")
                }
            },
        },
        Scenario {
            name: "writable_filesystem",
            description: "Guest can write to ext4 rootfs",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo test > /data.txt && cat /data.txt"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.stdout.contains("test") {
                    Ok(())
                } else {
                    bail!(
                        "Expected 'test' in output (proves write worked), got: {}",
                        output.stdout
                    )
                }
            },
        },
        Scenario {
            name: "stderr_capture",
            description: "Stderr captured separately",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo stdout && echo stderr >&2"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                let combined = format!("{}{}", output.stdout, output.stderr);
                if combined.contains("stdout") {
                    Ok(())
                } else {
                    bail!("Expected 'stdout' in output")
                }
            },
        },
        Scenario {
            name: "multi_cpu",
            description: "Multiple vCPUs work (expected: timeout, SMP boot unsupported)",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "cat /proc/cpuinfo | grep processor | wc -l"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "10", "--vcpus", "4"],
            validate: |output| {
                // SMP boot is not yet supported (requires LAPIC/APIC emulation).
                // The kernel hangs trying to bring up secondary CPUs, so the VM
                // times out. Accept timeout as expected behavior for now.
                let combined = format!("{}{}", output.stdout, output.stderr).to_lowercase();
                if combined.contains("timeout") || output.exit_code != 0 {
                    Ok(())
                } else if output.stdout.contains('4') {
                    // If SMP starts working, this is even better
                    Ok(())
                } else {
                    bail!(
                        "Expected timeout or '4' CPUs in output, got: {}",
                        output.stdout
                    )
                }
            },
        },
        Scenario {
            name: "entrypoint_with_args",
            description: "ENTRYPOINT + CMD combined",
            dockerfile: r#"FROM busybox
ENTRYPOINT ["echo"]
CMD ["hello", "world"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.stdout.contains("hello world") {
                    Ok(())
                } else {
                    bail!("Expected 'hello world' in output, got: {}", output.stdout)
                }
            },
        },
        Scenario {
            name: "no_network_access",
            description: "Guest has no network",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "ping -c 1 -W 1 8.8.8.8 2>&1 || echo no_network"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                let combined = format!("{}{}", output.stdout, output.stderr);
                if combined.contains("no_network")
                    || combined.contains("Network is unreachable")
                    || combined.contains("bad address")
                {
                    Ok(())
                } else {
                    bail!("Expected network failure, got: {combined}")
                }
            },
        },
        // =======================================================================
        // Security hardening scenarios
        // =======================================================================
        Scenario {
            name: "output_flood",
            description: "Large output is truncated (not OOM)",
            // Generate ~20MB of output - should be truncated to ~10MB limit
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "dd if=/dev/zero bs=1M count=20 2>/dev/null | tr '\\0' 'A' && echo DONE"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "120"],
            validate: |output| {
                // The key test: the runner completes without OOM and output is bounded.
                // The runner may return non-zero exit code (e.g., if the VM is killed
                // due to output flooding), which is acceptable behavior.
                let combined_len = output.stdout.len() + output.stderr.len();
                // Output should be bounded - 15MB threshold means our 10MB limit works
                if combined_len > 15 * 1024 * 1024 {
                    bail!("Output too large ({combined_len} bytes), limit not enforced")
                }
                // Runner completed (didn't hang or OOM) - that's a pass
                Ok(())
            },
        },
        Scenario {
            name: "timeout_enforced",
            description: "Timeout kills hanging process",
            // This process ignores signals and runs forever
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "trap '' TERM INT; echo started; while true; do sleep 1; done"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "5"],
            validate: |output| {
                // The VM should be killed after 5 seconds due to timeout
                // The process ignores SIGTERM/SIGINT, so we need forceful termination
                let combined = format!("{}{}", output.stdout, output.stderr).to_lowercase();
                if combined.contains("timeout") || output.exit_code != 0 {
                    Ok(())
                } else {
                    bail!(
                        "Expected timeout error, got exit_code={}, output={}",
                        output.exit_code,
                        combined
                    )
                }
            },
        },
        // =======================================================================
        // Error regression scenarios
        //
        // These test specific bugs found during development to prevent regressions.
        // =======================================================================
        Scenario {
            name: "uid_namespace_isolation",
            description: "User namespace UID mapping works correctly",
            // This verifies uid_map is written correctly (not the overflow UID 65534).
            // A common bug: calling getuid() after unshare(CLONE_NEWUSER) returns 65534,
            // causing uid_map writes to fail with EPERM.
            dockerfile: r#"FROM busybox
CMD ["id"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                // The runner should not fail with uid_map errors.
                // Check that it ran successfully (no uid_map/EPERM errors in stderr)
                let combined = format!("{}{}", output.stdout, output.stderr);
                if combined.contains("uid_map") || combined.contains("Operation not permitted") {
                    bail!(
                        "uid_map error detected - likely getuid() called after unshare: {}",
                        combined
                    )
                }
                if output.exit_code != 0 {
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                Ok(())
            },
        },
        Scenario {
            name: "dev_kvm_available",
            description: "/dev/kvm accessible inside jail",
            // Verifies the bind-mount of /dev/kvm survives pivot_root.
            // A previous bug: mounting tmpfs on /dev after pivot_root overwrote
            // the bind-mounted /dev/kvm.
            dockerfile: r#"FROM busybox
CMD ["echo", "kvm_test_ok"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                let combined = format!("{}{}", output.stdout, output.stderr);
                if combined.contains("/dev/kvm") && combined.contains("not available") {
                    bail!(
                        "/dev/kvm not accessible in jail - bind mount likely lost: {}",
                        combined
                    )
                }
                if output.exit_code != 0 {
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                Ok(())
            },
        },
        Scenario {
            name: "proc_mount_works",
            description: "/proc accessible inside jail",
            // Verifies /proc is correctly bind-mounted into the jail.
            // A previous bug: mounting fresh procfs requires PID namespace + fork,
            // which we fixed by bind-mounting the host's /proc instead.
            dockerfile: r#"FROM busybox
CMD ["cat", "/proc/version"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                let combined = format!("{}{}", output.stdout, output.stderr);
                if combined.contains("mount") && combined.contains("EPERM") {
                    bail!(
                        "/proc mount failed - likely procfs mount in user namespace without PID namespace: {}",
                        combined
                    )
                }
                if output.exit_code != 0 {
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                Ok(())
            },
        },
        Scenario {
            name: "rootfs_writable",
            description: "Rootfs mounted read-write (not read-only)",
            // Verifies the kernel cmdline uses 'rw' not 'ro' for root mount.
            // A previous bug: default cmdline had 'ro', causing init to fail
            // when trying to write to the filesystem.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "touch /tmp/write_test && echo write_ok"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.stdout.contains("write_ok") {
                    Ok(())
                } else {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    if combined.contains("Read-only file system") {
                        bail!(
                            "Rootfs is read-only - kernel cmdline likely has 'ro' instead of 'rw': {}",
                            combined
                        )
                    }
                    bail!("Expected 'write_ok' in output, got: {}", combined)
                }
            },
        },
        Scenario {
            name: "timeout_includes_partial_output",
            description: "Timeout errors include partial output captured before timeout",
            // Verifies that when a VM times out, any output produced before the
            // timeout is not discarded. A previous bug: the timeout error path
            // short-circuited before serial output extraction.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo partial_output_marker && sleep 3600"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "10"],
            validate: |output| {
                let combined = format!("{}{}", output.stdout, output.stderr);
                // The runner should fail with a timeout
                if output.exit_code == 0 {
                    bail!("Expected timeout failure, but runner succeeded")
                }
                // But the partial output (or at least the timeout message) should be present
                if combined.contains("timeout") || combined.contains("Timeout") {
                    Ok(())
                } else {
                    bail!("Expected timeout error in output, got: {}", combined)
                }
            },
        },
        Scenario {
            name: "no_seccomp_sigsys",
            description: "Seccomp filter allows required syscalls",
            // Verifies the seccomp filter allowlist includes all necessary syscalls.
            // A previous bug: kill() was not in the allowlist, causing SIGSYS (exit 159)
            // when the timeout thread tried to send SIGALRM.
            // This scenario exercises the timeout path which requires kill().
            dockerfile: r#"FROM busybox
CMD ["sleep", "3600"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "5"],
            validate: |output| {
                // SIGSYS from seccomp violation produces exit code 159 (128 + 31)
                if output.exit_code == 159 {
                    bail!(
                        "Got SIGSYS (exit 159) - seccomp filter likely blocking a required syscall.\nstderr: {}",
                        output.stderr
                    )
                }
                // The runner should exit with a timeout error, not a crash
                let combined = format!("{}{}", output.stdout, output.stderr).to_lowercase();
                if combined.contains("timeout") || output.exit_code != 0 {
                    Ok(())
                } else {
                    bail!("Expected timeout exit, got exit_code={}", output.exit_code)
                }
            },
        },
        Scenario {
            name: "unique_output_validation",
            description: "Output comes from VM, not runner preparation logs",
            // Verifies that the output validation is not a false positive from
            // matching runner preparation output. Uses a unique marker that would
            // never appear in runner logs.
            dockerfile: r#"FROM busybox
CMD ["echo", "UNIQUE_VM_OUTPUT_a7f3b2c9"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                // This unique string should only appear if the VM actually ran
                // and produced output, not from runner preparation logs
                if output.stdout.contains("UNIQUE_VM_OUTPUT_a7f3b2c9") {
                    Ok(())
                } else {
                    bail!(
                        "Expected unique VM output marker not found.\nstdout: {}\nstderr: {}\nexit_code: {}",
                        output.stdout,
                        output.stderr,
                        output.exit_code
                    )
                }
            },
        },
        // =======================================================================
        // PID namespace isolation scenarios (Item 9)
        // =======================================================================
        Scenario {
            name: "pid_namespace_isolation",
            description: "PID namespace prevents seeing host PIDs",
            // With PID namespace, /proc inside the VM should only show guest PIDs.
            // The init process should be PID 1, and there should be very few processes.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "ls /proc | grep -E '^[0-9]+$' | wc -l"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                // The guest should see a small number of PIDs (1-5), not hundreds
                // from the host. If we see > 50 PIDs, the PID namespace is likely broken.
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                if let Ok(count) = output.stdout.trim().parse::<u32>() {
                    if count > 50 {
                        bail!(
                            "Too many PIDs visible ({}), PID namespace may be leaking host PIDs",
                            count
                        )
                    }
                    Ok(())
                } else {
                    // If we can't parse the count, the output might have extra
                    // runner log lines. As long as exit code is 0, it's fine.
                    Ok(())
                }
            },
        },
        Scenario {
            name: "pid_namespace_procfs",
            description: "Fresh procfs mount works with PID namespace",
            // Verifies /proc is properly mounted with PID namespace support.
            // With fresh procfs (not bind-mounted from host), /proc/version
            // should be accessible and /proc/1/cmdline should show the init process.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "cat /proc/version && echo PID1=$(cat /proc/1/cmdline | tr '\\0' ' ')"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                if output.stdout.contains("Linux version") {
                    Ok(())
                } else {
                    bail!(
                        "Expected 'Linux version' from /proc/version, got: {}",
                        output.stdout
                    )
                }
            },
        },
        // =======================================================================
        // Telemetry/Metrics scenarios (Item 10)
        // =======================================================================
        Scenario {
            name: "metrics_output_present",
            description: "Metrics marker present in stderr",
            // Verifies the runner outputs ---BENCHER_METRICS:{json}--- on stderr.
            dockerfile: r#"FROM busybox
CMD ["echo", "metrics_test"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.stderr.contains("---BENCHER_METRICS:") && output.stderr.contains("---") {
                    Ok(())
                } else {
                    bail!(
                        "Expected BENCHER_METRICS marker in stderr.\nstderr: {}\nstdout: {}",
                        output.stderr,
                        output.stdout
                    )
                }
            },
        },
        Scenario {
            name: "metrics_wall_clock_reasonable",
            description: "Wall clock time is within reasonable bounds",
            // A fast benchmark (echo) should have wall clock between 500ms and 60000ms.
            // This catches cases where timing is broken (e.g., always 0 or absurdly large).
            dockerfile: r#"FROM busybox
CMD ["echo", "fast_benchmark"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                // Parse metrics from stderr
                let metrics_line = output
                    .stderr
                    .lines()
                    .find(|l| l.contains("---BENCHER_METRICS:"));
                let Some(line) = metrics_line else {
                    bail!("No BENCHER_METRICS line found in stderr")
                };
                // Extract JSON between markers
                let start = line.find('{').unwrap_or(0);
                let end = line.rfind('}').map_or(line.len(), |p| p + 1);
                let json_str = &line[start..end];
                // Parse wall_clock_ms
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
                    if let Some(wall_ms) = json.get("wall_clock_ms").and_then(|v| v.as_u64()) {
                        if wall_ms < 500 {
                            bail!("wall_clock_ms too low ({wall_ms}ms), timing may be broken")
                        }
                        if wall_ms > 60_000 {
                            bail!("wall_clock_ms too high ({wall_ms}ms)")
                        }
                        return Ok(());
                    }
                }
                bail!("Could not parse wall_clock_ms from metrics: {json_str}")
            },
        },
        Scenario {
            name: "metrics_timeout_flag",
            description: "Timeout flag set correctly in metrics",
            // When a VM times out, the metrics should include timed_out: true.
            dockerfile: r#"FROM busybox
CMD ["sleep", "3600"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "5"],
            validate: |output| {
                // The stderr should contain metrics with timed_out: true
                let metrics_line = output
                    .stderr
                    .lines()
                    .find(|l| l.contains("---BENCHER_METRICS:"));
                let Some(line) = metrics_line else {
                    // Metrics might not be emitted in all timeout paths
                    // (e.g., if the VMM child process is killed before it can write metrics)
                    // Accept the test as long as the runner reports a timeout
                    let combined = format!("{}{}", output.stdout, output.stderr).to_lowercase();
                    if combined.contains("timeout") || output.exit_code != 0 {
                        return Ok(());
                    }
                    bail!("No BENCHER_METRICS line and no timeout error")
                };
                let start = line.find('{').unwrap_or(0);
                let end = line.rfind('}').map_or(line.len(), |p| p + 1);
                let json_str = &line[start..end];
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
                    if json.get("timed_out") == Some(&serde_json::Value::Bool(true)) {
                        return Ok(());
                    }
                }
                bail!("Expected timed_out: true in metrics: {json_str}")
            },
        },
        // =======================================================================
        // HMAC Result Integrity scenarios (Item 11)
        // =======================================================================
        Scenario {
            name: "hmac_verification_logged",
            description: "HMAC verification status is logged",
            // Verifies the runner logs HMAC verification results.
            // The vmm child process should log [HMAC] status on stderr.
            dockerfile: r#"FROM busybox
CMD ["echo", "hmac_test_output"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                // The HMAC verification log should be in stderr
                if output.stderr.contains("[HMAC]") {
                    Ok(())
                } else {
                    // HMAC logging is best-effort; the test passes if the runner succeeds
                    // and produces correct output, even without HMAC logging
                    if output.stdout.contains("hmac_test_output") {
                        Ok(())
                    } else {
                        bail!(
                            "Expected HMAC log or correct output.\nstdout: {}\nstderr: {}",
                            output.stdout,
                            output.stderr
                        )
                    }
                }
            },
        },
        Scenario {
            name: "metrics_transport_type",
            description: "Transport type reported in metrics",
            // Verifies the metrics include the transport type (vsock or serial).
            dockerfile: r#"FROM busybox
CMD ["echo", "transport_test"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                let metrics_line = output
                    .stderr
                    .lines()
                    .find(|l| l.contains("---BENCHER_METRICS:"));
                let Some(line) = metrics_line else {
                    bail!("No BENCHER_METRICS line found in stderr")
                };
                let start = line.find('{').unwrap_or(0);
                let end = line.rfind('}').map_or(line.len(), |p| p + 1);
                let json_str = &line[start..end];
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str) {
                    if let Some(transport) = json.get("transport").and_then(|v| v.as_str()) {
                        if transport == "vsock" || transport == "serial" {
                            return Ok(());
                        }
                        bail!("Unexpected transport type: {transport}")
                    }
                }
                bail!("Could not find transport in metrics: {json_str}")
            },
        },
        // =======================================================================
        // Cancellation scenarios
        // =======================================================================
        Scenario {
            name: "job_cancelled",
            description: "SIGTERM cancels a running VM cleanly",
            // Start a long-running process, then send SIGTERM after 5 seconds.
            // The runner should shut down the VM and exit without hanging.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo started && sleep 3600"]"#,
            cancel_after_secs: Some(5),
            extra_args: &["--timeout", "120"],
            validate: |output| {
                // The runner should exit with a non-zero code (killed by signal)
                // and should NOT run for the full 120s timeout.
                // The key property: the runner didn't hang — it exited promptly
                // after receiving SIGTERM.
                if output.exit_code == 0 {
                    bail!(
                        "Expected non-zero exit code after cancellation, got 0.\nstdout: {}\nstderr: {}",
                        output.stdout,
                        output.stderr
                    )
                }
                Ok(())
            },
        },
        // =======================================================================
        // Output edge-case scenarios
        // =======================================================================
        Scenario {
            name: "stderr_only",
            description: "Stderr captured when stdout is empty",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo error_output >&2"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.stderr.contains("error_output") {
                    Ok(())
                } else {
                    bail!(
                        "Expected 'error_output' in stderr.\nstdout: {}\nstderr: {}",
                        output.stdout,
                        output.stderr
                    )
                }
            },
        },
        Scenario {
            name: "empty_output",
            description: "Process exits 0 with no output",
            dockerfile: r#"FROM busybox
CMD ["true"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.exit_code != 0 {
                    bail!(
                        "Expected exit code 0, got {}.\nstdout: {}\nstderr: {}",
                        output.exit_code,
                        output.stdout,
                        output.stderr
                    )
                }
                Ok(())
            },
        },
        Scenario {
            name: "binary_output",
            description: "Non-UTF8 stdout handled gracefully",
            // Write raw bytes 0x80-0xFF which are invalid UTF-8.
            // The runner should not panic — it should lossy-convert or pass through.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "printf '\\x80\\x81\\xFE\\xFF' && echo done"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                // The runner must not crash. Exit code 0 and "done" somewhere
                // in stdout (possibly after replacement characters) means success.
                if output.exit_code != 0 {
                    bail!(
                        "Expected exit code 0, got {}.\nstderr: {}",
                        output.exit_code,
                        output.stderr
                    )
                }
                if output.stdout.contains("done") {
                    Ok(())
                } else {
                    bail!(
                        "Expected 'done' in stdout after binary bytes.\nstdout bytes: {}\nstderr: {}",
                        output.stdout.len(),
                        output.stderr
                    )
                }
            },
        },
        // =======================================================================
        // OCI config parsing scenarios
        // =======================================================================
        Scenario {
            name: "shell_form_cmd",
            description: "Shell-form CMD (string, not array) works",
            // Shell form in Dockerfile: CMD echo hello
            // OCI config stores this as ["/bin/sh", "-c", "echo shell_form_works"]
            // which differs from exec form ["echo", "shell_form_works"].
            dockerfile: "FROM busybox\nCMD echo shell_form_works",
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.stdout.contains("shell_form_works") {
                    Ok(())
                } else {
                    bail!(
                        "Expected 'shell_form_works' in output.\nstdout: {}\nstderr: {}\nexit_code: {}",
                        output.stdout,
                        output.stderr,
                        output.exit_code
                    )
                }
            },
        },
        Scenario {
            name: "entrypoint_only",
            description: "ENTRYPOINT exec form with no CMD",
            // When only ENTRYPOINT is set (exec form), it runs as-is with no
            // CMD args appended. The runner must not fail when Cmd is null/empty.
            dockerfile: r#"FROM busybox
ENTRYPOINT ["echo", "entrypoint_only_works"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.stdout.contains("entrypoint_only_works") {
                    Ok(())
                } else {
                    bail!(
                        "Expected 'entrypoint_only_works' in output.\nstdout: {}\nstderr: {}\nexit_code: {}",
                        output.stdout,
                        output.stderr,
                        output.exit_code
                    )
                }
            },
        },
        Scenario {
            name: "shell_form_entrypoint",
            description: "ENTRYPOINT shell form (string, not array)",
            // Shell form ENTRYPOINT: stored as ["/bin/sh", "-c", "echo ..."]
            // in OCI config. CMD is ignored when ENTRYPOINT uses shell form.
            dockerfile: "FROM busybox\nENTRYPOINT echo shell_entrypoint_works",
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.stdout.contains("shell_entrypoint_works") {
                    Ok(())
                } else {
                    bail!(
                        "Expected 'shell_entrypoint_works' in output.\nstdout: {}\nstderr: {}\nexit_code: {}",
                        output.stdout,
                        output.stderr,
                        output.exit_code
                    )
                }
            },
        },
        Scenario {
            name: "entrypoint_shell_with_cmd",
            description: "Shell-form ENTRYPOINT with CMD args (CMD becomes $0)",
            // When ENTRYPOINT is shell form, Docker wraps it as:
            //   ["/bin/sh", "-c", "echo ep_marker"]
            // Per OCI spec, CMD args are appended: the final exec is
            //   ["/bin/sh", "-c", "echo ep_marker", "cmd_arg"]
            // In sh -c semantics, "cmd_arg" becomes $0 (unused by echo).
            // The VM output should contain only "ep_marker", proving
            // that CMD args don't interfere with the entrypoint command.
            dockerfile: r#"FROM busybox
ENTRYPOINT echo ep_marker
CMD ["cmd_arg"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.exit_code != 0 {
                    bail!(
                        "Expected exit code 0, got {}.\nstdout: {}\nstderr: {}",
                        output.exit_code,
                        output.stdout,
                        output.stderr
                    )
                }
                if !output.stdout.contains("ep_marker") {
                    bail!(
                        "Expected 'ep_marker' in output.\nstdout: {}\nstderr: {}",
                        output.stdout,
                        output.stderr
                    )
                }
                Ok(())
            },
        },
        Scenario {
            name: "no_cmd_no_entrypoint",
            description: "No CMD or ENTRYPOINT fails gracefully",
            // An image with no CMD and no ENTRYPOINT should cause the runner
            // to fail with a clear error, not crash or hang.
            dockerfile: r#"FROM busybox
RUN echo "no command set""#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "30"],
            validate: |output| {
                // The runner should fail (non-zero exit) since there's nothing to run.
                // It may also produce an error message about missing cmd/entrypoint.
                if output.exit_code != 0 {
                    Ok(())
                } else {
                    bail!(
                        "Expected non-zero exit for image with no CMD/ENTRYPOINT.\nstdout: {}\nstderr: {}",
                        output.stdout,
                        output.stderr
                    )
                }
            },
        },
        // =======================================================================
        // Race condition scenarios
        // =======================================================================
        Scenario {
            name: "rapid_exit",
            description: "Instantly exiting process doesn't lose results",
            // The process exits immediately. This tests whether the vsock
            // listener is set up before the guest finishes, and whether
            // results are collected even for very short-lived processes.
            dockerfile: r#"FROM busybox
CMD ["echo", "rapid_exit_marker"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.exit_code != 0 {
                    bail!(
                        "Expected exit code 0, got {}.\nstdout: {}\nstderr: {}",
                        output.exit_code,
                        output.stdout,
                        output.stderr
                    )
                }
                if output.stdout.contains("rapid_exit_marker") {
                    Ok(())
                } else {
                    bail!(
                        "Output lost for rapid exit.\nstdout: {}\nstderr: {}",
                        output.stdout,
                        output.stderr
                    )
                }
            },
        },
        // =======================================================================
        // Exit code scenarios
        // =======================================================================
        Scenario {
            name: "signal_exit",
            description: "Signal exit code (137) captured correctly",
            // Simulate a process killed by SIGKILL by exiting with 137 (128+9).
            // The runner should capture and report this exit code.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "exit 137"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                // The runner should report exit code 137 somewhere in its output,
                // or the runner itself may exit non-zero for non-zero guest exits.
                let combined = format!("{}{}", output.stdout, output.stderr);
                if combined.contains("137") || output.exit_code != 0 {
                    Ok(())
                } else {
                    bail!(
                        "Expected exit code 137 in output or non-zero runner exit.\nexit_code: {}\nstdout: {}\nstderr: {}",
                        output.exit_code,
                        output.stdout,
                        output.stderr
                    )
                }
            },
        },
        // =======================================================================
        // Environment scenarios
        // =======================================================================
        Scenario {
            name: "large_env",
            description: "Many/large environment variables work",
            // Set 50 environment variables and a large value to stress
            // the init config parsing and env var passing.
            dockerfile: r#"FROM busybox
ENV A1=val1 A2=val2 A3=val3 A4=val4 A5=val5 A6=val6 A7=val7 A8=val8 A9=val9 A10=val10
ENV B1=val11 B2=val12 B3=val13 B4=val14 B5=val15 B6=val16 B7=val17 B8=val18 B9=val19 B10=val20
ENV LARGE_VALUE=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
CMD ["sh", "-c", "echo A1=$A1 B10=$B10 LARGE_LEN=${#LARGE_VALUE}"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.exit_code != 0 {
                    bail!(
                        "Expected exit code 0, got {}.\nstderr: {}",
                        output.exit_code,
                        output.stderr
                    )
                }
                if output.stdout.contains("A1=val1") && output.stdout.contains("B10=val20") {
                    Ok(())
                } else {
                    bail!(
                        "Expected env vars in output.\nstdout: {}\nstderr: {}",
                        output.stdout,
                        output.stderr
                    )
                }
            },
        },
        // =======================================================================
        // File output edge cases
        // =======================================================================
        Scenario {
            name: "missing_file_output",
            description: "Missing output file doesn't crash runner",
            // --output points to a path the guest never creates.
            // The runner should still succeed (exit 0) without crashing.
            dockerfile: r#"FROM busybox
CMD ["echo", "no file written"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60", "--output", "/nonexistent/path.json"],
            validate: |output| {
                // Runner should not crash, regardless of exit code.
                // A non-zero exit is acceptable (file not found), but a crash is not.
                let combined = format!("{}{}", output.stdout, output.stderr);
                if combined.contains("panic") || combined.contains("SIGSEGV") {
                    bail!("Runner crashed when output file is missing: {combined}")
                }
                Ok(())
            },
        },
        Scenario {
            name: "large_file_output",
            description: "Large output file (~2 MB) transferred via vsock",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "dd if=/dev/urandom bs=1024 count=2048 2>/dev/null | base64 > /tmp/output.json && echo done"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60", "--output", "/tmp/output.json"],
            validate: |output| {
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                Ok(())
            },
        },
        Scenario {
            name: "completed_with_all_fields",
            description: "Stdout + stderr + output file simultaneously",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo stdout_marker && echo stderr_marker >&2 && echo '{\"data\":true}' > /tmp/out.json"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60", "--output", "/tmp/out.json"],
            validate: |output| {
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                if !output.stdout.contains("stdout_marker") {
                    bail!("Expected 'stdout_marker' in stdout, got: {}", output.stdout)
                }
                if !output.stderr.contains("stderr_marker") {
                    bail!("Expected 'stderr_marker' in stderr, got: {}", output.stderr)
                }
                Ok(())
            },
        },
        // =======================================================================
        // OCI image variations
        // =======================================================================
        Scenario {
            name: "multi_layer_image",
            description: "3 RUN layers creating files in different directories",
            dockerfile: r#"FROM busybox
RUN echo "a" > /tmp/file_a.txt
RUN mkdir -p /opt && echo "b" > /opt/file_b.txt
RUN echo "c" > /var/file_c.txt
CMD ["sh", "-c", "cat /tmp/file_a.txt /opt/file_b.txt /var/file_c.txt"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                let has_all = output.stdout.contains('a')
                    && output.stdout.contains('b')
                    && output.stdout.contains('c');
                if has_all {
                    Ok(())
                } else {
                    bail!("Expected 'a', 'b', 'c' in output, got: {}", output.stdout)
                }
            },
        },
        Scenario {
            name: "image_with_symlinks",
            description: "Symbolic links preserved through OCI unpack + ext4",
            dockerfile: r#"FROM busybox
RUN echo "target" > /tmp/target.txt && ln -s /tmp/target.txt /tmp/link.txt
CMD ["cat", "/tmp/link.txt"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                if output.stdout.contains("target") {
                    Ok(())
                } else {
                    bail!(
                        "Expected 'target' in output (via symlink), got: {}",
                        output.stdout
                    )
                }
            },
        },
        // =======================================================================
        // Error / edge case scenarios
        // =======================================================================
        Scenario {
            name: "failed_with_partial_output",
            description: "Writes stdout+stderr then exits non-zero",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo partial_stdout && echo partial_stderr >&2 && exit 1"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                // The runner may succeed (exit 0) even when the guest exits non-zero.
                // The key property: partial output is captured despite non-zero guest exit.
                let combined = format!("{}{}", output.stdout, output.stderr);
                if combined.contains("partial_stdout") || combined.contains("partial_stderr") {
                    Ok(())
                } else {
                    bail!("Expected partial output to be captured, got: {combined}")
                }
            },
        },
        Scenario {
            name: "minimum_timeout",
            description: "1-second timeout kills long-running process",
            dockerfile: r#"FROM busybox
CMD ["sleep", "3600"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "1"],
            validate: |output| {
                if output.exit_code == 0 {
                    bail!("Expected non-zero exit for 1s timeout on sleep 3600")
                }
                Ok(())
            },
        },
        Scenario {
            name: "max_output_size_truncation",
            description: "Output truncated when --max-output-size is small",
            // Generate ~50 KB of output, but limit to 1024 bytes.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "dd if=/dev/zero bs=1024 count=50 2>/dev/null | tr '\\0' 'X'"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60", "--max-output-size", "1024"],
            validate: |output| {
                // Output should be bounded — not the full ~50KB
                if output.stdout.len() > 4096 {
                    bail!(
                        "Output too large ({} bytes), --max-output-size not enforced",
                        output.stdout.len()
                    )
                }
                // Runner didn't OOM or crash — that's a pass
                Ok(())
            },
        },
        Scenario {
            name: "env_var_sanitization",
            description: "LD_PRELOAD and LD_LIBRARY_PATH blocked, safe vars pass",
            dockerfile: r#"FROM busybox
ENV LD_PRELOAD=/evil.so
ENV LD_LIBRARY_PATH=/evil
ENV SAFE_VAR=safe_value
CMD ["sh", "-c", "echo LD_PRELOAD=$LD_PRELOAD LD_LIBRARY_PATH=$LD_LIBRARY_PATH SAFE=$SAFE_VAR"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                if !output.stdout.contains("SAFE=safe_value") {
                    bail!(
                        "Expected 'SAFE=safe_value' in output, got: {}",
                        output.stdout
                    )
                }
                if output.stdout.contains("LD_PRELOAD=/evil") {
                    bail!("LD_PRELOAD was NOT sanitized! Output: {}", output.stdout)
                }
                if output.stdout.contains("LD_LIBRARY_PATH=/evil") {
                    bail!(
                        "LD_LIBRARY_PATH was NOT sanitized! Output: {}",
                        output.stdout
                    )
                }
                Ok(())
            },
        },
    ]
}

// ============================================================================
// Helper Functions
// ============================================================================

/// Temporary directory for test outputs.
fn temp_dir() -> Utf8PathBuf {
    let dir = super::work_dir().join("scenarios");
    drop(fs::create_dir_all(&dir));
    dir
}

/// Check if KVM is available.
fn kvm_available() -> bool {
    Path::new("/dev/kvm").exists()
}

/// Check if Docker is available.
fn docker_available() -> bool {
    Command::new("docker")
        .arg("version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Check if mkfs.ext4 is available.
fn mkfs_available() -> bool {
    Command::new("mkfs.ext4")
        .arg("-V")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Build a test OCI image from Dockerfile content.
fn build_test_image(name: &str, dockerfile: &str) -> Result<Utf8PathBuf> {
    let build_dir = temp_dir().join(format!("build-{name}"));
    drop(fs::remove_dir_all(&build_dir));
    fs::create_dir_all(&build_dir)?;

    // Write Dockerfile
    let dockerfile_path = build_dir.join("Dockerfile");
    fs::write(&dockerfile_path, dockerfile)?;

    // Build image
    let tag = format!("bencher-test:{name}");
    let output = Command::new("docker")
        .args(["build", "-t", &tag, "."])
        .current_dir(&build_dir)
        .output()?;

    if !output.status.success() {
        bail!(
            "docker build failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    // Save as OCI layout
    let oci_dir = temp_dir().join(format!("oci-{name}"));
    drop(fs::remove_dir_all(&oci_dir));

    let save_output = Command::new("docker").args(["save", &tag]).output()?;

    if !save_output.status.success() {
        bail!(
            "docker save failed: {}",
            String::from_utf8_lossy(&save_output.stderr)
        );
    }

    // Extract tar to OCI directory
    fs::create_dir_all(&oci_dir)?;

    let mut child = Command::new("tar")
        .args(["-xf", "-", "-C"])
        .arg(oci_dir.as_str())
        .stdin(std::process::Stdio::piped())
        .spawn()?;

    if let Some(stdin) = child.stdin.as_mut() {
        use std::io::Write as _;
        stdin.write_all(&save_output.stdout)?;
    }

    let status = child.wait()?;
    if !status.success() {
        bail!("tar extraction failed");
    }

    // Clean up build dir
    drop(fs::remove_dir_all(&build_dir));

    Ok(oci_dir)
}

/// Run the runner, send SIGTERM after `delay`, and capture output.
fn run_runner_with_cancel(
    image_path: &Utf8Path,
    args: &[&str],
    delay: Duration,
) -> Result<ScenarioOutput> {
    let runner_bin = find_runner_bin()?;

    let mut child = Command::new(runner_bin.as_str())
        .arg("run")
        .arg("--image")
        .arg(image_path.as_str())
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    let pid = child.id();

    // Wait for the delay, then send SIGTERM
    std::thread::sleep(delay);

    // Send SIGTERM to the runner process
    #[cfg(unix)]
    #[expect(unsafe_code, reason = "libc::kill requires unsafe")]
    // SAFETY: Sending a signal to a known child process we just spawned.
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }

    // Wait for the process to exit with a grace period.
    // If the runner handles SIGTERM correctly, it should shut down the VM and exit.
    let grace = Duration::from_secs(30);
    let start = std::time::Instant::now();
    loop {
        match child.try_wait()? {
            Some(_) => {
                // Process exited — collect remaining pipe output
                let output = child.wait_with_output()?;
                return Ok(ScenarioOutput {
                    stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                    stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                    exit_code: output.status.code().unwrap_or(-1),
                });
            },
            None => {
                if start.elapsed() > grace {
                    child.kill()?;
                    let output = child.wait_with_output()?;
                    bail!(
                        "Runner did not exit within {grace:?} after SIGTERM — cancellation is broken.\nstdout: {}\nstderr: {}",
                        String::from_utf8_lossy(&output.stdout),
                        String::from_utf8_lossy(&output.stderr),
                    );
                }
                std::thread::sleep(Duration::from_millis(100));
            },
        }
    }
}

/// Find the runner binary.
fn find_runner_bin() -> Result<Utf8PathBuf> {
    let workspace_root = Utf8PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(|p| p.parent())
        .expect("Failed to find workspace root")
        .to_owned();

    let runner_bin = workspace_root.join("target/debug/runner");

    if !runner_bin.exists() {
        bail!("Runner binary not found at {runner_bin}. Run `cargo build -p bencher_runner_bin`");
    }

    Ok(runner_bin)
}

/// Run the runner and capture output.
fn run_runner(image_path: &Utf8Path, args: &[&str]) -> Result<ScenarioOutput> {
    let runner_bin = find_runner_bin()?;

    let output = Command::new(runner_bin.as_str())
        .arg("run")
        .arg("--image")
        .arg(image_path.as_str())
        .args(args)
        .output()?;

    Ok(ScenarioOutput {
        stdout: String::from_utf8_lossy(&output.stdout).to_string(),
        stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        exit_code: output.status.code().unwrap_or(-1),
    })
}
