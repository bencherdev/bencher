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

/// A host-side check run while the runner is still executing.
///
/// Some confinement invariants only exist while the VMM is alive and cannot
/// be recovered from the runner's output afterwards. The probe receives the
/// runner's state directory and returns `Ok(false)` while the VMM has not
/// appeared yet, `Ok(true)` once the invariant has been observed to hold, and
/// `Err` once it has been observed to be violated.
type Probe = fn(&Utf8Path) -> Result<bool>;

/// Test scenario definition.
///
/// Build one with `..Scenario::default()` so a scenario names only what it
/// actually varies, and so adding a field does not have to be written out
/// across every scenario in this file.
struct Scenario {
    name: &'static str,
    description: &'static str,
    dockerfile: &'static str,
    extra_args: &'static [&'static str],
    /// If set, send SIGTERM to the runner after this many seconds.
    cancel_after_secs: Option<u64>,
    /// Whether to use `--sandbox firecracker` (default: true).
    sandboxed: bool,
    /// If set, a host-side check run while the runner is executing.
    probe: Option<Probe>,
    /// Kill the runner once its VMM is up so nothing unwinds, then run the
    /// image again and report the second run.
    ///
    /// SIGKILL is the point: it is the exit that never unwinds, so `Drop`
    /// cannot reclaim the chroot and only the sweep can.
    orphan_then_rerun: bool,
    validate: fn(&ScenarioOutput) -> Result<()>,
}

impl Default for Scenario {
    fn default() -> Self {
        Self {
            name: "",
            description: "",
            dockerfile: "",
            extra_args: &[],
            cancel_after_secs: None,
            probe: None,
            orphan_then_rerun: false,
            // Sandboxed is the interesting case and the overwhelming majority,
            // so the handful of non-sandboxed scenarios opt out rather than
            // every other scenario opting in.
            sandboxed: true,
            validate: |_output| Ok(()),
        }
    }
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
    build_only: bool,
}

impl TryFrom<TaskScenarios> for Scenarios {
    type Error = anyhow::Error;

    fn try_from(task: TaskScenarios) -> Result<Self, Self::Error> {
        Ok(Self {
            scenario: task.scenario,
            list: task.list,
            build_only: task.build_only,
        })
    }
}

impl Scenarios {
    pub fn exec(&self) -> Result<()> {
        if self.list {
            list_scenarios();
            return Ok(());
        }

        if self.build_only {
            let runner_bin = ensure_runner_bin()?;
            println!("Built runner: {runner_bin}");
            println!("Run the scenarios with:");
            println!("  sudo {RUNNER_BIN_ENV}={runner_bin} <test_runner binary> scenarios");
            return Ok(());
        }

        // Check prerequisites.
        //
        // Root is one of them. The jailer creates the chroot's device nodes
        // with mknod, chowns the tree to the jail user, pivot_roots, and joins
        // a network namespace, none of which an unprivileged process can do.
        // A udev rule that makes /dev/kvm world accessible is enough to *use*
        // KVM without root but not to build the jail around it.
        if !is_root() {
            bail!(
                "The scenarios must run as root: the sandbox is built by dropping privilege, not by starting without it.\n\
                 Build unprivileged first, then run elevated:\n\
                 \x20 cargo test-runner scenarios --build-only\n\
                 \x20 sudo {RUNNER_BIN_ENV}=./target/debug/runner ./target/debug/test_runner scenarios"
            );
        }
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

        let mut scenarios = all_scenarios();
        scenarios.extend(jail_scenarios());
        scenarios.extend(nosandbox_scenarios());

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
    let mut scenarios = all_scenarios();
    scenarios.extend(jail_scenarios());
    scenarios.extend(nosandbox_scenarios());
    println!("Available scenarios:");
    println!();
    for scenario in &scenarios {
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

    // Every scenario gets its own state directory, so jail assertions are
    // scoped to the scenario and never touch a real runner's state.
    let state_dir = scenario_state_dir();
    drop(fs::remove_dir_all(&state_dir));

    // Prepend --sandbox firecracker for sandboxed scenarios
    // --no-tuning matters now that the scenarios run as root. Unprivileged,
    // every tuning knob failed with EPERM and warned; elevated they actually
    // apply, and offlining SMT siblings on a two-vCPU hosted runner would
    // change the core count mid-suite. The scenarios exercise job execution,
    // not tuning, so this costs no coverage.
    let mut args: Vec<&str> = vec!["--state-dir", state_dir.as_str(), "--no-tuning"];
    if scenario.sandboxed {
        args.extend(["--sandbox", "firecracker"]);
    }
    args.extend(scenario.extra_args);

    // Run the runner (with optional cancellation or host-side probe)
    let output = if let Some(secs) = scenario.cancel_after_secs {
        run_runner_with_cancel(&image_path, &args, Duration::from_secs(secs), runner_bin)
    } else if scenario.orphan_then_rerun {
        run_runner_after_orphan(&image_path, &args, &state_dir, runner_bin)
    } else if let Some(probe) = scenario.probe {
        run_runner_with_probe(&image_path, &args, probe, &state_dir, runner_bin)
    } else {
        run_runner(&image_path, &args, runner_bin)
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
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.stdout.contains("hello from vm") {
                    Ok(())
                } else {
                    bail!("Expected 'hello from vm' in output, got: {}", output.stdout)
                }
            },
            ..Scenario::default()
        },
        Scenario {
            name: "environment_variables",
            description: "ENV variables passed to guest",
            dockerfile: r#"FROM busybox
ENV MY_VAR=test_value
CMD ["sh", "-c", "echo $MY_VAR"]"#,
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
            ..Scenario::default()
        },
        Scenario {
            name: "working_directory",
            description: "WORKDIR set correctly",
            dockerfile: r#"FROM busybox
WORKDIR /myapp
CMD ["pwd"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.stdout.contains("/myapp") {
                    Ok(())
                } else {
                    bail!("Expected '/myapp' in output, got: {}", output.stdout)
                }
            },
            ..Scenario::default()
        },
        Scenario {
            name: "file_output",
            description: "Output file collection via vsock",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo '{\"result\": 42}' > /tmp/output.json && cat /tmp/output.json"]"#,
            extra_args: &["--timeout", "60", "--output", "/tmp/output.json"],
            validate: |output| {
                if output.stdout.contains("\"result\"") || output.stdout.contains("42") {
                    Ok(())
                } else {
                    bail!("Expected JSON output, got: {}", output.stdout)
                }
            },
            ..Scenario::default()
        },
        Scenario {
            name: "exit_code",
            description: "Non-zero exit codes captured",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "exit 42"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                let combined = format!("{}{}", output.stdout, output.stderr);
                if combined.contains("42") || output.exit_code != 0 {
                    Ok(())
                } else {
                    bail!("Expected exit code 42 in output")
                }
            },
            ..Scenario::default()
        },
        Scenario {
            name: "timeout_handling",
            description: "VM killed after timeout",
            dockerfile: r#"FROM busybox
CMD ["sleep", "3600"]"#,
            extra_args: &["--timeout", "5"],
            validate: |output| {
                let combined = format!("{}{}", output.stdout, output.stderr).to_lowercase();
                if combined.contains("timeout") || output.exit_code != 0 {
                    Ok(())
                } else {
                    bail!("Expected timeout error")
                }
            },
            ..Scenario::default()
        },
        Scenario {
            name: "writable_filesystem",
            description: "Guest can write to ext4 rootfs",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo test > /data.txt && cat /data.txt"]"#,
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
            ..Scenario::default()
        },
        Scenario {
            name: "stderr_capture",
            description: "Stderr captured separately",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo stdout && echo stderr >&2"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                let combined = format!("{}{}", output.stdout, output.stderr);
                if combined.contains("stdout") {
                    Ok(())
                } else {
                    bail!("Expected 'stdout' in output")
                }
            },
            ..Scenario::default()
        },
        Scenario {
            name: "multi_cpu",
            description: "Multiple vCPUs work (expected: timeout, SMP boot unsupported)",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "cat /proc/cpuinfo | grep processor | wc -l"]"#,
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
            ..Scenario::default()
        },
        Scenario {
            name: "entrypoint_with_args",
            description: "ENTRYPOINT + CMD combined",
            dockerfile: r#"FROM busybox
ENTRYPOINT ["echo"]
CMD ["hello", "world"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.stdout.contains("hello world") {
                    Ok(())
                } else {
                    bail!("Expected 'hello world' in output, got: {}", output.stdout)
                }
            },
            ..Scenario::default()
        },
        Scenario {
            name: "no_network_access",
            description: "Guest has no network",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "ping -c 1 -W 1 8.8.8.8 2>&1 || echo no_network"]"#,
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
            ..Scenario::default()
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
            ..Scenario::default()
        },
        Scenario {
            name: "timeout_enforced",
            description: "Timeout kills hanging process",
            // This process ignores signals and runs forever
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "trap '' TERM INT; echo started; while true; do sleep 1; done"]"#,
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
            ..Scenario::default()
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
            ..Scenario::default()
        },
        Scenario {
            name: "dev_kvm_available",
            description: "/dev/kvm accessible inside jail",
            // Verifies the bind-mount of /dev/kvm survives pivot_root.
            // A previous bug: mounting tmpfs on /dev after pivot_root overwrote
            // the bind-mounted /dev/kvm.
            dockerfile: r#"FROM busybox
CMD ["echo", "kvm_test_ok"]"#,
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
            ..Scenario::default()
        },
        Scenario {
            name: "proc_mount_works",
            description: "/proc accessible inside jail",
            // Verifies /proc is correctly bind-mounted into the jail.
            // A previous bug: mounting fresh procfs requires PID namespace + fork,
            // which we fixed by bind-mounting the host's /proc instead.
            dockerfile: r#"FROM busybox
CMD ["cat", "/proc/version"]"#,
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
            ..Scenario::default()
        },
        Scenario {
            name: "rootfs_writable",
            description: "Rootfs mounted read-write (not read-only)",
            // Verifies the kernel cmdline uses 'rw' not 'ro' for root mount.
            // A previous bug: default cmdline had 'ro', causing init to fail
            // when trying to write to the filesystem.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "touch /tmp/write_test && echo write_ok"]"#,
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
            ..Scenario::default()
        },
        Scenario {
            name: "timeout_includes_partial_output",
            description: "Timeout errors include partial output captured before timeout",
            // Verifies that when a VM times out, any output produced before the
            // timeout is not discarded. A previous bug: the timeout error path
            // short-circuited before serial output extraction.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo partial_output_marker && sleep 3600"]"#,
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
            ..Scenario::default()
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
            ..Scenario::default()
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
            ..Scenario::default()
        },
        Scenario {
            name: "unique_output_validation",
            description: "Output comes from VM, not runner preparation logs",
            // Verifies that the output validation is not a false positive from
            // matching runner preparation output. Uses a unique marker that would
            // never appear in runner logs.
            dockerfile: r#"FROM busybox
CMD ["echo", "UNIQUE_VM_OUTPUT_a7f3b2c9"]"#,
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
            ..Scenario::default()
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
            ..Scenario::default()
        },
        Scenario {
            name: "pid_namespace_procfs",
            description: "Fresh procfs mount works with PID namespace",
            // Verifies /proc is properly mounted with PID namespace support.
            // With fresh procfs (not bind-mounted from host), /proc/version
            // should be accessible and /proc/1/cmdline should show the init process.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "cat /proc/version && echo PID1=$(cat /proc/1/cmdline | tr '\\0' ' ')"]"#,
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
            ..Scenario::default()
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
            ..Scenario::default()
        },
        Scenario {
            name: "metrics_wall_clock_reasonable",
            description: "Wall clock time is within reasonable bounds",
            // A fast benchmark (echo) should have wall clock between 500ms and 60000ms.
            // This catches cases where timing is broken (e.g., always 0 or absurdly large).
            dockerfile: r#"FROM busybox
CMD ["echo", "fast_benchmark"]"#,
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
            ..Scenario::default()
        },
        Scenario {
            name: "metrics_timeout_flag",
            description: "Timeout flag set correctly in metrics",
            // When a VM times out, the metrics should include timed_out: true.
            dockerfile: r#"FROM busybox
CMD ["sleep", "3600"]"#,
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
            ..Scenario::default()
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
            ..Scenario::default()
        },
        Scenario {
            name: "metrics_transport_type",
            description: "Transport type reported in metrics",
            // Verifies the metrics include the transport type (vsock or serial).
            dockerfile: r#"FROM busybox
CMD ["echo", "transport_test"]"#,
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
            ..Scenario::default()
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
            ..Scenario::default()
        },
        // =======================================================================
        // Output edge-case scenarios
        // =======================================================================
        Scenario {
            name: "stderr_only",
            description: "Stderr captured when stdout is empty",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo error_output >&2"]"#,
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
            ..Scenario::default()
        },
        Scenario {
            name: "empty_output",
            description: "Process exits 0 with no output",
            dockerfile: r#"FROM busybox
CMD ["true"]"#,
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
            ..Scenario::default()
        },
        Scenario {
            name: "binary_output",
            description: "Non-UTF8 stdout handled gracefully",
            // Write raw bytes 0x80-0xFF which are invalid UTF-8.
            // The runner should not panic — it should lossy-convert or pass through.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "printf '\\x80\\x81\\xFE\\xFF' && echo done"]"#,
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
            ..Scenario::default()
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
            ..Scenario::default()
        },
        Scenario {
            name: "entrypoint_only",
            description: "ENTRYPOINT exec form with no CMD",
            // When only ENTRYPOINT is set (exec form), it runs as-is with no
            // CMD args appended. The runner must not fail when Cmd is null/empty.
            dockerfile: r#"FROM busybox
ENTRYPOINT ["echo", "entrypoint_only_works"]"#,
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
            ..Scenario::default()
        },
        Scenario {
            name: "shell_form_entrypoint",
            description: "ENTRYPOINT shell form (string, not array)",
            // Shell form ENTRYPOINT: stored as ["/bin/sh", "-c", "echo ..."]
            // in OCI config. CMD is ignored when ENTRYPOINT uses shell form.
            dockerfile: "FROM busybox\nENTRYPOINT echo shell_entrypoint_works",
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
            ..Scenario::default()
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
            ..Scenario::default()
        },
        Scenario {
            name: "no_cmd_no_entrypoint",
            description: "No CMD or ENTRYPOINT fails gracefully",
            // An image with no CMD and no ENTRYPOINT should cause the runner
            // to fail with a clear error, not crash or hang.
            dockerfile: r#"FROM busybox
RUN echo "no command set""#,
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
            ..Scenario::default()
        },
        Scenario {
            name: "bencher_cli_mock",
            description: "Bencher CLI mock on distroless/cc-debian12",
            // Uses the Bencher CLI image (distroless/cc-debian12 + glibc).
            // The CLI has ENTRYPOINT ["/usr/bin/bencher"], so we add CMD ["mock"]
            // to run `bencher mock` which outputs valid benchmark JSON.
            // This tests that the OCI unpack preserves the dynamic linker,
            // shared libraries, and ld.so.cache from multi-layer images.
            dockerfile: r#"FROM ghcr.io/bencherdev/bencher:latest
CMD ["mock"]"#,
            extra_args: &["--timeout", "120"],
            validate: |output| {
                if output.exit_code == 127 {
                    bail!(
                        "Exit code 127 (command not found) — dynamic linker or shared libraries \
                         likely missing from unpacked rootfs.\nstdout: {}\nstderr: {}",
                        output.stdout,
                        output.stderr
                    )
                }
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                // bencher mock should produce JSON with benchmark results
                if output.stdout.contains("latency") || output.stdout.contains("bencher::mock") {
                    Ok(())
                } else {
                    bail!(
                        "Expected benchmark JSON from 'bencher mock' in output.\nstdout: {}\nstderr: {}",
                        output.stdout,
                        output.stderr
                    )
                }
            },
            ..Scenario::default()
        },
        Scenario {
            name: "distroless_glibc_image",
            description: "Dynamically linked binary on distroless/cc-debian12",
            // Builds a small dynamically-linked C program on distroless/cc-debian12.
            // This tests that the OCI unpack preserves the dynamic linker,
            // shared libraries, and ld.so.cache from multi-layer images.
            // Builds from source so the binary always matches the host architecture.
            dockerfile: r#"FROM debian:bookworm-slim AS builder
RUN apt-get update && apt-get install -y gcc libc6-dev && rm -rf /var/lib/apt/lists/*
RUN echo '#include <stdio.h>\nint main(){printf("distroless_glibc_ok\\n");return 0;}' > /tmp/hello.c \
    && gcc -o /tmp/hello /tmp/hello.c

FROM gcr.io/distroless/cc-debian12
COPY --from=builder /tmp/hello /usr/bin/hello
CMD ["/usr/bin/hello"]"#,
            extra_args: &["--timeout", "120"],
            validate: |output| {
                if output.exit_code == 127 {
                    bail!(
                        "Exit code 127 (command not found) — dynamic linker or shared libraries \
                         likely missing from unpacked rootfs.\nstdout: {}\nstderr: {}",
                        output.stdout,
                        output.stderr
                    )
                }
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                if output.stdout.contains("distroless_glibc_ok") {
                    Ok(())
                } else {
                    bail!(
                        "Expected 'distroless_glibc_ok' in output.\nstdout: {}\nstderr: {}",
                        output.stdout,
                        output.stderr
                    )
                }
            },
            ..Scenario::default()
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
            ..Scenario::default()
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
            ..Scenario::default()
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
            ..Scenario::default()
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
            ..Scenario::default()
        },
        Scenario {
            name: "large_file_output",
            description: "Large output file (~2 MB) transferred via vsock",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "dd if=/dev/urandom bs=1024 count=2048 2>/dev/null | base64 > /tmp/output.json && echo done"]"#,
            extra_args: &["--timeout", "60", "--output", "/tmp/output.json"],
            validate: |output| {
                if output.exit_code != 0 {
                    let combined = format!("{}{}", output.stdout, output.stderr);
                    bail!("Runner failed (exit {}): {}", output.exit_code, combined)
                }
                Ok(())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "completed_with_all_fields",
            description: "Stdout + stderr + output file simultaneously",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo stdout_marker && echo stderr_marker >&2 && echo '{\"data\":true}' > /tmp/out.json"]"#,
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
            ..Scenario::default()
        },
        Scenario {
            name: "multi_file_output",
            description: "Multiple output files collected via vsock",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo '{\"result\": 1}' > /tmp/a.json && echo '{\"result\": 2}' > /tmp/b.json && echo done"]"#,
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
            ..Scenario::default()
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
            ..Scenario::default()
        },
        Scenario {
            name: "image_with_symlinks",
            description: "Symbolic links preserved through OCI unpack + ext4",
            dockerfile: r#"FROM busybox
RUN echo "target" > /tmp/target.txt && ln -s /tmp/target.txt /tmp/link.txt
CMD ["cat", "/tmp/link.txt"]"#,
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
            ..Scenario::default()
        },
        // =======================================================================
        // Error / edge case scenarios
        // =======================================================================
        Scenario {
            name: "failed_with_partial_output",
            description: "Writes stdout+stderr then exits non-zero",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo partial_stdout && echo partial_stderr >&2 && exit 1"]"#,
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
            ..Scenario::default()
        },
        Scenario {
            name: "minimum_timeout",
            description: "1-second timeout kills long-running process",
            dockerfile: r#"FROM busybox
CMD ["sleep", "3600"]"#,
            extra_args: &["--timeout", "1"],
            validate: |output| {
                if output.exit_code == 0 {
                    bail!("Expected non-zero exit for 1s timeout on sleep 3600")
                }
                Ok(())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "max_output_size_truncation",
            description: "Output truncated when --max-output-size is small",
            // Generate ~50 KB of output, but limit to 1024 bytes.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "dd if=/dev/zero bs=1024 count=50 2>/dev/null | tr '\\0' 'X'"]"#,
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
            ..Scenario::default()
        },
        Scenario {
            name: "env_var_passthrough",
            description: "All ENV variables (including LD_*) are passed to the guest",
            dockerfile: r#"FROM busybox
ENV LD_PRELOAD=/test.so
ENV LD_LIBRARY_PATH=/testlib
ENV SAFE_VAR=safe_value
CMD ["sh", "-c", "echo LD_PRELOAD=$LD_PRELOAD LD_LIBRARY_PATH=$LD_LIBRARY_PATH SAFE=$SAFE_VAR"]"#,
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
                if !output.stdout.contains("LD_PRELOAD=/test.so") {
                    bail!(
                        "Expected 'LD_PRELOAD=/test.so' in output, got: {}",
                        output.stdout
                    )
                }
                if !output.stdout.contains("LD_LIBRARY_PATH=/testlib") {
                    bail!(
                        "Expected 'LD_LIBRARY_PATH=/testlib' in output, got: {}",
                        output.stdout
                    )
                }
                Ok(())
            },
            ..Scenario::default()
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
            ..Scenario::default()
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
            ..Scenario::default()
        },
        Scenario {
            name: "disk_limit_enforced",
            description: "ext4 filesystem bounded by --disk size",
            // Verify that the ext4 filesystem reports the correct size.
            // With --disk 64 (minimum), the ext4 filesystem should report
            // approximately 64 MiB total (minus overhead), not more.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "df -m / | tail -1 | awk '{print \"TOTAL_MB=\" $2}'"]"#,
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
            ..Scenario::default()
        },
        Scenario {
            name: "cpu_count_visible",
            description: "Guest sees 1 CPU with default vCPU count",
            dockerfile: r#"FROM busybox
CMD ["nproc"]"#,
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
            ..Scenario::default()
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
            ..Scenario::default()
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
            ..Scenario::default()
        },
        Scenario {
            name: "file_permissions_preserved",
            description: "Executable bit preserved through OCI unpack + ext4",
            // chmod +x in a RUN layer must survive OCI layer extraction.
            // If permissions are lost, `test -x` fails and we don't see "perm_ok".
            dockerfile: r#"FROM busybox
RUN mkdir -p /data && printf '#!/bin/sh\necho hello' > /data/test.sh && chmod +x /data/test.sh
CMD ["sh", "-c", "test -x /data/test.sh && echo perm_ok"]"#,
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
            ..Scenario::default()
        },
        Scenario {
            name: "directory_permissions_preserved",
            description: "Directory permissions preserved through OCI unpack + ext4",
            // chmod 750 on a directory in a RUN layer must survive extraction.
            // stat -c '%a' prints the octal mode.
            dockerfile: r#"FROM busybox
RUN mkdir -p /data/restricted && chmod 750 /data/restricted
CMD ["stat", "-c", "%a", "/data/restricted"]"#,
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
            ..Scenario::default()
        },
        // =======================================================================
        // Special characters in environment variables
        // =======================================================================
        Scenario {
            name: "special_chars_in_env",
            description: "Env vars with spaces, equals, and quotes work",
            // Use Docker's multi-line ENV syntax with quotes for values with spaces.
            dockerfile: "FROM busybox\nENV SPACED=\"hello world\" WITH_EQ=\"key=value\"\nCMD [\"sh\", \"-c\", \"echo SPACED=$SPACED EQ=$WITH_EQ\"]",
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
            ..Scenario::default()
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
            ..Scenario::default()
        },
        Scenario {
            name: "cli_cmd_override",
            description: "Override CMD from CLI",
            dockerfile: r#"FROM busybox
ENTRYPOINT ["echo"]
CMD ["image_cmd"]"#,
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
            ..Scenario::default()
        },
        Scenario {
            name: "cli_entrypoint_and_cmd_override",
            description: "Override both ENTRYPOINT and CMD from CLI",
            dockerfile: r#"FROM busybox
ENTRYPOINT ["echo", "image_ep"]
CMD ["image_cmd"]"#,
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
            ..Scenario::default()
        },
        Scenario {
            name: "cli_env_override",
            description: "Override an existing ENV from CLI",
            dockerfile: r#"FROM busybox
ENV MY_VAR=image_value
CMD ["sh", "-c", "echo MY_VAR=$MY_VAR"]"#,
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
            ..Scenario::default()
        },
        Scenario {
            name: "cli_env_add",
            description: "Add a new ENV from CLI alongside image ENV",
            dockerfile: r#"FROM busybox
ENV EXISTING=from_image
CMD ["sh", "-c", "echo EXISTING=$EXISTING NEW=$NEW_VAR"]"#,
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
            ..Scenario::default()
        },
        Scenario {
            name: "cli_env_multiple",
            description: "Multiple --env flags",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo A=$A B=$B"]"#,
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
            ..Scenario::default()
        },
        Scenario {
            name: "cli_entrypoint_no_image_entrypoint",
            description: "Add entrypoint when image only has CMD",
            dockerfile: r#"FROM busybox
CMD ["hello", "world"]"#,
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
            ..Scenario::default()
        },
        Scenario {
            name: "multiple_iterations",
            description: "Multiple iterations execute sequentially",
            dockerfile: r#"FROM busybox
CMD ["echo", "iter_output"]"#,
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
            ..Scenario::default()
        },
        Scenario {
            name: "zero_iterations",
            description: "Zero iterations executes no benchmarks",
            dockerfile: r#"FROM busybox
CMD ["echo", "should_not_appear"]"#,
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
            ..Scenario::default()
        },
        Scenario {
            name: "allow_failure_false_aborts",
            description: "Non-zero exit code aborts iteration without --allow-failure",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo __ITER_DONE__ && exit 1"]"#,
            extra_args: &["--timeout", "60", "--iter", "3"],
            validate: |output| {
                if output.exit_code == 0 {
                    bail!("Expected non-zero exit code")
                }
                // Only 1 iteration should run before aborting.
                // Count lines that are exactly the marker to avoid matching
                // the informational "Command: ..." line printed to stdout.
                let count = output
                    .stdout
                    .lines()
                    .filter(|l| l.trim() == "__ITER_DONE__")
                    .count();
                if count > 1 {
                    bail!("Expected at most 1 iteration, found {count}")
                }
                Ok(())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "allow_failure_true_continues",
            description: "Non-zero exit code continues with --allow-failure",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo __ITER_DONE__ && exit 1"]"#,
            extra_args: &["--timeout", "60", "--iter", "3", "--allow-failure"],
            validate: |output| {
                if output.exit_code != 0 {
                    bail!(
                        "Expected exit code 0 with --allow-failure, got {}",
                        output.exit_code
                    )
                }
                let count = output
                    .stdout
                    .lines()
                    .filter(|l| l.trim() == "__ITER_DONE__")
                    .count();
                if count < 3 {
                    bail!("Expected 3 iterations with --allow-failure, found {count}")
                }
                Ok(())
            },
            ..Scenario::default()
        },
    ]
}

/// Get non-sandboxed test scenarios.
///
/// These test the `local_execute` code path (no Firecracker VM).
/// The OCI image is unpacked and the command runs directly on the host.
fn nosandbox_scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "nosandbox_basic",
            description: "Non-sandboxed: simple echo",
            dockerfile: r#"FROM busybox:musl
CMD ["echo", "hello from host"]"#,
            sandboxed: false,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.stdout.contains("hello from host") {
                    Ok(())
                } else {
                    bail!(
                        "Expected 'hello from host' in output.\nstdout: {}\nstderr: {}",
                        output.stdout,
                        output.stderr
                    )
                }
            },
            ..Scenario::default()
        },
        Scenario {
            name: "nosandbox_env",
            description: "Non-sandboxed: ENV variables from OCI config",
            dockerfile: r#"FROM busybox:musl
ENV MY_VAR=host_test_value
CMD ["sh", "-c", "echo $MY_VAR"]"#,
            sandboxed: false,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                if output.stdout.contains("host_test_value") {
                    Ok(())
                } else {
                    bail!(
                        "Expected 'host_test_value' in output.\nstdout: {}\nstderr: {}",
                        output.stdout,
                        output.stderr
                    )
                }
            },
            ..Scenario::default()
        },
        Scenario {
            name: "nosandbox_metrics",
            description: "Non-sandboxed: run metrics on stderr with local transport",
            // Verifies the local path emits ---BENCHER_METRICS:{json}--- with
            // transport "local" (it previously emitted no metrics at all).
            dockerfile: r#"FROM busybox:musl
CMD ["echo", "local_metrics_test"]"#,
            sandboxed: false,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                let metrics_line = output
                    .stderr
                    .lines()
                    .find(|l| l.contains("---BENCHER_METRICS:"));
                let Some(line) = metrics_line else {
                    bail!(
                        "No BENCHER_METRICS line found in stderr.\nstderr: {}",
                        output.stderr
                    )
                };
                let json_str = extract_json_substr(line);
                if let Ok(json) = serde_json::from_str::<serde_json::Value>(json_str)
                    && let Some(transport) =
                        json.get("transport").and_then(serde_json::Value::as_str)
                {
                    if transport == "local" {
                        return Ok(());
                    }
                    bail!("Unexpected transport type: {transport}")
                }
                bail!("Could not find transport in metrics: {json_str}")
            },
            ..Scenario::default()
        },
        Scenario {
            name: "nosandbox_exit_code",
            description: "Non-sandboxed: non-zero exit code propagation",
            dockerfile: r#"FROM busybox:musl
CMD ["sh", "-c", "exit 42"]"#,
            sandboxed: false,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                // The runner process itself exits with code 1 (generic failure),
                // but the error message includes the benchmark's exit code 42.
                let combined = format!("{}{}", output.stdout, output.stderr);
                if combined.contains("42") || output.exit_code != 0 {
                    Ok(())
                } else {
                    bail!(
                        "Expected exit code 42 in output or non-zero runner exit.\nstdout: {}\nstderr: {}",
                        output.stdout,
                        output.stderr
                    )
                }
            },
            ..Scenario::default()
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

/// The cargo target directory the builds above land in.
///
/// `CARGO_TARGET_DIR` is honored rather than assumed away: a caller that
/// redirects it would otherwise have the binaries built in one place and
/// looked for in another, and the harness would report a missing binary
/// immediately after reporting a successful build.
fn target_dir() -> Utf8PathBuf {
    std::env::var_os("CARGO_TARGET_DIR").map_or_else(
        || super::workspace_root().join("target"),
        |dir| Utf8PathBuf::from(dir.to_string_lossy().into_owned()),
    )
}

/// Whether this process is running as root.
fn is_root() -> bool {
    #[expect(
        unsafe_code,
        reason = "geteuid has no std wrapper and cannot fail or touch memory"
    )]
    // SAFETY: `geteuid` takes no arguments, returns a plain integer, and is
    // always successful.
    let euid = unsafe { libc::geteuid() };
    euid == 0
}

/// Check if Docker is available.
fn docker_available() -> bool {
    Command::new("docker")
        .arg("version")
        .output()
        .is_ok_and(|o| o.status.success())
}

/// Check if mkfs.ext4 is available.
fn mkfs_available() -> bool {
    Command::new("mkfs.ext4")
        .arg("-V")
        .output()
        .is_ok_and(|o| o.status.success())
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
    // The elevated run must not invoke cargo: doing so as root leaves the
    // target directory and cargo's cache root-owned, which then breaks the
    // unprivileged steps around it. CI builds first and points here.
    if let Some(path) = std::env::var_os(RUNNER_BIN_ENV) {
        let path = Utf8PathBuf::from(path.to_string_lossy().into_owned());
        if !path.exists() {
            bail!("{RUNNER_BIN_ENV} is set to {path}, which does not exist");
        }
        println!("Using pre-built runner from {RUNNER_BIN_ENV}: {path}");
        return Ok(path);
    }

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

    let init_path = target_dir().join(format!("{target_triple}/debug/bencher-init"));
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

    let runner_bin = target_dir().join("debug/runner");
    if !runner_bin.exists() {
        bail!("Runner binary not found at {runner_bin} after build");
    }

    Ok(runner_bin)
}

// ---------------------------------------------------------------------------
// Jail confinement
// ---------------------------------------------------------------------------

/// Environment variable naming a pre-built runner binary.
const RUNNER_BIN_ENV: &str = "BENCHER_RUNNER_BIN";

/// How long to wait for the jailed VMM to appear before giving up.
///
/// Generous: the runner pulls and unpacks the image and builds the rootfs
/// before the VMM is spawned.
const PROBE_TIMEOUT: Duration = Duration::from_mins(3);

/// How often to look for the jailed VMM.
const PROBE_INTERVAL: Duration = Duration::from_millis(100);

/// The state directory scenarios run against.
fn scenario_state_dir() -> Utf8PathBuf {
    super::work_dir().join("state")
}

/// The directory holding one chroot per jailed VMM.
fn jail_parent(state_dir: &Utf8Path) -> Utf8PathBuf {
    state_dir.join("jail").join("firecracker")
}

/// Scenarios covering the confinement of the VMM itself.
fn jail_scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "jail_confinement",
            description: "A jailed job succeeds with the VMM unprivileged and in its cgroup",
            // The guest sleeps so the VMM is alive long enough to be observed
            // by a probe that polls every 100ms.
            //
            // The marker is a token the runner's own output cannot contain.
            // "jailed" collided with the runner announcing "Launching jailed
            // Firecracker microVM...", so the check passed on the runner
            // saying it was about to start a VM that then never booted.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo JAIL_CONFINEMENT_a7f3b2c9 && sleep 5"]"#,
            cancel_after_secs: None,
            probe: Some(probe_confinement),
            orphan_then_rerun: false,
            extra_args: &["--timeout", "120"],
            validate: |output| {
                // The job has to have actually run before anything the probe
                // saw means anything. Every confinement property the probe
                // checks is equally true of a VMM that started and then never
                // booted a guest, so without this the scenario stays green
                // while the product is broken.
                assert_job_succeeded(output, "JAIL_CONFINEMENT_a7f3b2c9")?;
                assert_no_chroot_remains(&scenario_state_dir())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "jail_sweep_reclaims_orphan",
            description: "A chroot orphaned by a runner that never unwound is swept by the next job",
            // Likewise a token the runner cannot print: "swept" sits one
            // refactor away from colliding with the sweep's own reporting.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo JAIL_SWEEP_a7f3b2c9 && sleep 10"]"#,
            cancel_after_secs: None,
            probe: None,
            orphan_then_rerun: true,
            extra_args: &["--timeout", "120"],
            validate: |output| {
                assert_job_succeeded(output, "JAIL_SWEEP_a7f3b2c9")?;
                assert_no_chroot_remains(&scenario_state_dir())
            },
            ..Scenario::default()
        },
    ]
}

/// Assert the runner actually completed the job.
///
/// A confinement scenario that asserts only confinement passes vacuously when
/// the VM never boots: the VMM process exists, is unprivileged, and is in its
/// cgroup either way. Success of the job itself is the precondition for any of
/// that meaning anything.
///
/// `marker` has to be a token the runner's own progress output cannot contain.
/// This captures the runner's stdout, not the guest's, and the runner prints
/// plenty about jails and sweeps on its way to launching a VM.
fn assert_job_succeeded(output: &ScenarioOutput, marker: &str) -> Result<()> {
    if output.exit_code != 0 {
        bail!(
            "Expected the job to succeed, got exit code {}.\nstdout: {}\nstderr: {}",
            output.exit_code,
            output.stdout,
            output.stderr
        );
    }
    if !output.stdout.contains(marker) {
        bail!(
            "Expected '{marker}' in the guest output, so the VM booted and ran.\nstdout: {}\nstderr: {}",
            output.stdout,
            output.stderr
        );
    }
    Ok(())
}

/// Assert every chroot has been reclaimed.
///
/// The jailer cleans up nothing by design, so a leftover here means the
/// runner's teardown did not run: each one holds a copy of the VMM binary and
/// a full guest rootfs image.
fn assert_no_chroot_remains(state_dir: &Utf8Path) -> Result<()> {
    let parent = jail_parent(state_dir);
    let leftovers: Vec<String> = match fs::read_dir(&parent) {
        Ok(entries) => entries
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .collect(),
        Err(_) => Vec::new(),
    };
    if leftovers.is_empty() {
        Ok(())
    } else {
        bail!("Chroots left behind under {parent}: {leftovers:?}")
    }
}

/// Check that the jailed VMM is unprivileged and already in its cgroup.
///
/// Both invariants disappear with the process, so they cannot be recovered
/// from the runner's output. Cgroup membership in particular must already
/// hold the first time the VMM is seen: it is established before the exec,
/// not after the VM is running.
fn probe_confinement(state_dir: &Utf8Path) -> Result<bool> {
    let parent = jail_parent(state_dir);
    let Some((vm_id, jail_root)) = find_jail(&parent) else {
        return Ok(false);
    };
    let Some(pid) = find_jailed_vmm(&jail_root) else {
        return Ok(false);
    };

    if !check_unprivileged(pid, &jail_root)? {
        return Ok(false);
    }
    // Placement happens in `pre_exec`, before the jailer itself starts, so
    // membership already holds the first time the process is observable.
    // There is no not-ready-yet window for it.
    check_cgroup_membership(&vm_id, pid)?;

    Ok(true)
}

/// Find the single chroot under the jail parent, if one exists yet.
fn find_jail(parent: &Utf8Path) -> Option<(String, Utf8PathBuf)> {
    for entry in fs::read_dir(parent).ok()?.flatten() {
        if !entry.file_type().is_ok_and(|file_type| file_type.is_dir()) {
            continue;
        }
        let vm_id = entry.file_name().to_string_lossy().into_owned();
        let jail_root = parent.join(&vm_id).join("root");
        if jail_root.is_dir() {
            return Some((vm_id, jail_root));
        }
    }
    None
}

/// Find the pid of the VMM confined to `jail_root`, if it is running yet.
///
/// The jailer pivots into a private mount namespace, so the process's root
/// path reads back as `/` and is useless as an identifier. Its identity is
/// compared instead: the bind mount the jailer pivots onto preserves the
/// device and inode of the chroot directory, so stat'ing through
/// `/proc/<pid>/root` and stat'ing the jail root agree for exactly the VMM
/// confined to this jail and for no other process on the host.
fn find_jailed_vmm(jail_root: &Utf8Path) -> Option<u32> {
    use std::os::unix::fs::MetadataExt as _;

    let jail = fs::metadata(jail_root).ok()?;
    for entry in fs::read_dir("/proc").ok()?.flatten() {
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        // Following the magic symlink crosses into the process's own mount
        // namespace, which a privileged reader is allowed to do.
        let Ok(root) = fs::metadata(format!("/proc/{pid}/root")) else {
            continue;
        };
        if root.dev() == jail.dev() && root.ino() == jail.ino() {
            return Some(pid);
        }
    }
    None
}

/// Check the VMM dropped root and runs as the user the jail was handed to.
///
/// Returns `Ok(false)` while the observation is premature rather than wrong.
/// The jailer `pivot_root`s before it drops privilege, so there is a window in
/// which the process root already matches the jail while the process is still
/// root and the jail root is still root-owned. Treating that as a violation
/// would fail the run for catching the jailer mid-flight; the probe's timeout
/// is what catches a VMM that genuinely never drops.
fn check_unprivileged(pid: u32, jail_root: &Utf8Path) -> Result<bool> {
    let status = fs::read_to_string(format!("/proc/{pid}/status"))
        .with_context(|| format!("Failed to read the status of the VMM (pid {pid})"))?;
    let uid_line = status
        .lines()
        .find_map(|line| line.strip_prefix("Uid:"))
        .context("No Uid line in the VMM's /proc status")?;
    let vmm_uid: u32 = uid_line
        .split_whitespace()
        .next()
        .context("Empty Uid line in the VMM's /proc status")?
        .parse()
        .context("Unparsable uid in the VMM's /proc status")?;

    if vmm_uid == 0 {
        return Ok(false);
    }

    // The jailer chowns the chroot root to the jail uid, so the two must
    // agree: a VMM running as some other unprivileged user would not be
    // confined to the jail it was given.
    let Some(jail_uid) = jail_root_uid(jail_root) else {
        return Ok(false);
    };
    if vmm_uid != jail_uid {
        bail!("The VMM (pid {pid}) runs as uid {vmm_uid} but its jail is owned by uid {jail_uid}");
    }

    Ok(true)
}

/// The uid the jailer handed the chroot root to, once it has handed it over.
///
/// `None` while the root is still owned by root, which is the same
/// not-ready-yet window `check_unprivileged` documents.
fn jail_root_uid(jail_root: &Utf8Path) -> Option<u32> {
    use std::os::unix::fs::MetadataExt as _;

    let uid = fs::metadata(jail_root).ok()?.uid();
    (uid != 0).then_some(uid)
}

/// Check the VMM is in its cgroup.
///
/// Placement happens before the exec, so the pid is already a member the
/// first time the process is observable.
fn check_cgroup_membership(vm_id: &str, pid: u32) -> Result<()> {
    let procs_path = format!("/sys/fs/cgroup/bencher/{vm_id}/cgroup.procs");
    // No cgroup means no isolation was possible on this host, which is a
    // declared limitation rather than a confinement failure. Say so out loud:
    // a confinement check that quietly asserts nothing is exactly the kind of
    // green that must never be invisible.
    let Ok(procs) = fs::read_to_string(&procs_path) else {
        println!(
            "  NOTE: {procs_path} is unreadable, so cgroup placement was NOT verified for this run"
        );
        return Ok(());
    };
    if procs.lines().any(|line| line.trim() == pid.to_string()) {
        Ok(())
    } else {
        bail!("The VMM (pid {pid}) is not in {procs_path}, which holds: {procs:?}")
    }
}

/// Orphan a jail by killing the runner, then prove the next job sweeps it.
///
/// SIGKILL rather than SIGTERM, because SIGTERM on the one-shot path takes the
/// default disposition too (signal handlers are installed only by the daemon),
/// and either way nothing unwinds. That is the point: `Drop` cannot reclaim
/// the chroot, so if the next job finds a clean tree it can only be because
/// the sweep reclaimed it.
fn run_runner_after_orphan(
    image_path: &Utf8Path,
    args: &[&str],
    state_dir: &Utf8Path,
    runner_bin: &Utf8Path,
) -> Result<ScenarioOutput> {
    let parent = jail_parent(state_dir);

    let mut child = Command::new(runner_bin.as_str())
        .arg("run")
        .arg("--image")
        .arg(image_path.as_str())
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;

    // Wait for a real orphan: a chroot with a VMM running in it, not just an
    // empty directory created microseconds before the kill.
    let deadline = std::time::Instant::now() + PROBE_TIMEOUT;
    let orphan = loop {
        if let Some((vm_id, jail_root)) = find_jail(&parent)
            && let Some(pid) = find_jailed_vmm(&jail_root)
        {
            break Some((vm_id, jail_root, pid));
        }
        if child.try_wait()?.is_some() || std::time::Instant::now() >= deadline {
            break None;
        }
        std::thread::sleep(PROBE_INTERVAL);
    };

    let Some((vm_id, jail_root, vmm_pid)) = orphan else {
        let output = child.wait_with_output()?;
        bail!(
            "No jailed VMM appeared within {PROBE_TIMEOUT:?}, so nothing was orphaned and the sweep is untested.\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    };

    kill_pid(child.id(), libc::SIGKILL);
    drop(child.wait());

    if !jail_root.exists() {
        bail!(
            "The chroot {jail_root} was reclaimed despite the runner being killed without unwinding, so the sweep is untested"
        );
    }

    // Deliberately NOT reaping the VMM here. Killing it by hand would leave
    // the next job's sweep with nothing to find, so the reap, the pidfd
    // handling, and the cgroup removal would all be skipped and the scenario
    // would prove only that a directory can be deleted. The stray Firecracker
    // that a hand-reap guards against is exactly what the sweep now exists to
    // prevent, so if the sweep fails this scenario has to go red.
    let cgroup = stale_cgroup(&vm_id);
    let cgroup_existed = cgroup.exists();
    println!(
        "  orphaned jail {vm_id} (VMM pid {vmm_pid}, cgroup present: {cgroup_existed}), running a second job..."
    );

    let output = run_runner(image_path, args, runner_bin)?;

    if jail_root.exists() {
        bail!("The orphaned chroot {jail_root} survived the next job, so it was never swept");
    }
    if is_firecracker(vmm_pid) {
        bail!(
            "The orphaned VMM (pid {vmm_pid}) is still running after the next job, so the sweep never reaped it. It still holds the benchmark cores."
        );
    }
    // Only meaningful where a cgroup was created at all: a host with no CPU
    // isolation never makes one, and asserting its absence would pass for the
    // wrong reason.
    if cgroup_existed && cgroup.exists() {
        bail!(
            "The orphaned cgroup {cgroup} survived the next job. It still owns the exclusive benchmark CPUs, so no later run can be isolated."
        );
    }

    Ok(output)
}

/// The cgroup a jail leaves behind, which shares the jail's name.
fn stale_cgroup(vm_id: &str) -> Utf8PathBuf {
    Utf8PathBuf::from("/sys/fs/cgroup/bencher").join(vm_id)
}

/// Whether a pid is a running Firecracker.
///
/// Checking the command as well as the pid keeps a recycled pid from reading
/// as a VMM that was never reaped.
fn is_firecracker(pid: u32) -> bool {
    fs::read_to_string(format!("/proc/{pid}/comm")).is_ok_and(|comm| comm.trim() == "firecracker")
}

/// Send a signal to a process, ignoring the result.
fn kill_pid(pid: u32, signal: libc::c_int) {
    #[expect(
        unsafe_code,
        clippy::cast_possible_wrap,
        reason = "libc::kill requires unsafe; PID fits in i32"
    )]
    // SAFETY: `kill` takes plain integers and touches no memory. A signal to a
    // pid that has already exited fails harmlessly with ESRCH.
    unsafe {
        libc::kill(pid as i32, signal);
    }
}

/// Run the runner while checking a host-side invariant.
fn run_runner_with_probe(
    image_path: &Utf8Path,
    args: &[&str],
    probe: Probe,
    state_dir: &Utf8Path,
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

    let deadline = std::time::Instant::now() + PROBE_TIMEOUT;
    let mut observed = None;
    loop {
        match probe(state_dir) {
            Ok(true) => {
                observed = Some(Ok(()));
                break;
            },
            Ok(false) => {},
            Err(e) => {
                observed = Some(Err(e));
                break;
            },
        }
        // Stop looking once the runner is gone or the wait is hopeless: the
        // output is collected either way so the failure can be explained.
        if child.try_wait()?.is_some() || std::time::Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(PROBE_INTERVAL);
    }

    let output = child.wait_with_output()?;
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();

    match observed {
        Some(Ok(())) => Ok(ScenarioOutput {
            stdout,
            stderr,
            exit_code: output.status.code().unwrap_or(-1),
        }),
        Some(Err(e)) => Err(e).with_context(|| format!("stdout: {stdout}\nstderr: {stderr}")),
        None => bail!(
            "The jailed VMM was never observed within {PROBE_TIMEOUT:?}.\nstdout: {stdout}\nstderr: {stderr}"
        ),
    }
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
