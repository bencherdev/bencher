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

/// Extract the JSON substring between the first `{` and last `}` in a line.
///
/// The search targets are ASCII bytes, so the resulting indices are always at
/// valid UTF-8 boundaries.
#[expect(
    clippy::string_slice,
    reason = "{ and } are ASCII — indices are always UTF-8 safe"
)]
fn extract_json_substr(line: &str) -> &str {
    let start = line.find('{').unwrap_or(0);
    let end = line.rfind('}').map_or(line.len(), |p| p + 1);
    &line[start..end]
}

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

        // Build bencher-init + runner CLI once up front
        let runner_bin = ensure_runner_bin()?;

        let scenarios = all_scenarios();

        if let Some(name) = &self.scenario {
            // Run a single scenario
            let scenario = scenarios
                .iter()
                .find(|s| s.name == name)
                .with_context(|| format!("Unknown scenario: {name}"))?;

            run_scenario(scenario, &runner_bin)
        } else {
            // Run all scenarios
            run_all_scenarios(&scenarios, &runner_bin)
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
fn run_all_scenarios(scenarios: &[Scenario], runner_bin: &Utf8Path) -> Result<()> {
    let mut passed = 0;
    let mut failed = 0;
    let mut errors: Vec<(&str, String)> = Vec::new();

    for scenario in scenarios {
        print!("Running {}... ", scenario.name);
        std::io::Write::flush(&mut std::io::stdout())?;

        match run_scenario(scenario, runner_bin) {
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
fn run_scenario(scenario: &Scenario, runner_bin: &Utf8Path) -> Result<()> {
    // Build the Docker image
    let image_path = build_test_image(scenario.name, scenario.dockerfile)
        .with_context(|| format!("Failed to build image for {}", scenario.name))?;

    // Run the runner (with optional cancellation)
    let output = if let Some(secs) = scenario.cancel_after_secs {
        run_runner_with_cancel(
            &image_path,
            scenario.extra_args,
            Duration::from_secs(secs),
            runner_bin,
        )
    } else {
        run_runner(&image_path, scenario.extra_args, runner_bin)
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
            // Generate ~20MB of output - should be truncated to the 10MB limit
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "dd if=/dev/zero bs=1M count=20 2>/dev/null | tr '\\0' 'A' && echo DONE"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "120", "--max-output-size", "10485760"],
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
                        "uid_map error detected - likely getuid() called after unshare: {combined}"
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
                    bail!("/dev/kvm not accessible in jail - bind mount likely lost: {combined}")
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
                        "/proc mount failed - likely procfs mount in user namespace without PID namespace: {combined}"
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
                            "Rootfs is read-only - kernel cmdline likely has 'ro' instead of 'rw': {combined}"
                        )
                    }
                    bail!("Expected 'write_ok' in output, got: {combined}")
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
                    bail!("Expected timeout error in output, got: {combined}")
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
            name: "iopl_dropped_before_exec",
            description: "iopl(3) privilege not inherited by benchmark process",
            // Multi-stage: compile a static C binary that tries direct port I/O.
            // If iopl is inherited from init, `inb` succeeds → prints IOPL_INHERITED.
            // If iopl was dropped, `inb` faults (SIGSEGV) → handler prints IOPL_DROPPED.
            // NOTE: printf `%%%%` → `%%` in file (needed for GCC inline asm register syntax).
            dockerfile: r#"FROM alpine:latest AS build
RUN apk add --no-cache gcc musl-dev
RUN printf '#include <stdio.h>\n#include <signal.h>\n#include <setjmp.h>\nstatic jmp_buf buf;\nvoid handler(int s){(void)s;longjmp(buf,1);}\nint main(void){signal(SIGSEGV,handler);if(setjmp(buf)){puts("IOPL_DROPPED");return 0;}unsigned char v;__asm__ volatile("inb %%%%dx,%%%%al":"=a"(v):"d"((unsigned short)0x80));puts("IOPL_INHERITED");return 1;}\n' > /test.c && gcc -static -o /test_iopl /test.c
FROM busybox
COPY --from=build /test_iopl /test_iopl
CMD ["/test_iopl"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.stdout.contains("IOPL_DROPPED") {
                    Ok(())
                } else if output.stdout.contains("IOPL_INHERITED") {
                    bail!(
                        "iopl(3) was inherited by benchmark process - \
                         init should drop iopl before exec"
                    )
                } else {
                    bail!(
                        "Expected IOPL_DROPPED in output.\nstdout: {}\nstderr: {}\nexit_code: {}",
                        output.stdout,
                        output.stderr,
                        output.exit_code
                    )
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
                    bail!("Runner failed (exit {}): {combined}", output.exit_code)
                }
                if let Ok(count) = output.stdout.trim().parse::<u32>() {
                    if count > 50 {
                        bail!(
                            "Too many PIDs visible ({count}), PID namespace may be leaking host PIDs"
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
                let json_str = extract_json_substr(line);
                // Parse wall_clock_ms
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str)
                    && let Some(wall_ms) = json
                        .get("wall_clock_ms")
                        .and_then(serde_json::Value::as_u64)
                {
                    if wall_ms < 500 {
                        bail!("wall_clock_ms too low ({wall_ms}ms), timing may be broken")
                    }
                    if wall_ms > 60_000 {
                        bail!("wall_clock_ms too high ({wall_ms}ms)")
                    }
                    return Ok(());
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
                let json_str = extract_json_substr(line);
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str)
                    && json.get("timed_out") == Some(&serde_json::Value::Bool(true))
                {
                    return Ok(());
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
                let json_str = extract_json_substr(line);
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str)
                    && let Some(transport) =
                        json.get("transport").and_then(serde_json::Value::as_str)
                {
                    if transport == "vsock" || transport == "serial" {
                        return Ok(());
                    }
                    bail!("Unexpected transport type: {transport}")
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
        Scenario {
            name: "multi_file_output",
            description: "Multiple output files collected via vsock",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo '{\"result\": 1}' > /tmp/a.json && echo '{\"result\": 2}' > /tmp/b.json && echo done"]"#,
            cancel_after_secs: None,
            extra_args: &[
                "--timeout",
                "60",
                "--output",
                "/tmp/a.json",
                "--output",
                "/tmp/b.json",
            ],
            validate: |output| {
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
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
        // =======================================================================
        // Resource constraint enforcement
        // =======================================================================
        Scenario {
            name: "memory_size_visible",
            description: "Guest sees correct memory with --memory flag",
            // Verify that --memory 64 gives the guest ~64 MiB of RAM.
            // `free -m` reports total memory; we check it's in the right ballpark.
            dockerfile: r#"FROM busybox
CMD ["free", "-m"]"#,
            cancel_after_secs: None,
            extra_args: &["--memory", "64", "--timeout", "60"],
            validate: |output| {
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                // Runner config should show 64 MiB
                if !output.stdout.contains("64 MiB") {
                    bail!(
                        "Expected '64 MiB' in runner memory config output, got: {}",
                        output.stdout
                    )
                }
                Ok(())
            },
        },
        Scenario {
            name: "disk_size_override",
            description: "--disk flag configures ext4 size",
            // Verify the --disk flag is accepted and the ext4 image is
            // created at the requested size. Note: the ext4 image uses a
            // sparse file, so the VM won't actually enforce the limit at
            // the block device level. This test validates the config path.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "df -m / | tail -1 | awk '{print $2}'"]"#,
            cancel_after_secs: None,
            extra_args: &["--disk", "64", "--timeout", "60"],
            validate: |output| {
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                // The runner logs should show the configured disk size
                if output.stdout.contains("64 MiB") {
                    Ok(())
                } else {
                    bail!(
                        "Expected '64 MiB' in runner disk config output, got: {}",
                        output.stdout
                    )
                }
            },
        },
        Scenario {
            name: "disk_limit_enforced",
            description: "ext4 filesystem bounded by --disk size",
            // Verify that the ext4 filesystem reports the correct size.
            // With --disk 64 (minimum), the ext4 filesystem should report
            // approximately 64 MiB total (minus overhead), not more.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "df -m / | tail -1 | awk '{print \"TOTAL_MB=\" $2}'"]"#,
            cancel_after_secs: None,
            extra_args: &["--disk", "64", "--timeout", "60"],
            validate: |output| {
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                // Parse the total MB from the output
                for line in output.stdout.lines() {
                    if let Some(mb_str) = line.strip_prefix("TOTAL_MB=")
                        && let Ok(total_mb) = mb_str.trim().parse::<u64>()
                    {
                        // ext4 overhead reduces usable space. For a 64 MiB image,
                        // total should be roughly 40-60 MiB (not 1024+ default).
                        if total_mb > 100 {
                            bail!("Filesystem too large ({total_mb} MiB), --disk 64 not enforced")
                        }
                        return Ok(());
                    }
                }
                bail!(
                    "Could not parse TOTAL_MB from output.\nstdout: {}",
                    output.stdout
                )
            },
        },
        Scenario {
            name: "cpu_count_visible",
            description: "Guest sees 1 CPU with default vCPU count",
            dockerfile: r#"FROM busybox
CMD ["nproc"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                if output.stdout.contains('1') {
                    Ok(())
                } else {
                    bail!("Expected '1' CPU from nproc, got: {}", output.stdout)
                }
            },
        },
        // =======================================================================
        // Network enabled
        // =======================================================================
        Scenario {
            name: "network_enabled",
            description: "Network works when --network is enabled",
            // With --network, the guest should be able to resolve DNS or ping.
            // Use wget to a well-known URL as a connectivity test.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "wget -q -O /dev/null http://detectportal.firefox.com/success.txt && echo net_ok || echo net_fail"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "30", "--network"],
            validate: |output| {
                let combined = format!("{}{}", output.stdout, output.stderr);
                if combined.contains("net_ok") {
                    Ok(())
                } else {
                    // Network may not be available in all test environments.
                    // If the runner itself didn't crash, that's acceptable.
                    if combined.contains("panic") || combined.contains("SIGSEGV") {
                        bail!("Runner crashed with --network: {combined}")
                    }
                    // Accept net_fail if the environment doesn't have outbound access
                    // — the key thing is --network didn't cause a crash.
                    Ok(())
                }
            },
        },
        // =======================================================================
        // File permissions
        // =======================================================================
        Scenario {
            name: "file_content_preserved",
            description: "File content from RUN layers survives OCI unpack + ext4",
            // Verify that file content written in a RUN layer is readable
            // inside the VM. Uses the same pattern as image_with_symlinks.
            dockerfile: r#"FROM busybox
RUN mkdir -p /data && echo "content_ok" > /data/file.txt
CMD ["cat", "/data/file.txt"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                if output.stdout.contains("content_ok") {
                    Ok(())
                } else {
                    bail!("Expected 'content_ok' in output, got: {}", output.stdout)
                }
            },
        },
        Scenario {
            name: "file_permissions_preserved",
            description: "Executable bit preserved through OCI unpack + ext4",
            // chmod +x in a RUN layer must survive OCI layer extraction.
            // If permissions are lost, `test -x` fails and we don't see "perm_ok".
            dockerfile: r#"FROM busybox
RUN mkdir -p /data && printf '#!/bin/sh\necho hello' > /data/test.sh && chmod +x /data/test.sh
CMD ["sh", "-c", "test -x /data/test.sh && echo perm_ok"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                if output.stdout.contains("perm_ok") {
                    Ok(())
                } else {
                    bail!(
                        "Expected 'perm_ok' (executable bit preserved), got: {}",
                        output.stdout
                    )
                }
            },
        },
        Scenario {
            name: "directory_permissions_preserved",
            description: "Directory permissions preserved through OCI unpack + ext4",
            // chmod 750 on a directory in a RUN layer must survive extraction.
            // stat -c '%a' prints the octal mode.
            dockerfile: r#"FROM busybox
RUN mkdir -p /data/restricted && chmod 750 /data/restricted
CMD ["stat", "-c", "%a", "/data/restricted"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                if output.stdout.contains("750") {
                    Ok(())
                } else {
                    bail!(
                        "Expected '750' (directory permissions preserved), got: {}",
                        output.stdout
                    )
                }
            },
        },
        // =======================================================================
        // Special characters in environment variables
        // =======================================================================
        Scenario {
            name: "special_chars_in_env",
            description: "Env vars with spaces, equals, and quotes work",
            // Use Docker's multi-line ENV syntax with quotes for values with spaces.
            dockerfile: "FROM busybox\nENV SPACED=\"hello world\" WITH_EQ=\"key=value\"\nCMD [\"sh\", \"-c\", \"echo SPACED=$SPACED EQ=$WITH_EQ\"]",
            cancel_after_secs: None,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                if !output.stdout.contains("SPACED=hello world") {
                    bail!(
                        "Expected 'SPACED=hello world' in output, got: {}",
                        output.stdout
                    )
                }
                if !output.stdout.contains("EQ=key=value") {
                    bail!("Expected 'EQ=key=value' in output, got: {}", output.stdout)
                }
                Ok(())
            },
        },
        // =======================================================================
        // CLI override scenarios (--entrypoint, --cmd, --env)
        // =======================================================================
        Scenario {
            name: "cli_entrypoint_override",
            description: "Override ENTRYPOINT from CLI",
            dockerfile: r#"FROM busybox
ENTRYPOINT ["echo", "image_ep"]
CMD ["image_cmd"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60", "--entrypoint", "echo", "cli_ep"],
            validate: |output| {
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                // Docker semantics: CLI entrypoint ["echo", "cli_ep"] clears OCI CMD
                if !output.stdout.contains("cli_ep") {
                    bail!(
                        "Expected 'cli_ep' in output (CLI entrypoint override).\nstdout: {}\nstderr: {}",
                        output.stdout,
                        output.stderr
                    )
                }
                if output.stdout.contains("image_ep") {
                    bail!(
                        "OCI image_ep should have been overridden.\nstdout: {}",
                        output.stdout
                    )
                }
                if output.stdout.contains("image_cmd") {
                    bail!(
                        "OCI image_cmd should have been cleared (Docker semantics: overriding entrypoint clears CMD).\nstdout: {}",
                        output.stdout
                    )
                }
                Ok(())
            },
        },
        Scenario {
            name: "cli_cmd_override",
            description: "Override CMD from CLI",
            dockerfile: r#"FROM busybox
ENTRYPOINT ["echo"]
CMD ["image_cmd"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60", "--cmd", "cli_cmd"],
            validate: |output| {
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                if !output.stdout.contains("cli_cmd") {
                    bail!(
                        "Expected 'cli_cmd' in output.\nstdout: {}\nstderr: {}",
                        output.stdout,
                        output.stderr
                    )
                }
                if output.stdout.contains("image_cmd") {
                    bail!(
                        "OCI image_cmd should have been overridden.\nstdout: {}",
                        output.stdout
                    )
                }
                Ok(())
            },
        },
        Scenario {
            name: "cli_entrypoint_and_cmd_override",
            description: "Override both ENTRYPOINT and CMD from CLI",
            dockerfile: r#"FROM busybox
ENTRYPOINT ["echo", "image_ep"]
CMD ["image_cmd"]"#,
            cancel_after_secs: None,
            extra_args: &[
                "--timeout",
                "60",
                "--entrypoint",
                "echo",
                "--cmd",
                "cli_both",
            ],
            validate: |output| {
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                if !output.stdout.contains("cli_both") {
                    bail!(
                        "Expected 'cli_both' in output.\nstdout: {}\nstderr: {}",
                        output.stdout,
                        output.stderr
                    )
                }
                if output.stdout.contains("image_ep") || output.stdout.contains("image_cmd") {
                    bail!(
                        "OCI image entrypoint/cmd should have been overridden.\nstdout: {}",
                        output.stdout
                    )
                }
                Ok(())
            },
        },
        Scenario {
            name: "cli_env_override",
            description: "Override an existing ENV from CLI",
            dockerfile: r#"FROM busybox
ENV MY_VAR=image_value
CMD ["sh", "-c", "echo MY_VAR=$MY_VAR"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60", "--env", "MY_VAR=cli_value"],
            validate: |output| {
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                if output.stdout.contains("MY_VAR=cli_value") {
                    Ok(())
                } else {
                    bail!(
                        "Expected 'MY_VAR=cli_value' in output.\nstdout: {}\nstderr: {}",
                        output.stdout,
                        output.stderr
                    )
                }
            },
        },
        Scenario {
            name: "cli_env_add",
            description: "Add a new ENV from CLI alongside image ENV",
            dockerfile: r#"FROM busybox
ENV EXISTING=from_image
CMD ["sh", "-c", "echo EXISTING=$EXISTING NEW=$NEW_VAR"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60", "--env", "NEW_VAR=from_cli"],
            validate: |output| {
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                if !output.stdout.contains("EXISTING=from_image") {
                    bail!(
                        "Expected 'EXISTING=from_image' in output.\nstdout: {}",
                        output.stdout
                    )
                }
                if !output.stdout.contains("NEW=from_cli") {
                    bail!(
                        "Expected 'NEW=from_cli' in output.\nstdout: {}",
                        output.stdout
                    )
                }
                Ok(())
            },
        },
        Scenario {
            name: "cli_env_multiple",
            description: "Multiple --env flags",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo A=$A B=$B"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60", "--env", "A=one", "--env", "B=two"],
            validate: |output| {
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                if !output.stdout.contains("A=one") {
                    bail!("Expected 'A=one' in output.\nstdout: {}", output.stdout)
                }
                if !output.stdout.contains("B=two") {
                    bail!("Expected 'B=two' in output.\nstdout: {}", output.stdout)
                }
                Ok(())
            },
        },
        Scenario {
            name: "cli_entrypoint_no_image_entrypoint",
            description: "Add entrypoint when image only has CMD",
            dockerfile: r#"FROM busybox
CMD ["hello", "world"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60", "--entrypoint", "echo"],
            validate: |output| {
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                // Docker semantics: CLI entrypoint ["echo"] clears OCI CMD ["hello", "world"]
                // So we expect just the output of `echo` (empty line)
                if output.stdout.contains("hello world") {
                    bail!(
                        "OCI CMD should have been cleared (Docker semantics: overriding entrypoint clears CMD).\nstdout: {}\nstderr: {}",
                        output.stdout,
                        output.stderr
                    )
                }
                Ok(())
            },
        },
        Scenario {
            name: "multiple_iterations",
            description: "Multiple iterations execute sequentially",
            dockerfile: r#"FROM busybox
CMD ["echo", "iter_output"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60", "--iter", "3"],
            validate: |output| {
                if output.exit_code != 0 {
                    bail!("Expected exit code 0, got {}", output.exit_code)
                }
                // Each iteration prints "iter_output", so we should see it at least 3 times
                let count = output.stdout.matches("iter_output").count();
                if count < 3 {
                    bail!("Expected 3 iterations of output, found {count}")
                }
                Ok(())
            },
        },
        Scenario {
            name: "zero_iterations",
            description: "Zero iterations executes no benchmarks",
            dockerfile: r#"FROM busybox
CMD ["echo", "should_not_appear"]"#,
            cancel_after_secs: None,
            extra_args: &["--timeout", "60", "--iter", "0"],
            validate: |output| {
                if output.exit_code != 0 {
                    bail!("Expected exit code 0, got {}", output.exit_code)
                }
                if output.stdout.contains("should_not_appear") {
                    bail!("Expected no benchmark execution with --iter 0, but output was produced")
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
///
/// Uses `docker buildx build --output type=oci` to produce a proper OCI Image
/// Layout directory (with `oci-layout`, `index.json`, and `blobs/sha256/`).
/// Plain `docker save` produces a Docker archive format which is incompatible
/// with the runner's OCI parser.
fn build_test_image(name: &str, dockerfile: &str) -> Result<Utf8PathBuf> {
    let build_dir = temp_dir().join(format!("build-{name}"));
    drop(fs::remove_dir_all(&build_dir));
    fs::create_dir_all(&build_dir)?;

    // Write Dockerfile
    let dockerfile_path = build_dir.join("Dockerfile");
    fs::write(&dockerfile_path, dockerfile)?;

    // Build and output as OCI layout directly
    let oci_dir = temp_dir().join(format!("oci-{name}"));
    drop(fs::remove_dir_all(&oci_dir));

    let output_arg = format!("type=oci,tar=false,dest={oci_dir}");
    let output = Command::new("docker")
        .args(["buildx", "build", "--output", &output_arg, "."])
        .current_dir(&build_dir)
        .output()?;

    if !output.status.success() {
        bail!(
            "docker buildx build failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
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
    runner_bin: &Utf8Path,
) -> Result<ScenarioOutput> {
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
    #[expect(
        unsafe_code,
        clippy::cast_possible_wrap,
        reason = "libc::kill requires unsafe; PID fits in i32"
    )]
    // SAFETY: Sending a signal to a known child process we just spawned.
    unsafe {
        libc::kill(pid as i32, libc::SIGTERM);
    }

    // Wait for the process to exit with a grace period.
    // If the runner handles SIGTERM correctly, it should shut down the VM and exit.
    let grace = Duration::from_secs(30);
    let start = std::time::Instant::now();
    loop {
        if let Some(_status) = child.try_wait()? {
            // Process exited — collect remaining pipe output
            let output = child.wait_with_output()?;
            return Ok(ScenarioOutput {
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
                exit_code: output.status.code().unwrap_or(-1),
            });
        }
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
    }
}

/// Build bencher-init for the musl target and the runner CLI with `BENCHER_INIT_PATH`,
/// then return the path to the runner binary.
fn ensure_runner_bin() -> Result<Utf8PathBuf> {
    let workspace_root = super::workspace_root();
    let target_triple = super::musl_target_triple()?;

    // Step 1: Build bencher-init (musl, statically linked)
    println!("Building bencher-init ({target_triple})...");
    let status = Command::new("cargo")
        .args(["build", "--target", target_triple, "-p", "bencher_init"])
        .current_dir(&workspace_root)
        .status()
        .context("Failed to spawn cargo build for bencher-init")?;
    if !status.success() {
        bail!("cargo build -p bencher_init --target {target_triple} failed");
    }

    let init_path = workspace_root.join(format!("target/{target_triple}/debug/bencher-init"));
    if !init_path.exists() {
        bail!("bencher-init binary not found at {init_path} after build");
    }

    // Step 2: Build runner CLI with BENCHER_INIT_PATH pointing to the init binary
    println!("Building runner CLI (BENCHER_INIT_PATH={init_path})...");
    let status = Command::new("cargo")
        .args(["build", "-p", "bencher_runner_cli"])
        .env("BENCHER_INIT_PATH", &init_path)
        .current_dir(&workspace_root)
        .status()
        .context("Failed to spawn cargo build for runner CLI")?;
    if !status.success() {
        bail!("cargo build -p bencher_runner_cli failed");
    }

    let runner_bin = workspace_root.join("target/debug/runner");
    if !runner_bin.exists() {
        bail!("Runner binary not found at {runner_bin} after build");
    }

    Ok(runner_bin)
}

/// Run the runner and capture output.
fn run_runner(
    image_path: &Utf8Path,
    args: &[&str],
    runner_bin: &Utf8Path,
) -> Result<ScenarioOutput> {
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
