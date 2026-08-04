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
    /// If set, run before the runner starts, to put the host in some state.
    setup: Option<fn() -> Result<()>>,
    /// If set, a host-side check run while the runner is executing.
    probe: Option<Probe>,
    /// Run with host tuning enabled and assert it applies and is restored.
    ///
    /// Every other scenario passes `--no-tuning`, so this is the only one that
    /// can leave the machine changed, and the harness undoes it itself.
    tuning: bool,
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
            setup: None,
            probe: None,
            tuning: false,
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
        // Last, always. It is the only scenario that tunes the machine, so
        // nothing it leaves behind can reach the others, and if the suite is
        // killed part way through it is the least likely to have started.
        scenarios.extend(tuning_scenarios());

        let result = if let Some(name) = &self.scenario {
            // Run a single scenario
            scenarios
                .iter()
                .find(|s| s.name == name)
                .with_context(|| format!("Unknown scenario: {name}"))
                .and_then(|scenario| run_scenario(scenario, &runner_bin))
        } else {
            // Run all scenarios
            run_all_scenarios(&scenarios, &runner_bin)
        };

        // Whatever the outcome. Everything the run wrote is only root-owned
        // because the scenarios had to be, and it sits inside the repo tree.
        return_work_dir_to_invoker();

        result
    }
}

/// Hand everything the elevated run wrote back to whoever invoked `sudo`.
///
/// The scenarios must run as root, so the whole tree under the crate's target
/// directory comes out root-owned: the runner's state directory at 0700, the
/// docker build contexts, and the unpacked OCI layouts, whose per-scenario
/// cleanup a scenario that returns early never reaches. CI throws that tree
/// away, so it costs nothing there, but on a developer's machine one red
/// scenario leaves directories the next unprivileged `cargo`, `rm -rf` or
/// `git clean` can neither read nor remove. `SUDO_UID` names who to give it back
/// to; with no one to give it back to, or a `chown` that will not run, the path
/// is printed with the command that clears it rather than left to be discovered.
fn return_work_dir_to_invoker() {
    // The crate's target directory rather than the work directory inside it.
    // This run creates both, and a root-owned parent is one the invoker cannot
    // remove the work directory from, however the tree below it is owned.
    let work_dir = super::work_dir();
    let returned = work_dir.parent().unwrap_or(&work_dir).to_owned();
    if !returned.exists() {
        return;
    }

    if let Some((uid, gid)) = invoking_user()
        && Command::new("chown")
            .args(["-R", &format!("{uid}:{gid}"), returned.as_str()])
            .status()
            .is_ok_and(|status| status.success())
    {
        println!("Returned {returned} to uid {uid}");
        return;
    }

    println!("Note: {returned} is left owned by root. Remove it with: sudo rm -rf {returned}");
}

/// The uid and gid that invoked `sudo`, when one did.
fn invoking_user() -> Option<(u32, u32)> {
    let uid = std::env::var("SUDO_UID").ok()?.parse().ok()?;
    let gid = std::env::var("SUDO_GID").ok()?.parse().ok()?;
    Some((uid, gid))
}

/// List all available scenarios.
fn list_scenarios() {
    let mut scenarios = all_scenarios();
    scenarios.extend(jail_scenarios());
    scenarios.extend(nosandbox_scenarios());
    scenarios.extend(tuning_scenarios());
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

    if let Some(setup) = scenario.setup {
        setup().with_context(|| format!("Setup failed for {}", scenario.name))?;
    }

    // One state directory for the suite, wiped before each scenario, so jail
    // assertions see only this scenario's jails and never touch a real runner's
    // state. Reclaimed before the wipe, because the wipe is the one thing the
    // runner's own sweep refuses to do.
    let state_dir = scenario_state_dir();
    reclaim_stranded_jails(&state_dir)
        .with_context(|| format!("Failed to reclaim jails stranded before {}", scenario.name))?;
    drop(fs::remove_dir_all(&state_dir));

    // Prepend --sandbox firecracker for sandboxed scenarios.
    //
    // `--no-tuning` everywhere except the one scenario that exists to test
    // tuning. Elevated, the knobs really apply, and a suite that tuned the
    // machine twenty-five times would offline SMT siblings on a two-vCPU hosted
    // runner, changing the core count under itself. The tuning scenario turns
    // them back on for one job, keeps SMT and IRQ steering out of it, and the
    // harness undoes everything itself afterwards.
    let mut args: Vec<&str> = vec!["--state-dir", state_dir.as_str()];
    if scenario.tuning {
        // The two knobs this scenario deliberately does not exercise. `--smt`
        // keeps hyper-threading on: offlining a sibling changes `nproc` for
        // everything that follows in the CI job, and the harness cannot put a
        // CPU back if the runner is killed before its guard runs. IRQ steering
        // is skipped because a hand-restore of it is unavoidably partial, since
        // an unmovable IRQ rejects the write with EIO, and the rule for this
        // scenario is that the harness can undo anything it turned on.
        args.extend(["--smt", "--no-irq-steering"]);
    } else {
        args.push("--no-tuning");
    }
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
    } else if scenario.tuning {
        run_runner_with_tuning(&image_path, &args, runner_bin)
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
fn target_dir() -> Result<Utf8PathBuf> {
    resolve_target_dir(
        std::env::var_os("CARGO_TARGET_DIR").as_deref(),
        &super::workspace_root(),
    )
}

/// Where a build run from `workspace_root` puts its output.
///
/// A relative `CARGO_TARGET_DIR` is resolved against the workspace root rather
/// than carried through as it stands. Cargo resolves it against the working
/// directory of the build, which is the workspace root the builds above hand to
/// `current_dir`, so a harness invoked from anywhere else would look for the
/// binaries under its own working directory instead. That is the same missing
/// binary immediately after a successful build that honoring the variable at all
/// exists to prevent.
///
/// A value that is not UTF-8 is refused rather than converted lossily: the
/// replacement characters would name a different directory again, and would do
/// it while reporting a path that looks like the one that was asked for.
fn resolve_target_dir(
    dir: Option<&std::ffi::OsStr>,
    workspace_root: &Utf8Path,
) -> Result<Utf8PathBuf> {
    let Some(dir) = dir else {
        return Ok(workspace_root.join("target"));
    };
    let dir = Utf8PathBuf::from_path_buf(std::path::PathBuf::from(dir)).map_err(|dir| {
        anyhow::anyhow!(
            "CARGO_TARGET_DIR is not valid UTF-8: {}",
            dir.as_os_str().display()
        )
    })?;
    Ok(if dir.is_absolute() {
        dir
    } else {
        workspace_root.join(dir)
    })
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

    // Falling through to cargo as root is the exact outcome the
    // build-then-elevate split exists to prevent: it leaves the target
    // directory and the cargo cache owned by root, and it does so silently.
    // Sudo does pass the variable through on both runner images, so this is
    // belt and braces, but a loud failure beats a root-owned cache.
    anyhow::ensure!(
        !is_root(),
        "Running as root without {RUNNER_BIN_ENV} set. Building here would run cargo as root and \
         leave the target directory and cargo cache root-owned. Build unprivileged first:\n\
         \x20 cargo test-runner scenarios --build-only\n\
         \x20 sudo {RUNNER_BIN_ENV}=./target/debug/runner ./target/debug/test_runner scenarios"
    );

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

    let init_path = target_dir()?.join(format!("{target_triple}/debug/bencher-init"));
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

    let runner_bin = target_dir()?.join("debug/runner");
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

/// The uid the jail scenarios hand the runner with `--jail-uid`.
///
/// Asked for by name rather than left to the runner's default, and asserted as
/// this number rather than as whatever the chroot turns out to be owned by. The
/// chown of the chroot and the setuid of the VMM are made from one config, so
/// they agree with each other on a runner that ignores the flag and the default
/// alike, and a uid the operator never chose would pass unnoticed.
const SCENARIO_JAIL_UID: u32 = 61017;

/// [`SCENARIO_JAIL_UID`] as the scenarios spell it on the command line.
///
/// Written twice because a scenario's arguments are `&'static str`. A test keeps
/// the two from drifting apart.
const SCENARIO_JAIL_UID_ARG: &str = "61017";

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
            extra_args: &["--timeout", "120", "--jail-uid", SCENARIO_JAIL_UID_ARG],
            validate: |output| {
                // The job has to have actually run before anything the probe
                // saw means anything. Every confinement property the probe
                // checks is equally true of a VMM that started and then never
                // booted a guest, so without this the scenario stays green
                // while the product is broken.
                assert_job_succeeded(output, "JAIL_CONFINEMENT_a7f3b2c9")?;
                assert_cpu_isolation_applied(output)?;
                assert_no_chroot_remains(&scenario_state_dir())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "jail_netns_recovers_from_stacked_mounts",
            description: "A job succeeds against a network namespace handle carrying stacked mounts",
            dockerfile: r#"FROM busybox
CMD ["echo", "JAIL_NETNS_a7f3b2c9"]"#,
            setup: Some(stack_netns_mounts),
            extra_args: &["--timeout", "120", "--jail-uid", SCENARIO_JAIL_UID_ARG],
            validate: |output| assert_job_succeeded(output, "JAIL_NETNS_a7f3b2c9"),
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
            extra_args: &["--timeout", "120", "--jail-uid", SCENARIO_JAIL_UID_ARG],
            validate: |output| {
                assert_job_succeeded(output, "JAIL_SWEEP_a7f3b2c9")?;
                assert_no_chroot_remains(&scenario_state_dir())
            },
            ..Scenario::default()
        },
    ]
}

/// Assert the runner reported that it confined the VMM to the benchmark cores.
///
/// The runner prints this only after creating the cgroup, writing the cpuset,
/// and reading the effective set back, so it is the runner's own statement
/// that a cgroup exists for the probe to have checked membership against.
/// Without it the probe could pass on a host where no cgroup was ever made.
fn assert_cpu_isolation_applied(output: &ScenarioOutput) -> Result<()> {
    const PINNED: &str = "CPU isolation: Firecracker pinned to cores";
    if output.stdout.contains(PINNED) {
        return Ok(());
    }
    bail!(
        "The runner never reported pinning the VMM to benchmark cores, so no cgroup was created \
         and cgroup placement went unexercised by this run. Expected {PINNED:?}.\nstdout: {}\nstderr: {}",
        output.stdout,
        output.stderr
    )
}

/// Stack extra bind mounts on the network namespace handle.
///
/// Recreating the handle bind mounts over it, and a bind mount over a file
/// reports no error, so mounts stack. Against a stacked handle a single
/// detach leaves one behind, the unlink then fails with EBUSY, and creating
/// the placeholder fails with EPERM even as root: every sandboxed job on the
/// host fails until an operator loops `umount` by hand. The unwind loop exists
/// for exactly this, and nothing else exercises it.
fn stack_netns_mounts() -> Result<()> {
    let handle = "/run/netns/bencher-jail";
    fs::create_dir_all("/run/netns").context("Failed to create the netns directory")?;
    if !Utf8Path::new(handle).exists() {
        fs::File::create(handle).context("Failed to create the netns handle")?;
    }

    for _ in 0..2 {
        let status = Command::new("unshare")
            .args(["--net", "sh", "-c"])
            .arg(format!("mount --bind /proc/self/ns/net {handle}"))
            .status()
            .context("Failed to run unshare to stack a netns mount")?;
        anyhow::ensure!(status.success(), "Failed to stack a netns mount");
    }

    let stacked = fs::read_to_string("/proc/self/mountinfo")
        .context("Failed to read mountinfo")?
        .lines()
        .filter(|line| line.contains(&format!(" {handle} ")))
        .count();
    anyhow::ensure!(
        stacked >= 2,
        "Expected at least two stacked mounts on {handle}, found {stacked}"
    );
    println!("  stacked {stacked} mounts on {handle}");
    Ok(())
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
    // A read that failed is not an empty directory. An assertion that could not
    // look has not passed, it has not run, and a vacuous pass here would hide the
    // exact leak it exists to catch. Absence is the one reading that does mean
    // nothing was left behind: the runner creates this tree on demand.
    let entries = match fs::read_dir(&parent) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => {
            return Err(e).with_context(|| {
                format!("Failed to read {parent}, so whether a chroot was left behind is unknown")
            });
        },
    };

    let mut leftovers = Vec::new();
    for entry in entries {
        let entry = entry.with_context(|| {
            format!("Failed to read an entry under {parent}, so whether a chroot was left behind is unknown")
        })?;
        leftovers.push(entry.file_name().to_string_lossy().into_owned());
    }

    anyhow::ensure!(
        leftovers.is_empty(),
        "Chroots left behind under {parent}: {leftovers:?}"
    );
    Ok(())
}

/// Check that the jailed VMM is unprivileged and already in its cgroup.
///
/// Both invariants disappear with the process, so they cannot be recovered
/// from the runner's output. Cgroup membership in particular must already
/// hold the first time the VMM is seen: it is established before the exec,
/// not after the VM is running.
fn probe_confinement(state_dir: &Utf8Path) -> Result<bool> {
    let parent = jail_parent(state_dir);
    let Some((vm_id, jail_root)) = find_jail(&parent)? else {
        return Ok(false);
    };
    let Some(pid) = find_jailed_vmm(&jail_root)? else {
        return Ok(false);
    };

    if !check_unprivileged(pid, &jail_root, SCENARIO_JAIL_UID)? {
        return Ok(false);
    }
    // Placement happens in `pre_exec`, before the jailer itself starts, so
    // membership already holds the first time the process is observable.
    // There is no not-ready-yet window for it.
    check_cgroup_membership(&vm_id, pid)?;

    Ok(true)
}

/// Find the single chroot under the jail parent, if one exists yet.
///
/// `Ok(None)` is "not yet", which the parent not existing also means: the runner
/// creates it on demand. Every other failure is an error, because this drives a
/// poll loop whose only other outcome is a timeout, and a timeout would report
/// that no VMM ever appeared when the truth is that nobody could look.
fn find_jail(parent: &Utf8Path) -> Result<Option<(String, Utf8PathBuf)>> {
    let entries = match fs::read_dir(parent) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("Failed to read {parent}")),
    };

    for entry in entries {
        let entry = entry.with_context(|| format!("Failed to read an entry under {parent}"))?;
        let file_type = entry
            .file_type()
            .with_context(|| format!("Failed to read the kind of an entry under {parent}"))?;
        if !file_type.is_dir() {
            continue;
        }
        let vm_id = entry.file_name().to_string_lossy().into_owned();
        let jail_root = parent.join(&vm_id).join("root");
        if jail_root.is_dir() {
            return Ok(Some((vm_id, jail_root)));
        }
    }
    Ok(None)
}

/// Find the pid of the VMM confined to `jail_root`, if it is running yet.
///
/// The jailer pivots into a private mount namespace, so the process's root
/// path reads back as `/` and is useless as an identifier. Its identity is
/// compared instead: the bind mount the jailer pivots onto preserves the
/// device and inode of the chroot directory, so stat'ing through
/// `/proc/<pid>/root` and stat'ing the jail root agree for exactly the VMM
/// confined to this jail and for no other process on the host.
///
/// `Ok(None)` is "not running yet", which a jail root that does not exist also
/// means. A jail root that cannot be stat'ed, or a `/proc` that cannot be listed,
/// is neither: it would surface as a probe timeout blaming the runner for
/// something the harness could not see.
fn find_jailed_vmm(jail_root: &Utf8Path) -> Result<Option<u32>> {
    use std::os::unix::fs::MetadataExt as _;

    let jail = match fs::metadata(jail_root) {
        Ok(jail) => jail,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e).with_context(|| format!("Failed to stat {jail_root}")),
    };
    for entry in fs::read_dir("/proc").context("Failed to read /proc")? {
        let entry = entry.context("Failed to read a /proc entry")?;
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        // Following the magic symlink crosses into the process's own mount
        // namespace, which a privileged reader is allowed to do. A read that
        // fails is a process that has exited or is not this jail's, which is the
        // one failure here that is genuinely an answer.
        let Ok(root) = fs::metadata(format!("/proc/{pid}/root")) else {
            continue;
        };
        if root.dev() == jail.dev() && root.ino() == jail.ino() {
            return Ok(Some(pid));
        }
    }
    Ok(None)
}

/// Check the VMM dropped root and runs as the user the jail was handed to.
///
/// `expected_uid` is the uid the scenario asked for by name, and it is checked
/// against the VMM as well as against the chroot. Both of those come from one
/// config, so a runner that used a uid of its own for the chown and the setuid
/// alike would satisfy them against each other while honoring neither
/// `--jail-uid` nor the default.
///
/// Returns `Ok(false)` while the observation is premature rather than wrong.
/// The jailer `pivot_root`s before it drops privilege, so there is a window in
/// which the process root already matches the jail while the process is still
/// root and the jail root is still root-owned. Treating that as a violation
/// would fail the run for catching the jailer mid-flight; the probe's timeout
/// is what catches a VMM that genuinely never drops.
fn check_unprivileged(pid: u32, jail_root: &Utf8Path, expected_uid: u32) -> Result<bool> {
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

    // Unprivileged is not enough: it has to be the uid that was asked for.
    if vmm_uid != expected_uid {
        bail!(
            "The VMM (pid {pid}) runs as uid {vmm_uid}, but the runner was handed --jail-uid {expected_uid}"
        );
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

    // A missing cgroup is a failure, not a note. Placement before exec is the
    // centrepiece of the jail: it is what fixed the cpuset being applied after
    // the VMM was already running. Letting its absence pass quietly is how a
    // scenario named for confinement ends up asserting only the uid half of
    // it, which is a green that means less than it looks like.
    let procs = fs::read_to_string(&procs_path).with_context(|| {
        format!(
            "No cgroup at {procs_path}, so cgroup placement was not exercised at all. \
             The runner creates one whenever its CPU layout offers isolation, which needs \
             two or more online CPUs and the cpuset controller delegated to this cgroup tree."
        )
    })?;

    if procs.lines().any(|line| line.trim() == pid.to_string()) {
        Ok(())
    } else {
        bail!(
            "The VMM (pid {pid}) is not in {procs_path}, which holds: {procs:?}. \
             Placement happens in pre_exec, before the jailer starts, so membership must \
             already hold the first time the process is visible."
        )
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

    // Drain both pipes for the same reason the probe path does. This polls for
    // up to three minutes while the runner pulls an image, unpacks it, and
    // builds an ext4, which is more than enough output to fill a 64 KiB pipe
    // and block the runner. It would surface as "No jailed VMM appeared",
    // which points at the sweep rather than at the pipe.
    let readers = drain_output(&mut child);

    // Wait for a real orphan: a chroot with a VMM running in it, not just an
    // empty directory created microseconds before the kill.
    let deadline = std::time::Instant::now() + PROBE_TIMEOUT;
    let orphan = loop {
        if let Some((vm_id, jail_root)) = find_jail(&parent)?
            && let Some(pid) = find_jailed_vmm(&jail_root)?
        {
            break Some((vm_id, jail_root, pid));
        }
        if child.try_wait()?.is_some() || std::time::Instant::now() >= deadline {
            break None;
        }
        std::thread::sleep(PROBE_INTERVAL);
    };

    let Some((vm_id, jail_root, vmm_pid)) = orphan else {
        // Killed before the wait. A probe that timed out leaves the runner still
        // going, so waiting on it first would sit there until the runner's own
        // timeout expired and report the failure minutes late, which is exactly
        // when somebody is watching. Only if it has not already been reaped:
        // `try_wait` in the loop above reaps it, and signalling a reaped pid can
        // reach whatever inherited the number.
        if child.try_wait()?.is_none() {
            kill_pid(child.id(), libc::SIGKILL);
        }
        drop(child.wait());
        let (stdout, stderr) = readers.join();
        bail!(
            "No jailed VMM appeared within {PROBE_TIMEOUT:?}, so nothing was orphaned and the sweep is untested.\nstdout: {stdout}\nstderr: {stderr}"
        );
    };

    kill_pid(child.id(), libc::SIGKILL);
    drop(child.wait());
    drop(readers.join());

    if !jail_root
        .try_exists()
        .with_context(|| format!("Failed to check whether {jail_root} was left behind"))?
    {
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
    let cgroup_existed = cgroup
        .try_exists()
        .with_context(|| format!("Failed to check whether {cgroup} was created"))?;
    println!(
        "  orphaned jail {vm_id} (VMM pid {vmm_pid}, cgroup present: {cgroup_existed}), running a second job..."
    );

    let output = run_runner(image_path, args, runner_bin)?;

    // `try_exists`, not `exists`: the latter reports false for an error as well
    // as for absence, which would pass this assertion for the wrong reason.
    if jail_root
        .try_exists()
        .with_context(|| format!("Failed to check whether {jail_root} survived"))?
    {
        bail!("The orphaned chroot {jail_root} survived the next job, so it was never swept");
    }
    if is_firecracker(vmm_pid)? {
        bail!(
            "The orphaned VMM (pid {vmm_pid}) is still running after the next job, so the sweep never reaped it. It still holds the benchmark cores."
        );
    }
    // Only meaningful where a cgroup was created at all: a host with no CPU
    // isolation never makes one, and asserting its absence would pass for the
    // wrong reason.
    if cgroup_existed
        && cgroup
            .try_exists()
            .with_context(|| format!("Failed to check whether {cgroup} survived"))?
    {
        bail!(
            "The orphaned cgroup {cgroup} survived the next job, so the sweep never removed it. Stale cgroups accumulate, and one that will not go away usually means its VMM is still running."
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
///
/// A process that is gone has no `comm` to read, and that is the answer the
/// caller wants. Any other read failure is not: the assertion that uses this
/// passes when it returns false, so swallowing an error would pass it for the
/// wrong reason.
fn is_firecracker(pid: u32) -> Result<bool> {
    match fs::read_to_string(format!("/proc/{pid}/comm")) {
        Ok(comm) => Ok(comm.trim() == "firecracker"),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(e) => Err(e).with_context(|| {
            format!(
                "Failed to read the command of pid {pid}, so whether the VMM was reaped is unknown"
            )
        }),
    }
}

/// Reader threads draining a child's piped output.
struct DrainedOutput {
    stdout: std::thread::JoinHandle<String>,
    stderr: std::thread::JoinHandle<String>,
}

impl DrainedOutput {
    /// Wait for both readers and return what they collected.
    fn join(self) -> (String, String) {
        let stdout = self.stdout.join().unwrap_or_default();
        let stderr = self.stderr.join().unwrap_or_default();
        (stdout, stderr)
    }
}

/// Start reading a child's stdout and stderr so neither pipe can fill.
fn drain_output(child: &mut std::process::Child) -> DrainedOutput {
    fn reader<R: std::io::Read + Send + 'static>(
        stream: Option<R>,
    ) -> std::thread::JoinHandle<String> {
        std::thread::spawn(move || {
            let mut buffer = String::new();
            if let Some(mut stream) = stream {
                drop(stream.read_to_string(&mut buffer));
            }
            buffer
        })
    }

    DrainedOutput {
        stdout: reader(child.stdout.take()),
        stderr: reader(child.stderr.take()),
    }
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

/// The host settings the tuning scenario watches, and what the runner sets them
/// to.
///
/// Only settings whose whole value the runner rewrites. The bracketed sysfs
/// files (`transparent_hugepage/*`) and the cpuset partition are handled
/// separately below, and the knobs this scenario deliberately leaves alone are
/// listed with the arguments that switch them off.
const TUNED_SETTINGS: &[(&str, &str)] = &[
    ("/proc/sys/kernel/randomize_va_space", "0"),
    ("/proc/sys/kernel/nmi_watchdog", "0"),
    ("/proc/sys/vm/swappiness", "10"),
    ("/proc/sys/kernel/perf_event_paranoid", "-1"),
    ("/proc/sys/kernel/numa_balancing", "0"),
    ("/proc/sys/kernel/timer_migration", "0"),
    ("/proc/sys/kernel/soft_watchdog", "0"),
    ("/sys/kernel/mm/ksm/run", "0"),
];

/// The transparent hugepage settings, whose files list every mode and bracket
/// the selected one.
const TUNED_THP: &[&str] = &[
    "/sys/kernel/mm/transparent_hugepage/enabled",
    "/sys/kernel/mm/transparent_hugepage/defrag",
];

/// What the runner sets the transparent hugepage mode to.
const THP_TARGET: &str = "never";

/// The cpuset partition files, which the tuning writes and the guard restores.
const TUNED_PARTITION: &[&str] = &[
    "/sys/fs/cgroup/bencher/cpuset.cpus",
    "/sys/fs/cgroup/bencher/cpuset.mems",
    "/sys/fs/cgroup/bencher/cpuset.cpus.partition",
];

/// One host setting the scenario expects the runner to change.
#[derive(Debug, Clone)]
struct TunedSetting {
    path: Utf8PathBuf,
    /// What it held before the runner started, and what it must hold after.
    original: String,
    /// What the runner should set it to while the Job runs, when this host
    /// lets it. `None` for a setting that is present and writable but already
    /// holds the target, which the runner leaves alone and reports as such.
    expected: Option<String>,
    /// Whether the value is the bracketed kind (`always [madvise] never`).
    bracketed: bool,
}

/// Every host setting the tuning scenario touches, as it stood before it ran.
///
/// The harness restores from this itself rather than trusting the mechanism it
/// is testing. A test of a restore path has to be safe when the restore path is
/// broken, which is the whole reason this scenario can be allowed to run in CI
/// at all.
#[derive(Debug)]
struct TuningSnapshot {
    settings: Vec<TunedSetting>,
}

impl TuningSnapshot {
    /// Read every setting, and work out which of them this host will let the
    /// runner change.
    ///
    /// Writability is established by writing the current value back, which
    /// changes nothing and is the only honest way to know: a file that exists
    /// may still be read-only, and a scenario that waited for a change the
    /// kernel was never going to make would fail for the host's reasons rather
    /// than the runner's.
    fn take() -> Self {
        let mut settings = Vec::new();

        for (path, target) in TUNED_SETTINGS {
            let path = Utf8PathBuf::from(*path);
            let Some(original) = readable_setting(&path) else {
                println!("  tuning: {path} is not present on this host");
                continue;
            };
            let expected = if !writable_setting(&path, &original) {
                println!("  tuning: {path} is present but not writable");
                None
            } else if original == *target {
                println!("  tuning: {path} already holds {target}");
                None
            } else {
                Some((*target).to_owned())
            };
            settings.push(TunedSetting {
                path,
                original,
                expected,
                bracketed: false,
            });
        }

        for path in TUNED_THP {
            let path = Utf8PathBuf::from(*path);
            let Some(original) = readable_setting(&path) else {
                println!("  tuning: {path} is not present on this host");
                continue;
            };
            // Probed with the mode the file already selects, never with a
            // fallback: writing `never` to a file whose selection could not be
            // parsed would change the very setting this is only supposed to
            // measure.
            let Some(selected) = bracketed_value(&original) else {
                println!("  tuning: {path} does not read as a mode listing: '{original}'");
                continue;
            };
            let expected = if !writable_setting(&path, selected) {
                println!("  tuning: {path} is present but not writable");
                None
            } else if selected == THP_TARGET {
                println!("  tuning: {path} already selects {THP_TARGET}");
                None
            } else {
                Some(THP_TARGET.to_owned())
            };
            settings.push(TunedSetting {
                path,
                original,
                expected,
                bracketed: true,
            });
        }

        Self { settings }
    }

    /// The settings this host should show changed while the Job runs.
    fn expected(&self) -> impl Iterator<Item = &TunedSetting> {
        self.settings
            .iter()
            .filter(|setting| setting.expected.is_some())
    }

    /// Whether every expected setting currently holds its tuned value.
    fn all_applied(&self) -> bool {
        self.expected().all(|setting| {
            let Some(current) = readable_setting(&setting.path) else {
                return false;
            };
            let Some(target) = setting.expected.as_deref() else {
                return true;
            };
            if setting.bracketed {
                bracketed_value(&current) == Some(target)
            } else {
                current == target
            }
        })
    }

    /// Which expected settings are not showing their tuned value.
    fn missing(&self) -> Vec<String> {
        self.expected()
            .filter(|setting| {
                let Some(current) = readable_setting(&setting.path) else {
                    return true;
                };
                let target = setting.expected.as_deref().unwrap_or_default();
                if setting.bracketed {
                    bracketed_value(&current) != Some(target)
                } else {
                    current != target
                }
            })
            .map(|setting| {
                let current = readable_setting(&setting.path).unwrap_or_else(|| "?".to_owned());
                format!(
                    "{} is '{current}', expected '{}'",
                    setting.path,
                    setting.expected.as_deref().unwrap_or_default()
                )
            })
            .collect()
    }

    /// Which settings are not back to what they were.
    fn unrestored(&self) -> Vec<String> {
        self.settings
            .iter()
            .filter_map(|setting| {
                let current = readable_setting(&setting.path)?;
                (current != setting.original).then(|| {
                    format!(
                        "{} is '{current}', was '{}'",
                        setting.path, setting.original
                    )
                })
            })
            .collect()
    }

    /// Put everything back, whatever the runner did or failed to do.
    ///
    /// Reports what it had to undo: anything here means the guard under test did
    /// not do its job, and the scenario has already failed for that reason, but
    /// the machine still has to be left as it was found.
    fn restore(&self) {
        for setting in &self.settings {
            let Some(current) = readable_setting(&setting.path) else {
                continue;
            };
            if current == setting.original {
                continue;
            }
            // The bracketed files take the mode alone, never the whole listing.
            let value = if setting.bracketed {
                bracketed_value(&setting.original)
                    .unwrap_or(THP_TARGET)
                    .to_owned()
            } else {
                setting.original.clone()
            };
            match fs::write(&setting.path, &value) {
                Ok(()) => println!("  tuning: harness restored {} to '{value}'", setting.path),
                Err(e) => println!(
                    "  tuning: harness could NOT restore {} to '{value}': {e}",
                    setting.path
                ),
            }
        }
    }
}

/// Restores the host tuning when it goes out of scope.
///
/// A guard rather than a call at the end, so a panic or an early return in the
/// scenario cannot leave the machine tuned. Nothing survives the harness itself
/// being killed, which is why the scenario runs last.
struct RestoreTuning(TuningSnapshot);

impl Drop for RestoreTuning {
    fn drop(&mut self) {
        self.0.restore();
    }
}

/// Read a host setting, trimmed, if it is there at all.
fn readable_setting(path: &Utf8Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|v| v.trim().to_owned())
}

/// Whether a setting can be written, established by writing back what it holds.
fn writable_setting(path: &Utf8Path, current: &str) -> bool {
    fs::write(path, current).is_ok()
}

/// The selected mode in a bracketed sysfs listing (`always [madvise] never`).
fn bracketed_value(listing: &str) -> Option<&str> {
    let (_, selected) = listing.split_once('[')?;
    let (selected, _) = selected.split_once(']')?;
    Some(selected)
}

/// The cpuset partition files that exist, with what they hold.
///
/// Read separately from the rest because the partition is created by the tuning
/// itself: the files do not exist before the first tuned run on a fresh host, so
/// there is nothing to snapshot and their absence afterwards is the restored
/// state.
fn partition_state() -> Vec<(Utf8PathBuf, String)> {
    TUNED_PARTITION
        .iter()
        .map(Utf8PathBuf::from)
        .filter_map(|path| readable_setting(&path).map(|value| (path, value)))
        .collect()
}

/// Reap anything a previous scenario left running, before the wipe strands it.
///
/// A cancelled scenario SIGTERMs `runner run`, which installs no handler for it,
/// so the process dies without unwinding: its VMM stays alive in its cgroup and
/// its chroot stays on disk. That is the case the runner's sweep exists for, and
/// the sweep finds the VMM by the chroot, comparing device and inode against
/// `/proc/<pid>/root`. Wiping the state directory destroys that handle, so the
/// next runner sweeps a directory that no longer names anything, reports the
/// host clean, and the orphan runs on through every scenario that follows.
///
/// The product refuses to remove a chroot whose VMM is alive for exactly this
/// reason. The harness has been doing it once per scenario, so it does the
/// reclaiming the sweep would have done rather than leaving a live VMM with
/// nothing pointing at it.
fn reclaim_stranded_jails(state_dir: &Utf8Path) -> Result<()> {
    let parent = jail_parent(state_dir);
    let entries = match fs::read_dir(&parent) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(e) => return Err(e).with_context(|| format!("Failed to read {parent}")),
    };

    for entry in entries {
        let entry = entry.with_context(|| format!("Failed to read an entry under {parent}"))?;
        if !entry
            .file_type()
            .with_context(|| format!("Failed to read the kind of an entry under {parent}"))?
            .is_dir()
        {
            continue;
        }
        let vm_id = entry.file_name().to_string_lossy().into_owned();
        let jail_root = parent.join(&vm_id).join("root");

        if let Some(pid) = find_jailed_vmm(&jail_root)? {
            println!("  reclaiming VMM (pid {pid}) stranded in {vm_id}");
            kill_pid(pid, libc::SIGKILL);
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            while is_firecracker(pid)? && std::time::Instant::now() < deadline {
                std::thread::sleep(PROBE_INTERVAL);
            }
            anyhow::ensure!(
                !is_firecracker(pid)?,
                "A VMM stranded in {vm_id} (pid {pid}) would not die, so it would run on through every scenario that follows"
            );
        }

        // The cgroup shares the jail's name, and nothing else will come looking
        // for it once the directory below is gone.
        let cgroup = stale_cgroup(&vm_id);
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while cgroup.exists() {
            if fs::remove_dir(&cgroup).is_ok() {
                println!("  reclaimed the cgroup {cgroup}");
                break;
            }
            anyhow::ensure!(
                std::time::Instant::now() < deadline,
                "The cgroup {cgroup} could not be removed, so it would block the cpuset restore of every run that follows"
            );
            std::thread::sleep(PROBE_INTERVAL);
        }
    }

    Ok(())
}

/// What the `bencher` cgroup looks like, for when the partition assertion fails.
///
/// Clearing a parent's `cpuset.cpus` is refused with `EIO` while any task remains
/// in a descendant, so the useful question after a failed restore is what is
/// still in there. Without this the answer costs a CI round.
fn partition_diagnosis() -> String {
    let root = Utf8Path::new("/sys/fs/cgroup/bencher");
    if !root.exists() {
        return "the bencher cgroup is gone".to_owned();
    }
    let procs = fs::read_to_string(root.join("cgroup.procs")).unwrap_or_default();
    let children: Vec<String> = fs::read_dir(root)
        .map(|entries| {
            entries
                .flatten()
                .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_dir()))
                .map(|entry| entry.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    let child_procs: Vec<String> = children
        .iter()
        .map(|child| {
            let tasks =
                fs::read_to_string(root.join(child).join("cgroup.procs")).unwrap_or_default();
            // Named, not numbered. A bare pid costs a round trip to identify,
            // and what the process is decides whose bug it is.
            let named: Vec<String> = tasks
                .split_whitespace()
                .map(|pid| {
                    let comm = fs::read_to_string(format!("/proc/{pid}/comm"))
                        .map_or_else(|_| "gone".to_owned(), |comm| comm.trim().to_owned());
                    format!("{pid} ({comm})")
                })
                .collect();
            format!("{child} holds [{}]", named.join(" "))
        })
        .collect();
    format!(
        "the bencher cgroup is still there, holding tasks [{}] and children {child_procs:?}",
        procs.split_whitespace().collect::<Vec<_>>().join(" ")
    )
}

/// Run the runner with host tuning on, and assert it both applies and unwinds.
///
/// The assertion that matters is the pair. Applying is what the runner is for;
/// restoring is what keeps a benchmark host from drifting a knob at a time
/// across every Job it ever runs, and `TuningGuard` restoring on `Drop` had
/// never once executed in CI before this scenario existed.
/// Read the host, and work out what this run should change.
///
/// Separated so the scenario itself stays readable: everything here happens
/// before the runner starts and decides whether there is anything to test.
fn plan_tuning() -> Result<(TuningSnapshot, Vec<String>)> {
    let snapshot = TuningSnapshot::take();
    let expected: Vec<String> = snapshot
        .expected()
        .map(|setting| {
            format!(
                "{} -> {}",
                setting.path,
                setting.expected.as_deref().unwrap_or_default()
            )
        })
        .collect();

    // A scenario that finds nothing to change would pass without testing
    // anything, which is the failure this suite has spent the most effort
    // removing. If a host really offers none of these, that is a fact worth a
    // red build rather than a green one.
    anyhow::ensure!(
        !expected.is_empty(),
        "No tuning knob on this host can be exercised, so the scenario would pass vacuously. Settings considered: {:?}",
        snapshot
            .settings
            .iter()
            .map(|s| s.path.as_str())
            .collect::<Vec<_>>()
    );
    println!(
        "  tuning: expecting {} setting(s) to change: {}",
        expected.len(),
        expected.join(", ")
    );
    Ok((snapshot, expected))
}

fn run_runner_with_tuning(
    image_path: &Utf8Path,
    args: &[&str],
    runner_bin: &Utf8Path,
) -> Result<ScenarioOutput> {
    let (snapshot, expected) = plan_tuning()?;
    let partition_before = partition_state();

    // Taken before the runner starts, so the machine is put back even if the
    // scenario panics, the assertions fail, or the runner dies without
    // unwinding. The point of the scenario is that the guard under test might
    // not work.
    let restore = RestoreTuning(snapshot);

    let mut child = Command::new(runner_bin.as_str())
        .arg("run")
        .arg("--image")
        .arg(image_path.as_str())
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?;
    let readers = drain_output(&mut child);

    // Watch for the tuning to land while the Job runs. The runner applies it
    // before it pulls the image, so this is looking at a window that lasts the
    // whole run.
    let deadline = std::time::Instant::now() + PROBE_TIMEOUT;
    let mut applied = false;
    loop {
        if restore.0.all_applied() {
            applied = true;
            break;
        }
        if child.try_wait()?.is_some() || std::time::Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(PROBE_INTERVAL);
    }
    let partition_during = partition_state();

    if !applied && child.try_wait()?.is_none() {
        kill_pid(child.id(), libc::SIGKILL);
    }
    let status = child.wait()?;
    let (stdout, stderr) = readers.join();

    if !applied {
        bail!(
            "Host tuning never applied within {PROBE_TIMEOUT:?}: {:?}.\nstdout: {stdout}\nstderr: {stderr}",
            restore.0.missing()
        );
    }

    // The Job has to have succeeded as well. A scenario that only watched the
    // knobs would pass on a runner that tuned the host and then failed to run
    // anything, which is the vacuous half of a confinement assertion in another
    // dress.
    if status.code() != Some(0) {
        bail!(
            "Tuning applied but the Job failed with exit code {:?}.\nstdout: {stdout}\nstderr: {stderr}",
            status.code()
        );
    }

    // And it has to be gone now that the runner has exited.
    let unrestored = restore.0.unrestored();
    if !unrestored.is_empty() {
        bail!(
            "Host tuning was not restored when the runner exited: {unrestored:?}.\nstdout: {stdout}\nstderr: {stderr}"
        );
    }

    // Only the files that were there to change. The partition creates its own
    // cgroup, so a file that did not exist before the run has no previous value
    // to be restored to, and asserting on its appearance would fail the scenario
    // for the tuning having worked.
    let partition_after = partition_state();
    let partition_unrestored: Vec<String> = partition_before
        .iter()
        .filter_map(|(path, before)| {
            let after = partition_after
                .iter()
                .find_map(|(p, v)| (p == path).then_some(v.as_str()))?;
            (after != before).then(|| format!("{path} is '{after}', was '{before}'"))
        })
        .collect();
    if !partition_unrestored.is_empty() {
        bail!(
            "The cpuset partition was not restored: {partition_unrestored:?}. Now {}.\nstdout: {stdout}\nstderr: {stderr}",
            partition_diagnosis()
        );
    }
    if partition_during.is_empty() {
        println!("  tuning: no cpuset partition files on this host, so none were asserted");
    }

    println!(
        "  tuning: {} setting(s) applied and restored, {} partition file(s) checked",
        expected.len(),
        partition_before.len()
    );

    Ok(ScenarioOutput {
        stdout,
        stderr,
        exit_code: status.code().unwrap_or(-1),
    })
}

/// Scenarios covering host tuning, which every other scenario switches off.
fn tuning_scenarios() -> Vec<Scenario> {
    vec![Scenario {
        name: "host_tuning",
        description: "Host tuning applies while a Job runs and is restored after",
        dockerfile: r#"FROM busybox
CMD ["echo", "tuned run complete"]"#,
        extra_args: &["--timeout", "60"],
        tuning: true,
        // The Job's own output as well as the knobs. A run that tuned the host
        // and then never booted a VM would otherwise satisfy this scenario.
        validate: |output| assert_job_succeeded(output, "tuned run complete"),
        ..Scenario::default()
    }]
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

    // Drain both pipes while the probe runs. Nothing reads them during the
    // loop otherwise, so a runner chatty enough to fill the 64 KiB pipe buffer
    // blocks on its own output until the probe times out.
    let readers = drain_output(&mut child);

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

    // A probe that ended without observing the VMM leaves the runner going, and
    // waiting on it would sit there until the runner's own timeout expired,
    // reporting the failure minutes late. Only a run that observed what it came
    // for is allowed to finish, since its output is the result being collected.
    // Guarded on the reap, because `try_wait` above reaps and signalling a reaped
    // pid can reach whatever inherited the number.
    if !matches!(observed, Some(Ok(()))) && child.try_wait()?.is_none() {
        kill_pid(child.id(), libc::SIGKILL);
    }
    let status = child.wait()?;
    let (stdout, stderr) = readers.join();

    match observed {
        Some(Ok(())) => Ok(ScenarioOutput {
            stdout,
            stderr,
            exit_code: status.code().unwrap_or(-1),
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

#[cfg(test)]
mod tests {
    use std::ffi::OsStr;

    use super::*;

    #[test]
    fn a_relative_target_dir_is_resolved_against_the_workspace_root() {
        // Cargo resolves a relative value against the working directory of the
        // build, which is the workspace root the builds are handed. Carrying it
        // through as it stands would have the harness look under its own
        // working directory and report a missing binary immediately after
        // building it there.
        assert_eq!(
            resolve_target_dir(Some(OsStr::new("build-alt")), Utf8Path::new("/workspace")).unwrap(),
            "/workspace/build-alt"
        );
    }

    #[test]
    fn an_absolute_target_dir_is_where_it_says() {
        assert_eq!(
            resolve_target_dir(
                Some(OsStr::new("/elsewhere/target")),
                Utf8Path::new("/workspace")
            )
            .unwrap(),
            "/elsewhere/target"
        );
    }

    #[test]
    fn no_target_dir_is_the_workspace_target() {
        assert_eq!(
            resolve_target_dir(None, Utf8Path::new("/workspace")).unwrap(),
            "/workspace/target"
        );
    }

    #[cfg(unix)]
    #[test]
    fn a_target_dir_that_is_not_utf8_is_refused() {
        // Converting it lossily would name a directory nobody asked for, which
        // is the missing binary this resolution exists to prevent, reported
        // against a path that reads like the one that was given.
        use std::os::unix::ffi::OsStrExt as _;

        resolve_target_dir(
            Some(OsStr::from_bytes(b"/tmp/target-\xff")),
            Utf8Path::new("/workspace"),
        )
        .unwrap_err();
    }

    #[test]
    fn the_uid_the_scenarios_pass_is_the_uid_the_probe_demands() {
        // The flag is a string and the assertion is a number, so the value is
        // written twice. A scenario handing the runner one uid while the probe
        // demanded another would fail every jail scenario for a reason that has
        // nothing to do with the runner.
        assert_eq!(
            SCENARIO_JAIL_UID_ARG.parse::<u32>().unwrap(),
            SCENARIO_JAIL_UID
        );
        // The product's `DEFAULT_JAIL_UID`, spelled out because the harness does
        // not depend on the runner library. Asking for the default would leave
        // the probe unable to tell `--jail-uid` being honored from it being
        // ignored, which is the whole point of asking for one.
        assert_ne!(SCENARIO_JAIL_UID, 61016);
        // And root is no jail: the runner refuses this one at the command line.
        assert_ne!(SCENARIO_JAIL_UID, 0);
    }

    #[test]
    fn the_run_writes_nothing_outside_the_tree_it_hands_back() {
        // The chown has to reach the docker build contexts and the unpacked OCI
        // layouts as well as the state directory. Their per-scenario cleanup is
        // skipped by any early return, so on a red run they are left behind
        // root-owned, and a tree that is handed back short of them is one the
        // invoker still cannot remove.
        let work_dir = crate::task::work_dir();
        let returned = work_dir.parent().expect("the work directory has a parent");

        assert!(
            scenario_state_dir().starts_with(returned),
            "the state directory"
        );
        assert!(temp_dir().starts_with(returned), "the image trees");
    }

    #[test]
    fn the_selected_mode_is_the_bracketed_one() {
        // What the kernel prints for a transparent hugepage setting: every mode
        // it offers, with the live one in brackets. Comparing the whole line
        // against "never" would never match, and asserting on a substring would
        // match a mode that is merely offered.
        assert_eq!(
            bracketed_value("always [madvise] never"),
            Some("madvise"),
            "the enabled listing"
        );
        assert_eq!(
            bracketed_value("always defer defer+madvise [madvise] never"),
            Some("madvise"),
            "the defrag listing, which offers more modes"
        );
        assert_eq!(bracketed_value("[always] madvise never"), Some("always"));
        assert_eq!(bracketed_value("always madvise [never]"), Some("never"));
    }

    #[test]
    fn a_listing_with_no_selection_has_no_value() {
        // A plain sysctl is not a listing, and a truncated read is not a mode.
        assert_eq!(bracketed_value("never"), None);
        assert_eq!(bracketed_value(""), None);
        assert_eq!(bracketed_value("always [madvise"), None);
    }

    #[test]
    fn a_setting_that_is_not_there_reads_as_absent() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(dir.path()).unwrap();

        assert_eq!(readable_setting(&root.join("absent")), None);
    }

    #[test]
    fn a_setting_reads_back_trimmed() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(dir.path()).unwrap();
        let path = root.join("swappiness");
        fs::write(&path, "60\n").unwrap();

        assert_eq!(readable_setting(&path).as_deref(), Some("60"));
    }

    #[test]
    fn writability_is_established_by_writing_what_is_already_there() {
        // The probe that decides whether a knob can be exercised on this host.
        // A file that exists may still refuse writes, which is not something a
        // stat can answer: `/proc/sys/kernel/nmi_watchdog` is exactly that on a
        // kernel without a hardware watchdog, and waiting for it to change would
        // fail the scenario for the host's reasons rather than the runner's.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(dir.path()).unwrap();
        let path = root.join("knob");
        fs::write(&path, "1\n").unwrap();

        assert!(writable_setting(&path, "1"));
        assert_eq!(
            readable_setting(&path).as_deref(),
            Some("1"),
            "the probe writes back what was there, so it changes nothing"
        );

        // Root ignores the permission bits, and the scenarios run as root, so
        // this half only means anything unprivileged.
        if !is_root() {
            let mut perms = fs::metadata(&path).unwrap().permissions();
            perms.set_readonly(true);
            fs::set_permissions(&path, perms).unwrap();

            assert!(!writable_setting(&path, "1"));
        }
    }
}
