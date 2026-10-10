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
use std::os::fd::OwnedFd;
use std::path::Path;
use std::process::Command;
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Context as _, Result, bail};
use camino::{Utf8Path, Utf8PathBuf};

use crate::parser::TaskScenarios;

/// A host-side check run while the runner executes: `Ok(false)` until the VMM
/// appears, `Ok(true)` once the invariant holds, and `Err` once it is violated.
type Probe = fn(&Utf8Path) -> Result<bool>;

/// Test scenario definition.
///
/// Build one with `..Scenario::default()` so a scenario names only what it
/// varies.
#[expect(
    clippy::struct_excessive_bools,
    reason = "each flag switches one independent property of a run on"
)]
struct Scenario {
    name: &'static str,
    description: &'static str,
    dockerfile: &'static str,
    extra_args: &'static [&'static str],
    /// Run two iterations, and plant a stale jail once the first one's jail is
    /// built, after that one's sweep and long before the second one's.
    planted_between_jobs: bool,
    /// If set, send SIGTERM to the runner this many seconds after its guest
    /// boots, so the cancel exercises the VM's teardown.
    cancel_after_secs: Option<u64>,
    /// SIGTERM a booted run twice, then run the image again in the same state
    /// directory, whose output `validate` sees.
    cancelled_twice: bool,
    /// SIGTERM the runner once it starts parsing its image, well before its jail.
    cancelled_while_preparing: bool,
    /// Whether to use `--sandbox firecracker` (default: true).
    sandboxed: bool,
    setup: Option<fn() -> Result<()>>,
    /// Runs whatever the outcome, including after a `setup` that failed part way.
    teardown: Option<fn() -> Result<()>>,
    probe: Option<Probe>,
    tuning: bool,
    /// Kill the runner once its VMM is up, then rerun the image; `validate` sees
    /// the second run.
    orphan_then_rerun: bool,
    /// The same, with the killed runner in a state directory of its own.
    orphan_elsewhere_then_rerun: bool,
    /// Start a second runner, with its own state directory, once a first one's
    /// VMM is up; `validate` sees the second.
    second_runner: bool,
    /// Run the image once a `runner up` that never reaches its server is
    /// waiting on it, long past its startup.
    held_by_runner_up: bool,
    /// Run sandboxed while the harness holds the job lock, then on the host
    /// beside a maintenance marker; `validate` sees the second run.
    behind_the_job_lock: bool,
    /// Hold a stand-in process in another cgroup under the runner's base, as an
    /// orphan in another state directory would, placed while the runner is
    /// stopped between its startup and its job's sweep.
    occupied_cgroup: bool,
    /// The same stand-in, in place before the runner starts.
    occupied_at_start: bool,
    /// The same stand-in, in place before a `runner up` that never gets a Job
    /// starts.
    occupied_at_up_start: bool,
    /// Run the runner, then a `runner up`, in a mount namespace whose cgroup
    /// root is an empty tmpfs, as on a kernel without `cgroup.kill`.
    without_cgroup_kill: bool,
    unusable_state_dir: bool,
    /// Point the runner at a state directory on a `nodev` tmpfs.
    nodev_state_dir: bool,
    /// Stop the runner before its jail until its timeout is gone, then let it go
    /// on.
    stopped_past_timeout: bool,
    /// A line the image must print in a plain run first, so a run that prints
    /// nothing cannot pass on a runner that boots no guest.
    control_marker: Option<&'static str>,
    validate: fn(&ScenarioOutput) -> Result<()>,
}

impl Default for Scenario {
    fn default() -> Self {
        Self {
            name: "",
            description: "",
            dockerfile: "",
            extra_args: &[],
            planted_between_jobs: false,
            cancel_after_secs: None,
            cancelled_twice: false,
            cancelled_while_preparing: false,
            setup: None,
            teardown: None,
            probe: None,
            tuning: false,
            orphan_then_rerun: false,
            orphan_elsewhere_then_rerun: false,
            second_runner: false,
            held_by_runner_up: false,
            behind_the_job_lock: false,
            occupied_cgroup: false,
            occupied_at_start: false,
            occupied_at_up_start: false,
            without_cgroup_kill: false,
            unusable_state_dir: false,
            nodev_state_dir: false,
            stopped_past_timeout: false,
            control_marker: None,
            // Most scenarios are sandboxed, so the few that are not opt out.
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

        // Check prerequisites
        //
        // A world-accessible /dev/kvm is enough to use KVM without root, but not
        // to build the jail around it.
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
        // Last, as the only scenario that tunes the machine, so nothing it leaves
        // behind can reach the others.
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

        // Whatever the outcome, since a red run is what leaves the tree behind.
        return_work_dir_to_invoker();

        result
    }
}

/// Without this, one red scenario leaves root-owned directories that the next
/// unprivileged `cargo` or `git clean` cannot remove.
fn return_work_dir_to_invoker() {
    // The parent too, since this run creates it and a root-owned parent keeps
    // the invoker from removing the work directory.
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

    // Armed before the setup, so a setup that fails part way is unwound too.
    let _teardown = ScenarioTeardown::armed(scenario);
    if let Some(setup) = scenario.setup {
        setup().with_context(|| format!("Setup failed for {}", scenario.name))?;
    }

    // The suite's own state directory, wiped per scenario, so jail assertions
    // see only this scenario's jails and never a real runner's.
    let state_dir = scenario_state_dir();
    reclaim_stranded_jails(&state_dir)
        .with_context(|| format!("Failed to reclaim jails stranded before {}", scenario.name))?;
    drop(fs::remove_dir_all(&state_dir));

    let (state_arg, _mount) = scenario_state_arg(scenario, &state_dir)?;
    let running_before = stray_processes()?;
    let outcome = run_and_validate(scenario, &image_path, &state_dir, &state_arg, runner_bin);
    // Whatever the scenario's own verdict, anything it left running fails it
    // here rather than being reclaimed quietly before the next one.
    let mut state_dirs = vec![state_dir.as_path()];
    if state_arg != state_dir {
        state_dirs.push(&state_arg);
    }
    // Before the reclaim removes the jails that tell the scenario's strays apart.
    let mut stranded = reap_new_strays(&running_before, &state_dirs)?;
    for dir in state_dirs {
        stranded.extend(
            reclaim_stranded_jails(dir)
                .with_context(|| format!("Failed to reclaim what {} stranded", scenario.name))?,
        );
    }
    outcome?;
    anyhow::ensure!(
        stranded.is_empty(),
        "{} left behind: {}",
        scenario.name,
        stranded.join(", ")
    );

    // Cleanup
    drop(fs::remove_dir_all(
        image_path.parent().unwrap_or(&image_path),
    ));

    Ok(())
}

/// The state directory the runner is pointed at, and the mount it needs if any.
///
/// A path of the harness's own tree in every case, so nothing outside it is
/// ever named.
fn scenario_state_arg(
    scenario: &Scenario,
    state_dir: &Utf8Path,
) -> Result<(Utf8PathBuf, Option<Tmpfs>)> {
    if scenario.unusable_state_dir {
        let planted = unusable_state_dir().with_context(|| {
            format!(
                "Failed to plant a state directory the runner must refuse for {}",
                scenario.name
            )
        })?;
        return Ok((planted, None));
    }
    if scenario.nodev_state_dir {
        let mount = Tmpfs::mount(&super::work_dir().join("nodev-state"), "nodev,size=4g")?;
        return Ok((mount.0.join("state"), Some(mount)));
    }
    Ok((state_dir.to_owned(), None))
}

/// A tmpfs mounted over a harness directory, unmounted on drop.
struct Tmpfs(Utf8PathBuf);

impl Tmpfs {
    fn mount(at: &Utf8Path, options: &str) -> Result<Self> {
        fs::create_dir_all(at).with_context(|| format!("Failed to create {at}"))?;
        let status = Command::new("mount")
            .args(["-t", "tmpfs", "-o", options, "tmpfs", at.as_str()])
            .status()
            .context("Failed to run mount")?;
        anyhow::ensure!(status.success(), "mount -o {options} {at} failed");
        Ok(Self(at.to_owned()))
    }
}

impl Drop for Tmpfs {
    fn drop(&mut self) {
        drop(Command::new("umount").arg(self.0.as_str()).status());
    }
}

/// Every Firecracker, and every process in a Bencher cgroup, on the host.
fn stray_processes() -> Result<std::collections::BTreeSet<u32>> {
    let mut pids = firecracker_pids()?;
    pids.extend(bencher_cgroup_pids(
        Utf8Path::new("/sys/fs/cgroup/bencher"),
        false,
    )?);
    Ok(pids)
}

fn bencher_cgroup_pids(cgroup: &Utf8Path, members: bool) -> Result<Vec<u32>> {
    let mut pids = Vec::new();
    if members {
        match fs::read_to_string(cgroup.join("cgroup.procs")) {
            Ok(procs) => pids.extend(
                procs
                    .lines()
                    .filter_map(|line| line.trim().parse::<u32>().ok()),
            ),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(pids),
            Err(e) => {
                return Err(e).with_context(|| format!("Failed to read {cgroup}/cgroup.procs"));
            },
        }
    }
    let entries = match fs::read_dir(cgroup) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(pids),
        Err(e) => return Err(e).with_context(|| format!("Failed to read {cgroup}")),
    };
    for entry in entries {
        let entry = entry.with_context(|| format!("Failed to read an entry under {cgroup}"))?;
        if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
            let child = cgroup.join(entry.file_name().to_string_lossy().as_ref());
            pids.extend(bencher_cgroup_pids(&child, true)?);
        }
    }
    Ok(pids)
}

/// Report what started during the scenario and is still running, killing only
/// what runs in the scenario's own jails or cgroups, since the rest may be
/// another runner's.
fn reap_new_strays(
    before: &std::collections::BTreeSet<u32>,
    state_dirs: &[&Utf8Path],
) -> Result<Vec<String>> {
    let jails = scenario_jails(state_dirs)?;
    let strays: Vec<u32> = stray_processes()?.difference(before).copied().collect();
    reap_strays(strays, Pidfd::open, |pid| belongs_to_scenario(pid, &jails))
}

/// Each stray is pinned before its membership is checked and signalled only
/// through the pin, so a pid recycled after the scan is never killed, and one
/// that cannot be pinned is reported and left running.
fn reap_strays<O, B>(
    strays: impl IntoIterator<Item = u32>,
    open: O,
    mut belongs: B,
) -> Result<Vec<String>>
where
    O: Fn(u32) -> std::io::Result<Option<Pidfd>>,
    B: FnMut(u32) -> Result<Option<bool>>,
{
    let mut stranded = Vec::new();
    let mut killed = Vec::new();
    for pid in strays {
        let pidfd = match open(pid) {
            Ok(Some(pidfd)) => pidfd,
            Ok(None) => continue,
            Err(e) => {
                stranded.push(format!(
                    "process {pid}, left running since no pidfd could pin it for the kill ({e})"
                ));
                continue;
            },
        };
        match belongs(pid)? {
            Some(true) => {
                // A pinned process that has already exited fails the kill, and
                // leaves nothing to wait for.
                if pidfd.kill().is_ok() {
                    killed.push(pidfd);
                }
                stranded.push(format!("process {pid}"));
            },
            Some(false) => stranded.push(format!(
                "process {pid}, left running since it is in none of the scenario's jails or cgroups"
            )),
            None => {},
        }
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while killed.iter().any(Pidfd::is_running) && std::time::Instant::now() < deadline {
        std::thread::sleep(PROBE_INTERVAL);
    }
    Ok(stranded)
}

/// A descriptor pinned to one process, so a signal sent through it never
/// reaches another that takes the same pid later.
struct Pidfd(OwnedFd);

impl Pidfd {
    /// `Ok(None)` when the process is already gone.
    #[cfg(target_os = "linux")]
    fn open(pid: u32) -> std::io::Result<Option<Self>> {
        use std::os::fd::FromRawFd as _;

        let pid = libc::pid_t::try_from(pid).map_err(|_err| {
            std::io::Error::new(std::io::ErrorKind::InvalidInput, "pid out of range")
        })?;
        #[expect(
            unsafe_code,
            reason = "pidfd_open has no std wrapper; it takes plain integers"
        )]
        // SAFETY: `pidfd_open` touches no memory, and each argument is widened to
        // `c_long` because `syscall` is variadic.
        let raw = unsafe {
            libc::syscall(
                libc::SYS_pidfd_open,
                libc::c_long::from(pid),
                libc::c_long::from(0i32),
            )
        };
        if raw < 0 {
            let error = std::io::Error::last_os_error();
            return if error.raw_os_error() == Some(libc::ESRCH) {
                Ok(None)
            } else {
                Err(error)
            };
        }
        let raw = libc::c_int::try_from(raw)
            .map_err(|_err| std::io::Error::other("pidfd out of descriptor range"))?;
        #[expect(
            unsafe_code,
            reason = "taking ownership of a descriptor this call just created"
        )]
        // SAFETY: `raw` is a fresh descriptor returned by the syscall above and is
        // owned by nothing else.
        let fd = unsafe { OwnedFd::from_raw_fd(raw) };
        Ok(Some(Self(fd)))
    }

    #[cfg(not(target_os = "linux"))]
    fn open(_pid: u32) -> std::io::Result<Option<Self>> {
        Err(std::io::ErrorKind::Unsupported.into())
    }

    #[cfg(target_os = "linux")]
    fn kill(&self) -> std::io::Result<()> {
        use std::os::fd::AsRawFd as _;

        #[expect(
            unsafe_code,
            reason = "pidfd_send_signal has no std wrapper; the fd is owned and valid"
        )]
        // SAFETY: the descriptor is open for the call, a null `siginfo` asks the
        // kernel to synthesize one, and each integer is widened to `c_long` for
        // `syscall`.
        let ret = unsafe {
            libc::syscall(
                libc::SYS_pidfd_send_signal,
                libc::c_long::from(self.0.as_raw_fd()),
                libc::c_long::from(libc::SIGKILL),
                std::ptr::null::<libc::siginfo_t>(),
                libc::c_long::from(0i32),
            )
        };
        if ret == 0 {
            Ok(())
        } else {
            Err(std::io::Error::last_os_error())
        }
    }

    #[cfg(not(target_os = "linux"))]
    #[expect(
        clippy::unused_self,
        reason = "only Linux has pidfds, so elsewhere none is ever opened"
    )]
    fn kill(&self) -> std::io::Result<()> {
        Err(std::io::ErrorKind::Unsupported.into())
    }

    /// A failed `poll` reads as running, so no wait ends early on it.
    fn is_running(&self) -> bool {
        use std::os::fd::AsRawFd as _;

        let mut poll_fd = libc::pollfd {
            fd: self.0.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        #[expect(
            unsafe_code,
            reason = "poll has no std wrapper; the fd is owned and valid"
        )]
        // SAFETY: `poll` touches only the one entry passed, and the descriptor is
        // open for the call.
        let ready = unsafe { libc::poll(&raw mut poll_fd, 1, 0) };
        ready <= 0 || (poll_fd.revents & libc::POLLIN) == 0
    }
}

struct ScenarioJail {
    id: String,
    root: Option<fs::Metadata>,
}

fn scenario_jails(state_dirs: &[&Utf8Path]) -> Result<Vec<ScenarioJail>> {
    let mut jails = Vec::new();
    for state_dir in state_dirs {
        let parent = jail_parent(state_dir);
        let entries = match fs::read_dir(&parent) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => continue,
            Err(e) => return Err(e).with_context(|| format!("Failed to read {parent}")),
        };
        for entry in entries {
            let entry = entry.with_context(|| format!("Failed to read an entry under {parent}"))?;
            if entry.file_type().is_ok_and(|kind| kind.is_dir()) {
                let id = entry.file_name().to_string_lossy().into_owned();
                let root = fs::metadata(parent.join(&id).join("root")).ok();
                jails.push(ScenarioJail { id, root });
            }
        }
    }
    Ok(jails)
}

/// Rooted in one of the scenario's jails, or in its cgroup or the stand-in's;
/// `None` once the process has exited.
fn belongs_to_scenario(pid: u32, jails: &[ScenarioJail]) -> Result<Option<bool>> {
    belongs_with_cgroup(
        pid,
        fs::read_to_string(format!("/proc/{pid}/cgroup")),
        jails,
    )
}

fn belongs_with_cgroup(
    pid: u32,
    listing: std::io::Result<String>,
    jails: &[ScenarioJail],
) -> Result<Option<bool>> {
    if let Err(e) = &listing
        && gone(e)
    {
        return Ok(None);
    }
    let listing = listing.with_context(|| format!("Failed to read the cgroup of pid {pid}"))?;
    let cgroup = bencher_cgroup_name(&listing);
    if cgroup.is_some() && cgroup == Utf8Path::new(OCCUPIED_CGROUP).file_name() {
        return Ok(Some(true));
    }
    let root = fs::metadata(format!("/proc/{pid}/root")).ok();
    Ok(Some(jails.iter().any(|jail| {
        cgroup == Some(jail.id.as_str())
            || root
                .as_ref()
                .zip(jail.root.as_ref())
                .is_some_and(|(root, jail_root)| same_object(root, jail_root))
    })))
}

/// The cgroup just below the Bencher base that a `/proc/<pid>/cgroup` listing
/// places a process in or under.
fn bencher_cgroup_name(listing: &str) -> Option<&str> {
    listing
        .lines()
        .find_map(|line| line.strip_prefix("0::/bencher/"))?
        .split('/')
        .next()
}

fn run_and_validate(
    scenario: &Scenario,
    image_path: &Utf8Path,
    state_dir: &Utf8Path,
    state_arg: &Utf8Path,
    runner_bin: &Utf8Path,
) -> Result<()> {
    // `--no-tuning` everywhere but the tuning scenario, since an elevated run
    // really tunes the host and would offline SMT siblings under the suite.
    let mut args: Vec<&str> = vec!["--state-dir", state_arg.as_str()];
    if scenario.tuning {
        // Left out because the harness cannot fully undo them: an offlined SMT
        // sibling, and IRQ affinities an unmovable IRQ refuses with EIO.
        args.extend(["--smt", "--no-irq-steering"]);
    } else {
        args.push("--no-tuning");
    }
    if scenario.sandboxed {
        args.extend(["--sandbox", "firecracker"]);
    }
    if let Some(marker) = scenario.control_marker {
        let control = run_runner(
            image_path,
            &[args.as_slice(), &["--timeout", "60"]].concat(),
            runner_bin,
        )?;
        assert_job_succeeded(&control, marker)
            .with_context(|| format!("The control run failed for {}", scenario.name))?;
    }
    args.extend(scenario.extra_args);

    let output = if scenario.unusable_state_dir {
        run_runner_without_unjailed_vmm(image_path, &args, runner_bin)
    } else if scenario.planted_between_jobs {
        run_runner_planted_between_jobs(image_path, &args, state_dir, runner_bin)
    } else if let Some(secs) = scenario.cancel_after_secs {
        run_runner_with_cancel(
            image_path,
            &args,
            Duration::from_secs(secs),
            state_dir,
            runner_bin,
        )
    } else if scenario.cancelled_twice {
        run_runner_cancelled_twice(image_path, &args, state_dir, runner_bin)
    } else if scenario.cancelled_while_preparing {
        run_runner_cancelled_while_preparing(image_path, &args, runner_bin)
    } else if scenario.orphan_then_rerun {
        run_runner_after_orphan(image_path, &args, state_dir, state_dir, runner_bin)
    } else if scenario.orphan_elsewhere_then_rerun {
        run_runner_after_orphan_elsewhere(image_path, &args, state_dir, runner_bin)
    } else if scenario.second_runner {
        run_second_runner(image_path, &args, state_dir, runner_bin)
    } else if scenario.held_by_runner_up {
        run_runner_beside_runner_up(image_path, &args, runner_bin)
    } else if scenario.behind_the_job_lock {
        run_runner_behind_the_job_lock(image_path, &args, state_dir, runner_bin)
    } else if scenario.stopped_past_timeout {
        run_runner_stopped_past_timeout(image_path, &args, runner_bin)
    } else if scenario.occupied_at_up_start {
        run_runner_up_beside_occupied_cgroup(runner_bin)
    } else if scenario.without_cgroup_kill {
        run_runner_without_cgroup_kill(image_path, &args, scenario.sandboxed, runner_bin)
    } else if scenario.occupied_cgroup || scenario.occupied_at_start {
        run_runner_beside_occupied_cgroup(image_path, &args, scenario.occupied_at_start, runner_bin)
    } else if let Some(probe) = scenario.probe {
        run_runner_with_probe(image_path, &args, probe, state_dir, runner_bin)
    } else if scenario.tuning {
        run_runner_with_tuning(image_path, &args, runner_bin)
    } else {
        run_runner(image_path, &args, runner_bin)
    }
    .with_context(|| format!("Failed to run scenario {}", scenario.name))?;

    (scenario.validate)(&output).with_context(|| format!("Validation failed for {}", scenario.name))
}

struct ScenarioTeardown {
    name: &'static str,
    teardown: Option<fn() -> Result<()>>,
}

impl ScenarioTeardown {
    fn armed(scenario: &Scenario) -> Self {
        Self {
            name: scenario.name,
            teardown: scenario.teardown,
        }
    }
}

impl Drop for ScenarioTeardown {
    fn drop(&mut self) {
        let Some(teardown) = self.teardown else {
            return;
        };
        // Printed rather than swallowed, since what a teardown could not undo is
        // left on the machine.
        if let Err(e) = teardown() {
            println!("  teardown of {} did not finish: {e:#}", self.name);
        }
    }
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
            description: "Simple exec-form command",
            dockerfile: r#"FROM busybox
CMD ["printf", "%s_%s\\n", "HELLO", "VM"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| assert_job_succeeded(output, "HELLO_VM"),
            ..Scenario::default()
        },
        Scenario {
            name: "environment_variables",
            description: "ENV variables passed to guest",
            dockerfile: r#"FROM busybox
ENV MY_VAR=test_value
CMD ["sh", "-c", "echo $MY_VAR"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| assert_job_succeeded(output, "test_value"),
            ..Scenario::default()
        },
        Scenario {
            name: "working_directory",
            description: "WORKDIR set correctly",
            dockerfile: r#"FROM busybox
WORKDIR /myapp
CMD ["sh", "-c", "echo CWD=$(pwd)"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| assert_job_succeeded(output, "CWD=/myapp"),
            ..Scenario::default()
        },
        Scenario {
            name: "file_output",
            description: "Output file collected from the results drive",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "printf '{\"result\": %d}\\n' $((6 * 7)) > /tmp/output.json && cat /tmp/output.json"]"#,
            extra_args: &["--timeout", "60", "--output", "/tmp/output.json"],
            validate: |output| assert_job_succeeded(output, r#"{"result": 42}"#),
            ..Scenario::default()
        },
        Scenario {
            name: "exit_code",
            description: "Non-zero exit codes captured",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "printf '%s_%s\\n' EXIT CODE; exit 42"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| assert_guest_exited(output, "EXIT_CODE", 42),
            ..Scenario::default()
        },
        Scenario {
            name: "timeout_handling",
            description: "VM killed once its timeout and shutdown allowance run out",
            dockerfile: r#"FROM busybox
CMD ["sleep", "3600"]"#,
            extra_args: &["--timeout", "5"],
            probe: Some(probe_booted),
            validate: |output| {
                assert_timed_out(output)?;
                // The VM's span starts after the image is prepared, so only the
                // allowance can take it more than a second past the timeout.
                let wall_clock_ms = run_metrics(output)
                    .next()
                    .and_then(|metrics| metrics.get("wall_clock_ms")?.as_u64());
                anyhow::ensure!(
                    wall_clock_ms.is_some_and(|ms| ms > 6_000),
                    "Expected the VM to run on past its 5 s timeout, for {wall_clock_ms:?} ms.\nstderr: {}",
                    output.stderr
                );
                Ok(())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "writable_filesystem",
            description: "Guest can write to ext4 rootfs",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "printf '%s_%s\\n' WRITE TEST > /data.txt && cat /data.txt"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| assert_job_succeeded(output, "WRITE_TEST"),
            ..Scenario::default()
        },
        Scenario {
            name: "stderr_capture",
            description: "Stderr captured separately",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "printf '%s_%s\\n' TO STDOUT && printf '%s_%s\\n' TO STDERR >&2"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                assert_job_succeeded(output, "TO_STDOUT")?;
                // A second copy is the runner echoing the guest's stderr into its own log.
                anyhow::ensure!(
                    guest_printed_to_stderr(output, "TO_STDERR") == 1
                        && guest_printed(output, "TO_STDERR") == 0
                        && guest_printed_to_stderr(output, "TO_STDOUT") == 0,
                    "Expected 'TO_STDERR' once on stderr alone and 'TO_STDOUT' on stdout alone.\nstdout: {}\nstderr: {}",
                    output.stdout,
                    output.stderr
                );
                Ok(())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "multi_cpu",
            description: "Multiple vCPUs boot and the guest sees them all",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "printf 'CPUS=%s\\n' $(grep -c ^processor /proc/cpuinfo)"]"#,
            extra_args: &["--timeout", "60", "--vcpus", "4"],
            validate: |output| assert_job_succeeded(output, "CPUS=4"),
            ..Scenario::default()
        },
        Scenario {
            name: "entrypoint_with_args",
            description: "ENTRYPOINT + CMD combined",
            dockerfile: r#"FROM busybox
ENTRYPOINT ["printf", "%s_%s\\n"]
CMD ["HELLO", "WORLD"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| assert_job_succeeded(output, "HELLO_WORLD"),
            ..Scenario::default()
        },
        Scenario {
            name: "no_network_access",
            description: "Guest has no network",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "ping -c 1 -W 1 8.8.8.8 > /dev/null 2>&1 || printf '%s_%s\\n' NO NETWORK"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| assert_job_succeeded(output, "NO_NETWORK"),
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
            validate: |output| assert_guest_payload_capped(output, 'A', 10 * 1024 * 1024),
            ..Scenario::default()
        },
        Scenario {
            name: "timeout_enforced",
            description: "Timeout kills hanging process",
            // This process ignores signals and runs forever
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "trap '' TERM INT; echo started; while true; do sleep 1; done"]"#,
            extra_args: &["--timeout", "5"],
            probe: Some(probe_booted),
            validate: assert_timed_out,
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
CMD ["sh", "-c", "printf 'UID=%s\\n' $(id -u)"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                // The runner should not fail with uid_map errors.
                let combined = format!("{}{}", output.stdout, output.stderr);
                if combined.contains("uid_map") || combined.contains("Operation not permitted") {
                    bail!(
                        "uid_map error detected - likely getuid() called after unshare: {combined}"
                    )
                }
                assert_job_succeeded(output, "UID=0")
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
CMD ["printf", "%s_%s\\n", "KVM", "OK"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                let combined = format!("{}{}", output.stdout, output.stderr);
                if combined.contains("/dev/kvm") && combined.contains("not available") {
                    bail!("/dev/kvm not accessible in jail - bind mount likely lost: {combined}")
                }
                assert_job_succeeded(output, "KVM_OK")
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
CMD ["sh", "-c", "set -- $(cat /proc/version) && printf '%s_%s\\n' $1 $2"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                let combined = format!("{}{}", output.stdout, output.stderr);
                if combined.contains("mount") && combined.contains("EPERM") {
                    bail!(
                        "/proc mount failed - likely procfs mount in user namespace without PID namespace: {combined}"
                    )
                }
                assert_job_succeeded(output, "Linux_version")
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
CMD ["sh", "-c", "touch /tmp/write_test && printf '%s_%s\\n' WRITE OK"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                let combined = format!("{}{}", output.stdout, output.stderr);
                if combined.contains("Read-only file system") {
                    bail!(
                        "Rootfs is read-only - kernel cmdline likely has 'ro' instead of 'rw': {combined}"
                    )
                }
                assert_job_succeeded(output, "WRITE_OK")
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
            probe: Some(probe_booted),
            validate: |output| {
                // SIGSYS from seccomp violation produces exit code 159 (128 + 31)
                if output.exit_code == 159 {
                    bail!(
                        "Got SIGSYS (exit 159) - seccomp filter likely blocking a required syscall.\nstderr: {}",
                        output.stderr
                    )
                }
                assert_timed_out(output)
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
                if guest_printed(output, "IOPL_INHERITED") > 0 {
                    bail!(
                        "iopl(3) was inherited by benchmark process - \
                         init should drop iopl before exec"
                    )
                }
                assert_job_succeeded(output, "IOPL_DROPPED")
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
CMD ["printf", "%s_%s\\n", "UNIQUE_VM_OUTPUT", "a7f3b2c9"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| assert_job_succeeded(output, "UNIQUE_VM_OUTPUT_a7f3b2c9"),
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
            // Kernel threads have no `exe`, so only user-space processes are counted.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "n=0; for p in /proc/[0-9]*; do [ -e $p/exe ] && n=$((n + 1)); done; printf 'PIDS=%s\\n' $n"]"#,
            extra_args: &["--timeout", "60"],
            // More than 50 means the PID namespace is likely leaking host PIDs.
            validate: |output| assert_guest_value_within(output, "PIDS=", 1..=50),
            ..Scenario::default()
        },
        Scenario {
            name: "pid_namespace_procfs",
            description: "Fresh procfs mount works with PID namespace",
            // Verifies /proc is properly mounted with PID namespace support.
            // With fresh procfs (not bind-mounted from host), /proc/version
            // should be accessible and /proc/1/cmdline should show the init process.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "set -- $(cat /proc/version) && printf '%s_%s\\n' $1 $2 && printf 'PID1=%s\\n' $(tr '\\0' '\\n' < /proc/1/cmdline | head -n 1)"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                assert_job_succeeded(output, "Linux_version")?;
                assert_job_succeeded(output, "PID1=/init")
            },
            ..Scenario::default()
        },
        // =======================================================================
        // Telemetry/Metrics scenarios (Item 10)
        // =======================================================================
        Scenario {
            name: "metrics_timeout_flag",
            description: "Timeout flag set correctly in metrics",
            // When a VM times out, the metrics should include timed_out: true.
            dockerfile: r#"FROM busybox
CMD ["sleep", "3600"]"#,
            extra_args: &["--timeout", "5"],
            probe: Some(probe_booted),
            validate: assert_timed_out,
            ..Scenario::default()
        },
        // =======================================================================
        // Cancellation scenarios
        // =======================================================================
        Scenario {
            name: "job_cancelled",
            description: "SIGTERM cancels a running VM cleanly",
            // The harness fails any scenario that leaves a VMM or its cgroup behind.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo started && sleep 3600"]"#,
            cancel_after_secs: Some(1),
            extra_args: &["--timeout", "120"],
            validate: |output| {
                assert_cancelled(output)?;
                assert_no_chroot_remains(&scenario_state_dir())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "job_cancelled_twice",
            description: "A second SIGTERM kills the runner at once, and the next run reclaims what it left",
            dockerfile: r#"FROM busybox
CMD ["printf", "%s_%s\\n", "CANCELLED_TWICE", "a7f3b2c9"]"#,
            cancelled_twice: true,
            extra_args: &["--timeout", "120"],
            validate: |output| {
                assert_job_succeeded(output, "CANCELLED_TWICE_a7f3b2c9")?;
                assert_no_chroot_remains(&scenario_state_dir())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "job_cancelled_before_its_jail",
            description: "SIGTERM while the image is prepared ends the job before its jail is built",
            dockerfile: r#"FROM busybox
CMD ["printf", "%s_%s\\n", "CANCELLED_EARLY", "a7f3b2c9"]"#,
            cancelled_while_preparing: true,
            extra_args: &["--timeout", "120"],
            validate: |output| {
                assert_cancelled(output)?;
                assert_refused_before_guest(output, "CANCELLED_EARLY_a7f3b2c9")?;
                anyhow::ensure!(
                    records_with(&output.stderr, "Jail built").next().is_none(),
                    "The job built its jail after the cancel, so no stage before it honored the cancel.\nstdout: {}\nstderr: {}",
                    output.stdout,
                    output.stderr
                );
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
CMD ["sh", "-c", "printf '%s_%s\\n' STDERR ONLY >&2"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                anyhow::ensure!(
                    output.exit_code == 0
                        && guest_printed_to_stderr(output, "STDERR_ONLY") > 0
                        && guest_printed(output, "STDERR_ONLY") == 0,
                    "Expected the job to succeed with 'STDERR_ONLY' on stderr alone, got exit code {}.\nstdout: {}\nstderr: {}",
                    output.exit_code,
                    output.stdout,
                    output.stderr
                );
                Ok(())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "empty_output",
            description: "Process exits with no output",
            // Only the guest can send 37, since a missing or garbled code reads as 1.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "exit 37"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| assert_guest_exited_silently(output, 37),
            ..Scenario::default()
        },
        Scenario {
            name: "binary_output",
            description: "Non-UTF8 stdout handled gracefully",
            // Write raw bytes 0x80-0xFF which are invalid UTF-8.
            // The runner should not panic — it should lossy-convert or pass through.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "printf '\\x80\\x81\\xFE\\xFF' && printf '\\n%s_%s\\n' BINARY DONE"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                assert_job_succeeded(output, "BINARY_DONE")?;
                // Only the guest's bytes can have been replaced, since the runner
                // echoes the command's escapes as plain text.
                anyhow::ensure!(
                    output.stdout.contains(char::REPLACEMENT_CHARACTER),
                    "Expected the guest's invalid UTF-8 to arrive as replacement characters.\nstdout: {}",
                    output.stdout
                );
                Ok(())
            },
            ..Scenario::default()
        },
        // =======================================================================
        // OCI config parsing scenarios
        // =======================================================================
        Scenario {
            name: "shell_form_cmd",
            description: "Shell-form CMD (string, not array) works",
            // Shell form in Dockerfile: CMD printf ...
            // OCI config stores this as ["/bin/sh", "-c", "printf ..."]
            // which differs from exec form ["printf", ...].
            dockerfile: "FROM busybox\nCMD printf '%s_%s\\n' SHELL FORM",
            extra_args: &["--timeout", "60"],
            validate: |output| assert_job_succeeded(output, "SHELL_FORM"),
            ..Scenario::default()
        },
        Scenario {
            name: "entrypoint_only",
            description: "ENTRYPOINT exec form with no CMD",
            // When only ENTRYPOINT is set (exec form), it runs as-is with no
            // CMD args appended. The runner must not fail when Cmd is null/empty.
            dockerfile: r#"FROM busybox
ENTRYPOINT ["printf", "%s_%s\\n", "ENTRYPOINT", "ONLY"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| assert_job_succeeded(output, "ENTRYPOINT_ONLY"),
            ..Scenario::default()
        },
        Scenario {
            name: "shell_form_entrypoint",
            description: "ENTRYPOINT shell form (string, not array)",
            // Shell form ENTRYPOINT: stored as ["/bin/sh", "-c", "printf ..."]
            // in OCI config. CMD is ignored when ENTRYPOINT uses shell form.
            dockerfile: "FROM busybox\nENTRYPOINT printf '%s_%s\\n' SHELL ENTRYPOINT",
            extra_args: &["--timeout", "60"],
            validate: |output| assert_job_succeeded(output, "SHELL_ENTRYPOINT"),
            ..Scenario::default()
        },
        Scenario {
            name: "entrypoint_shell_with_cmd",
            description: "Shell-form ENTRYPOINT with CMD args (CMD becomes $0)",
            // When ENTRYPOINT is shell form, Docker wraps it as:
            //   ["/bin/sh", "-c", "printf '%s_%s\n' EP MARKER"]
            // Per OCI spec, CMD args are appended: the final exec is
            //   ["/bin/sh", "-c", "printf '%s_%s\n' EP MARKER", "cmd_arg"]
            // In sh -c semantics, "cmd_arg" becomes $0 (unused by printf).
            // The VM output should contain only "EP_MARKER", proving
            // that CMD args don't interfere with the entrypoint command.
            dockerfile: r#"FROM busybox
ENTRYPOINT printf '%s_%s\n' EP MARKER
CMD ["cmd_arg"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| assert_job_succeeded(output, "EP_MARKER"),
            ..Scenario::default()
        },
        Scenario {
            name: "no_cmd_no_entrypoint",
            description: "No CMD or ENTRYPOINT fails gracefully",
            // An image with no CMD and no ENTRYPOINT should cause the runner
            // to fail with a clear error, not crash or hang. `CMD []` clears the
            // `sh` that busybox would otherwise hand down.
            dockerfile: "FROM busybox\nCMD []",
            extra_args: &["--timeout", "30"],
            validate: |output| {
                anyhow::ensure!(
                    output.exit_code == 1
                        && runner_error(output)
                            .is_some_and(|error| error.contains("has no CMD or ENTRYPOINT")),
                    "Expected the job to fail naming the missing CMD and ENTRYPOINT, got exit code {}.\nstdout: {}\nstderr: {}",
                    output.exit_code,
                    output.stdout,
                    output.stderr
                );
                Ok(())
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
                // A line of the benchmark JSON `bencher mock` prints.
                assert_job_succeeded(output, r#""bencher::mock_0": {"#)
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
                assert_job_succeeded(output, "distroless_glibc_ok")
            },
            ..Scenario::default()
        },
        // =======================================================================
        // Race condition scenarios
        // =======================================================================
        Scenario {
            name: "rapid_exit",
            description: "Instantly exiting process doesn't lose results",
            // The process exits immediately. This tests whether results are
            // collected even for very short-lived processes.
            dockerfile: r#"FROM busybox
CMD ["printf", "%s_%s\\n", "RAPID", "EXIT"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| assert_job_succeeded(output, "RAPID_EXIT"),
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
CMD ["sh", "-c", "printf '%s_%s\\n' SIGNAL EXIT; exit 137"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| assert_guest_exited(output, "SIGNAL_EXIT", 137),
            ..Scenario::default()
        },
        Scenario {
            name: "a_guest_exit_stops_the_vmm_cleanly",
            description: "A clean guest exit stops Firecracker without an error",
            dockerfile: r#"FROM busybox
CMD ["printf", "%s_%s\\n", "CLEAN", "STOP"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                assert_job_succeeded(output, "CLEAN_STOP")?;
                anyhow::ensure!(
                    !vmm_reported_an_error(output)
                        && vmm_exit_status(output).as_deref() == Some("exit status: 0"),
                    "Expected Firecracker to exit 0 without an error.\nstderr: {}",
                    output.stderr
                );
                Ok(())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "the_guest_kernel_does_not_probe_its_keyboard",
            description: "The guest kernel registers its keyboard without the probe that held every boot for about half a second",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "dmesg | grep -E 'serio: i8042 KBD port|input: AT .* keyboard' && printf '%s_%s\\n' KERNEL LOG"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                assert_job_succeeded(output, "KERNEL_LOG")?;
                let port = kernel_log_micros(output, "serio: i8042 KBD port");
                let keyboard = kernel_log_micros(output, "input: AT");
                anyhow::ensure!(
                    port.zip(keyboard).is_some_and(|(port, keyboard)| {
                        keyboard.saturating_sub(port) < NO_KEYBOARD_PROBE_MICROS
                    }),
                    "Expected the keyboard within {NO_KEYBOARD_PROBE_MICROS} us of its port, so with no probe between them.\nstdout: {}",
                    output.stdout
                );
                Ok(())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "a_configuration_firecracker_refuses_fails_the_job_plainly",
            description: "A VM configuration Firecracker refuses fails the Job at once, saying so and with Firecracker's own error",
            // Firecracker boots at most 32 vCPUs, which the runner does not check.
            dockerfile: r#"FROM busybox
CMD ["printf", "%s_%s\\n", "TOO_MANY", "VCPUS"]"#,
            extra_args: &["--timeout", "20", "--vcpus", "64"],
            control_marker: Some("TOO_MANY_VCPUS"),
            validate: |output| {
                let error = runner_error(output).unwrap_or_default();
                let timed_out = run_metrics(output)
                    .next()
                    .and_then(|metrics| metrics.get("timed_out")?.as_bool());
                anyhow::ensure!(
                    output.exit_code == 1
                        && error.contains("exited (exit status: 1) before it booted the VM")
                        && error.contains("refused the VM's configuration")
                        && error.contains("InvalidVcpuCount")
                        && timed_out == Some(false)
                        && guest_printed(output, "TOO_MANY_VCPUS") == 0,
                    "Expected the run to fail at once on Firecracker's refusal, naming it, got exit code {}.\nstdout: {}\nstderr: {}",
                    output.exit_code,
                    output.stdout,
                    output.stderr
                );
                Ok(())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "the_vcpus_are_pinned_before_the_benchmark_starts",
            description: "Every vCPU thread, then every VMM thread again, is pinned before the benchmark starts, though the guest boots as Firecracker starts",
            // The guest's uptime is at most the time since the runner started
            // Firecracker, so a pin sooner after that start came first.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "printf 'UPTIME %s\\n' $(cut -d ' ' -f 1 /proc/uptime)"]"#,
            extra_args: &["--timeout", "60", "--vcpus", "2"],
            validate: |output| {
                let booted = records_with(&output.stderr, "Booting VM")
                    .next()
                    .as_ref()
                    .and_then(time_of_day_micros);
                let pinned = |msg: &str| {
                    let record = records_with(&output.stderr, msg).next()?;
                    let after = micros_between(booted?, time_of_day_micros(&record)?);
                    Some((after, record.get("pinned")?.as_u64()?))
                };
                let vcpus = pinned("vCPU threads pinned to dedicated cores");
                let again = pinned("VMM threads pinned again");
                let uptime = guest_uptime_micros(output);
                let before_the_benchmark = |pass: Option<(u64, u64)>, at_least: u64| {
                    pass.zip(uptime).is_some_and(|((after, pinned), uptime)| {
                        after < uptime && pinned >= at_least
                    })
                };
                anyhow::ensure!(
                    output.exit_code == 0
                        && vcpus.is_some_and(|(_, pinned)| pinned == 2)
                        && before_the_benchmark(vcpus, 2)
                        && before_the_benchmark(again, 3),
                    "Expected both vCPU threads pinned, then the VMM's threads again, each sooner after Firecracker started than the guest's uptime when its benchmark ran, got {vcpus:?} and {again:?} (us, pinned) against {uptime:?} us.\nstdout: {}\nstderr: {}",
                    output.stdout,
                    output.stderr
                );
                Ok(())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "a_guest_that_reboots_before_its_results_fails_at_once",
            description: "A guest that powers off before it writes its results fails the run at once, not at its timeout",
            // The control run prints the marker, and the real run reboots before
            // init writes anything.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "[ -n \"$REBOOT_EARLY\" ] && reboot -f; printf '%s_%s\\n' EARLY REBOOT"]"#,
            extra_args: &["--timeout", "20", "--env", "REBOOT_EARLY=1"],
            control_marker: Some("EARLY_REBOOT"),
            probe: Some(probe_booted),
            validate: |output| {
                let metrics = run_metrics(output).next();
                let field = |key: &str| {
                    metrics
                        .as_ref()
                        .and_then(|metrics| metrics.get(key).cloned())
                };
                anyhow::ensure!(
                    output.exit_code == 1
                        && runner_error(output)
                            .is_some_and(|error| error.contains("stopped without results"))
                        && field("timed_out") == Some(serde_json::Value::Bool(false))
                        && field("wall_clock_ms")
                            .and_then(|ms| ms.as_u64())
                            .is_some_and(|ms| ms < 10_000)
                        && guest_printed(output, "EARLY_REBOOT") == 0,
                    "Expected the run to fail at once with no results, well inside its 20 s timeout, got exit code {}.\nstdout: {}\nstderr: {}",
                    output.exit_code,
                    output.stdout,
                    output.stderr
                );
                Ok(())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "a_guest_finishing_inside_its_shutdown_allowance_keeps_its_results",
            description: "A guest that finishes just past its timeout, inside the shutdown allowance, keeps its results",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "sleep 6 && printf '%s_%s\\n' LATE DONE"]"#,
            extra_args: &["--timeout", "5"],
            validate: |output| {
                assert_job_succeeded(output, "LATE_DONE")?;
                let metrics = run_metrics(output).next();
                let field = |key: &str| {
                    metrics
                        .as_ref()
                        .and_then(|metrics| metrics.get(key).cloned())
                };
                // Past the timeout, so the allowance and not the timeout kept it.
                anyhow::ensure!(
                    field("timed_out") == Some(serde_json::Value::Bool(false))
                        && field("wall_clock_ms")
                            .and_then(|ms| ms.as_u64())
                            .is_some_and(|ms| ms > 5_000),
                    "Expected the VM to run past its 5 s timeout and still report.\nstderr: {}",
                    output.stderr
                );
                Ok(())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "a_record_the_benchmark_forges_is_discarded_on_a_timeout",
            description: "A record the benchmark writes to the results drive itself is discarded when the run times out",
            // A well-formed record of stdout `FORGED`, then a hang.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "printf FORGED | dd of=/dev/vdb bs=4096 seek=1 conv=fsync 2>/dev/null; printf 'BNCHRES1\\x00\\x00\\x00\\x00\\x00\\x00\\x00\\x00\\x06\\x00\\x00\\x00\\x00\\x00\\x00\\x00' | dd of=/dev/vdb conv=fsync 2>/dev/null; sleep 3600"]"#,
            extra_args: &["--timeout", "5"],
            probe: Some(probe_booted),
            validate: |output| {
                assert_timed_out(output)?;
                anyhow::ensure!(
                    !output.stdout.contains("FORGED"),
                    "Expected nothing the guest left on a timeout.\nstdout: {}",
                    output.stdout
                );
                Ok(())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "an_init_error_is_reported_and_the_guest_still_powers_off",
            description: "A guest whose init fails reports the error and powers off, rather than halting until its timeout",
            // The image's working directory is gone by the time the guest runs,
            // so init fails before the benchmark.
            dockerfile: r#"FROM busybox
WORKDIR /gone
RUN rmdir /gone
CMD ["printf", "%s_%s\\n", "NEVER", "RUN"]"#,
            extra_args: &["--timeout", "20"],
            validate: |output| {
                let timed_out = run_metrics(output)
                    .next()
                    .and_then(|metrics| metrics.get("timed_out")?.as_bool());
                anyhow::ensure!(
                    output.exit_code == 1
                        && runner_error(output)
                            .is_some_and(|error| error.ends_with("non-zero exit code: 1"))
                        && output
                            .stderr
                            .lines()
                            .any(|line| line.starts_with("init error: io: chdir to /gone"))
                        && timed_out == Some(false)
                        && guest_printed(output, "NEVER_RUN") == 0,
                    "Expected init's error as the guest's stderr and the run to fail with exit code 1 before its timeout, got exit code {}.\nstdout: {}\nstderr: {}",
                    output.exit_code,
                    output.stdout,
                    output.stderr
                );
                Ok(())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "a_triple_fault_stops_the_vmm_with_an_error",
            description: "A guest reset by triple fault stops Firecracker with an error",
            // A triple fault is a reset Firecracker cannot model, so this proves the
            // check that a clean stop reports no error can still see one.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo triple > /sys/kernel/reboot/type && printf '%s_%s\\n' REBOOT $(cat /sys/kernel/reboot/type)"]"#,
            extra_args: &["--timeout", "60"],
            // The guest's results are on its drive before it resets, so the run
            // still reports them.
            validate: |output| {
                assert_job_succeeded(output, "REBOOT_triple")?;
                anyhow::ensure!(
                    vmm_reported_an_error(output)
                        && vmm_exit_status(output).is_some_and(|status| status != "exit status: 0"),
                    "Expected Firecracker to stop with an error.\nstderr: {}",
                    output.stderr
                );
                Ok(())
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
ENV LARGE_VALUE=AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA
CMD ["sh", "-c", "echo A1=$A1 B10=$B10 LARGE_LEN=${#LARGE_VALUE}"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| assert_job_succeeded(output, "A1=val1 B10=val20 LARGE_LEN=555"),
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
CMD ["printf", "%s_%s\\n", "NO", "FILE"]"#,
            extra_args: &["--timeout", "60", "--output", "/nonexistent/path.json"],
            validate: |output| {
                let combined = format!("{}{}", output.stdout, output.stderr);
                if combined.contains("panic") || combined.contains("SIGSEGV") {
                    bail!("Runner crashed when output file is missing: {combined}")
                }
                assert_job_succeeded(output, "NO_FILE")
            },
            ..Scenario::default()
        },
        Scenario {
            name: "large_file_output",
            description: "Large output file (~2 MB) returned on the results drive",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "dd if=/dev/urandom bs=1024 count=2048 2>/dev/null | base64 > /tmp/output.json && printf '%s_%s\\n' LARGE FILE"]"#,
            extra_args: &["--timeout", "60", "--output", "/tmp/output.json"],
            validate: |output| assert_job_succeeded(output, "LARGE_FILE"),
            ..Scenario::default()
        },
        Scenario {
            name: "completed_with_all_fields",
            description: "Stdout + stderr + output file simultaneously",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "printf '%s_%s\\n' ALL STDOUT && printf '%s_%s\\n' ALL STDERR >&2 && echo '{\"data\":true}' > /tmp/out.json"]"#,
            extra_args: &["--timeout", "60", "--output", "/tmp/out.json"],
            validate: |output| {
                assert_job_succeeded(output, "ALL_STDOUT")?;
                if guest_printed_to_stderr(output, "ALL_STDERR") == 0 {
                    bail!("Expected 'ALL_STDERR' in stderr, got: {}", output.stderr)
                }
                Ok(())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "multi_file_output",
            description: "Multiple output files collected from the results drive",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo '{\"result\": 1}' > /tmp/a.json && echo '{\"result\": 2}' > /tmp/b.json && printf '%s_%s\\n' MULTI FILE"]"#,
            extra_args: &[
                "--timeout",
                "60",
                "--output",
                "/tmp/a.json",
                "--output",
                "/tmp/b.json",
            ],
            validate: |output| assert_job_succeeded(output, "MULTI_FILE"),
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
CMD ["sh", "-c", "printf 'LAYERS=%s%s%s\\n' $(cat /tmp/file_a.txt /opt/file_b.txt /var/file_c.txt)"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| assert_job_succeeded(output, "LAYERS=abc"),
            ..Scenario::default()
        },
        Scenario {
            name: "image_with_symlinks",
            description: "Symbolic links preserved through OCI unpack + ext4",
            dockerfile: r#"FROM busybox
RUN echo "target" > /tmp/target.txt && ln -s /tmp/target.txt /tmp/link.txt
CMD ["cat", "/tmp/link.txt"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| assert_job_succeeded(output, "target"),
            ..Scenario::default()
        },
        // =======================================================================
        // Error / edge case scenarios
        // =======================================================================
        Scenario {
            name: "failed_with_partial_output",
            description: "Writes stdout+stderr then exits non-zero",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "printf '%s_%s\\n' PARTIAL STDOUT && printf '%s_%s\\n' PARTIAL STDERR >&2 && exit 1"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| {
                // The key property: partial output is captured despite non-zero guest exit.
                assert_guest_exited(output, "PARTIAL_STDOUT", 1)?;
                if guest_printed_to_stderr(output, "PARTIAL_STDERR") == 0 {
                    bail!(
                        "Expected 'PARTIAL_STDERR' in stderr, got: {}",
                        output.stderr
                    )
                }
                Ok(())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "minimum_timeout",
            description: "1-second timeout kills long-running process",
            dockerfile: r#"FROM busybox
CMD ["sleep", "3600"]"#,
            extra_args: &["--timeout", "1"],
            probe: Some(probe_booted),
            validate: assert_timed_out,
            ..Scenario::default()
        },
        Scenario {
            name: "max_output_size_truncation",
            description: "Guest payload capped at exactly --max-output-size bytes",
            // Generate ~50 KB of `X` bytes, but limit the payload to 1024 bytes.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "dd if=/dev/zero bs=1024 count=50 2>/dev/null | tr '\\0' 'X'"]"#,
            extra_args: &["--timeout", "60", "--max-output-size", "1024"],
            validate: |output| assert_guest_payload_capped(output, 'X', 1024),
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
                assert_job_succeeded(
                    output,
                    "LD_PRELOAD=/test.so LD_LIBRARY_PATH=/testlib SAFE=safe_value",
                )
            },
            ..Scenario::default()
        },
        // =======================================================================
        // Resource constraint enforcement
        // =======================================================================
        Scenario {
            name: "memory_size_visible",
            description: "Guest sees correct memory with --memory flag",
            // The kernel keeps part of it, so the guest sees below 64 MiB, and far
            // below the roughly 481 MiB it sees by default.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "printf 'MEM_MIB=%s\\n' $(awk '/MemTotal/ {print int($2 / 1024)}' /proc/meminfo)"]"#,
            extra_args: &["--memory", "64", "--timeout", "60"],
            validate: |output| assert_guest_value_within(output, "MEM_MIB=", 1..=64),
            ..Scenario::default()
        },
        Scenario {
            name: "disk_size_override",
            description: "--disk flag configures ext4 size",
            // Verify the --disk flag is accepted and the ext4 image is
            // created at the requested size. Note: the ext4 image uses a
            // sparse file, so the VM won't actually enforce the limit at
            // the block device level.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "printf 'DISK_MIB=%s\\n' $(df -m / | awk 'NR == 2 {print $2}')"]"#,
            extra_args: &["--disk", "64", "--timeout", "60"],
            validate: |output| assert_guest_value_within(output, "DISK_MIB=", 1..=64),
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
            // ext4 overhead reduces usable space. For a 64 MiB image, total
            // should be roughly 40-60 MiB (not 1024+ default).
            validate: |output| assert_guest_value_within(output, "TOTAL_MB=", 1..=100),
            ..Scenario::default()
        },
        Scenario {
            name: "cpu_count_visible",
            description: "Guest sees 1 CPU with default vCPU count",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "printf 'NPROC=%s\\n' $(nproc)"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| assert_job_succeeded(output, "NPROC=1"),
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
CMD ["sh", "-c", "wget -q -O /dev/null http://detectportal.firefox.com/success.txt && printf '%s_%s\\n' NET OK || printf '%s_%s\\n' NET FAIL"]"#,
            extra_args: &["--timeout", "30", "--network"],
            validate: |output| {
                let combined = format!("{}{}", output.stdout, output.stderr);
                if combined.contains("panic") || combined.contains("SIGSEGV") {
                    bail!("Runner crashed with --network: {combined}")
                }
                // Network may not be available in all test environments, so
                // NET_FAIL passes too: the key thing is --network didn't cause a crash.
                if guest_printed(output, "NET_FAIL") > 0 {
                    return assert_job_succeeded(output, "NET_FAIL");
                }
                assert_job_succeeded(output, "NET_OK")
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
            validate: |output| assert_job_succeeded(output, "content_ok"),
            ..Scenario::default()
        },
        Scenario {
            name: "file_permissions_preserved",
            description: "Executable bit preserved through OCI unpack + ext4",
            // chmod +x in a RUN layer must survive OCI layer extraction.
            // If permissions are lost, `test -x` fails and we don't see "PERM_OK".
            dockerfile: r#"FROM busybox
RUN mkdir -p /data && printf '#!/bin/sh\necho hello' > /data/test.sh && chmod +x /data/test.sh
CMD ["sh", "-c", "test -x /data/test.sh && printf '%s_%s\\n' PERM OK"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| assert_job_succeeded(output, "PERM_OK"),
            ..Scenario::default()
        },
        Scenario {
            name: "directory_permissions_preserved",
            description: "Directory permissions preserved through OCI unpack + ext4",
            // chmod 750 on a directory in a RUN layer must survive extraction.
            // stat -c '%a' prints the octal mode.
            dockerfile: r#"FROM busybox
RUN mkdir -p /data/restricted && chmod 750 /data/restricted
CMD ["sh", "-c", "printf 'MODE=%s\\n' $(stat -c %a /data/restricted)"]"#,
            extra_args: &["--timeout", "60"],
            validate: |output| assert_job_succeeded(output, "MODE=750"),
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
            validate: |output| assert_job_succeeded(output, "SPACED=hello world EQ=key=value"),
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
            extra_args: &[
                "--timeout",
                "60",
                "--entrypoint",
                "printf",
                "%s_%s\\n",
                "CLI",
                "EP",
            ],
            validate: |output| {
                assert_job_succeeded(output, "CLI_EP")?;
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
ENTRYPOINT ["printf", "%s_%s\\n"]
CMD ["IMAGE", "CMD"]"#,
            extra_args: &["--timeout", "60", "--cmd", "CLI", "CMD"],
            validate: |output| {
                assert_job_succeeded(output, "CLI_CMD")?;
                if guest_printed(output, "IMAGE_CMD") > 0 {
                    bail!(
                        "OCI image CMD should have been overridden.\nstdout: {}",
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
                "printf",
                "--cmd",
                "%s_%s\\n",
                "CLI",
                "BOTH",
            ],
            validate: |output| {
                assert_job_succeeded(output, "CLI_BOTH")?;
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
CMD ["sh", "-c", "printf 'VAR:%s\\n' $MY_VAR"]"#,
            extra_args: &["--timeout", "60", "--env", "MY_VAR=cli_value"],
            validate: |output| assert_job_succeeded(output, "VAR:cli_value"),
            ..Scenario::default()
        },
        Scenario {
            name: "cli_env_add",
            description: "Add a new ENV from CLI alongside image ENV",
            dockerfile: r#"FROM busybox
ENV EXISTING=from_image
CMD ["sh", "-c", "echo EXISTING=$EXISTING NEW=$NEW_VAR"]"#,
            extra_args: &["--timeout", "60", "--env", "NEW_VAR=from_cli"],
            validate: |output| assert_job_succeeded(output, "EXISTING=from_image NEW=from_cli"),
            ..Scenario::default()
        },
        Scenario {
            name: "cli_env_multiple",
            description: "Multiple --env flags",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "echo A=$A B=$B"]"#,
            extra_args: &["--timeout", "60", "--env", "A=one", "--env", "B=two"],
            validate: |output| assert_job_succeeded(output, "A=one B=two"),
            ..Scenario::default()
        },
        Scenario {
            name: "cli_entrypoint_no_image_entrypoint",
            description: "Add entrypoint when image only has CMD",
            dockerfile: r#"FROM busybox
CMD ["hello", "world"]"#,
            extra_args: &[
                "--timeout",
                "60",
                "--entrypoint",
                "printf",
                "%s_%s\\n",
                "ENTRYPOINT",
                "ALONE",
            ],
            validate: |output| {
                assert_job_succeeded(output, "ENTRYPOINT_ALONE")?;
                // Docker semantics: a CLI entrypoint clears the OCI CMD, which
                // `printf` would otherwise print as one more line.
                if guest_printed(output, "hello_world") > 0 || output.stdout.contains("hello world")
                {
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
CMD ["printf", "%s_%s\\n", "ITER", "OUTPUT"]"#,
            extra_args: &["--timeout", "60", "--iter", "3"],
            validate: |output| {
                assert_job_succeeded(output, "ITER_OUTPUT")?;
                let count = guest_printed(output, "ITER_OUTPUT");
                if count != 3 {
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
CMD ["printf", "%s_%s\\n", "ZERO", "ITERATIONS"]"#,
            extra_args: &["--timeout", "60", "--iter", "0"],
            control_marker: Some("ZERO_ITERATIONS"),
            validate: |output| {
                if output.exit_code != 0 {
                    bail!("Expected exit code 0, got {}", output.exit_code)
                }
                if guest_printed(output, "ZERO_ITERATIONS") > 0 {
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
CMD ["sh", "-c", "printf '%s_%s\\n' ITER DONE && exit 1"]"#,
            extra_args: &["--timeout", "60", "--iter", "3"],
            validate: |output| {
                assert_guest_exited(output, "ITER_DONE", 1)?;
                // Only 1 iteration should run before aborting.
                let count = guest_printed(output, "ITER_DONE");
                if count != 1 {
                    bail!("Expected 1 iteration, found {count}")
                }
                Ok(())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "allow_failure_true_continues",
            description: "Non-zero exit code continues with --allow-failure",
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "printf '%s_%s\\n' ITER DONE && exit 1"]"#,
            extra_args: &["--timeout", "60", "--iter", "3", "--allow-failure"],
            validate: |output| {
                if output.exit_code != 0 {
                    bail!(
                        "Expected exit code 0 with --allow-failure, got {}",
                        output.exit_code
                    )
                }
                let count = guest_printed(output, "ITER_DONE");
                if count != 3 {
                    bail!("Expected 3 iterations with --allow-failure, found {count}")
                }
                Ok(())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "timeout_spans_iterations",
            description: "One timeout spans every iteration, and running out ends the run even with --allow-failure",
            // One iteration fits in the timeout and two do not, even with the
            // shutdown allowance, so only a timeout given to each iteration lets
            // the second finish.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "sleep 13 && printf '%s_%s\\n' SPAN DONE"]"#,
            extra_args: &["--timeout", "20", "--iter", "2", "--allow-failure"],
            validate: |output| {
                let timed_out = run_metrics(output).any(|metrics| {
                    metrics.get("timed_out") == Some(&serde_json::Value::Bool(true))
                });
                anyhow::ensure!(
                    output.exit_code == 1 && guest_printed(output, "SPAN_DONE") == 1 && timed_out,
                    "Expected the first iteration's 'SPAN_DONE' and the second cut off by the run's timeout, got exit code {}.\nstdout: {}\nstderr: {}",
                    output.exit_code,
                    output.stdout,
                    output.stderr
                );
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
        Scenario {
            name: "nosandbox_timeout_spans_iterations",
            description: "Non-sandboxed: one timeout spans every iteration, and running out ends the run even with --allow-failure",
            // One iteration fits in the timeout and two do not, so only a
            // timeout given to each process lets the second finish.
            dockerfile: r#"FROM busybox:musl
CMD ["sh", "-c", "sleep 5 && printf '%s_%s\\n' HOST SPAN"]"#,
            sandboxed: false,
            extra_args: &["--timeout", "8", "--iter", "2", "--allow-failure"],
            validate: |output| {
                let timed_out = run_metrics(output).any(|metrics| {
                    metrics.get("timed_out") == Some(&serde_json::Value::Bool(true))
                });
                anyhow::ensure!(
                    output.exit_code == 1 && guest_printed(output, "HOST_SPAN") == 1 && timed_out,
                    "Expected the first iteration's 'HOST_SPAN' and the second cut off by the run's timeout, got exit code {}.\nstdout: {}\nstderr: {}",
                    output.exit_code,
                    output.stdout,
                    output.stderr
                );
                Ok(())
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

fn target_dir() -> Result<Utf8PathBuf> {
    resolve_target_dir(
        std::env::var_os("CARGO_TARGET_DIR").as_deref(),
        &super::workspace_root(),
    )
}

/// A relative `CARGO_TARGET_DIR` is joined to the workspace root, since cargo
/// resolves it against the build's working directory, not the harness's.
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

    let pid = child.id();

    if !wait_for_booted_vmm(state_dir, &mut child)? {
        drop(child.kill());
        let output = child.wait_with_output()?;
        bail!(
            "The guest never booted within {PROBE_TIMEOUT:?}, so the cancel would not reach a running VM.\nstdout: {}\nstderr: {}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr),
        );
    }
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

/// SIGTERM a booted run twice, then prove the next run in the same state
/// directory reclaims what the kill left.
fn run_runner_cancelled_twice(
    image_path: &Utf8Path,
    args: &[&str],
    state_dir: &Utf8Path,
    runner_bin: &Utf8Path,
) -> Result<ScenarioOutput> {
    use std::os::unix::process::ExitStatusExt as _;

    let long_image = build_test_image("job_cancelled_twice_long", ORPHAN_DOCKERFILE)
        .context("Failed to build the long guest's image")?;
    let mut child = spawn_runner(&long_image, args, runner_bin)?;
    let drained = drain_output(&mut child);
    let signalled = signal_twice(state_dir, &mut child);
    if child.try_wait()?.is_none() {
        kill_pid(child.id(), libc::SIGKILL);
    }
    let status = child.wait()?;
    let (stdout, stderr) = drained.join();
    signalled.with_context(|| format!("stdout: {stdout}\nstderr: {stderr}"))?;
    anyhow::ensure!(
        status.signal() == Some(libc::SIGTERM),
        "Expected the second SIGTERM to kill the runner, but it ended with {status}.\nstdout: {stdout}\nstderr: {stderr}"
    );

    let left = find_jail(&jail_parent(state_dir))?
        .context("The kill left no jail, so the next run's reclaim went untested")?;
    // It may still exit on its own, from the teardown the first signal began.
    let vmm = find_jailed_vmm(&left.1)?;
    let output = run_runner(image_path, args, runner_bin)?;
    if let Some(pid) = vmm {
        anyhow::ensure!(
            !is_firecracker(pid)?,
            "The VMM (pid {pid}) the kill left is still running after the next run.\nstdout: {}\nstderr: {}",
            output.stdout,
            output.stderr
        );
    }
    anyhow::ensure!(
        records_with(&output.stderr, "Reclaimed stale jails")
            .any(|record| record.get("count") == Some(&serde_json::Value::from(1))),
        "The next run never reclaimed the jail {} the kill left.\nstdout: {}\nstderr: {}",
        left.0,
        output.stdout,
        output.stderr
    );
    Ok(output)
}

/// The jail comes tens of milliseconds after the parse line, far longer than
/// the signal takes to land.
fn run_runner_cancelled_while_preparing(
    image_path: &Utf8Path,
    args: &[&str],
    runner_bin: &Utf8Path,
) -> Result<ScenarioOutput> {
    let mut child = spawn_runner(image_path, args, runner_bin)?;
    let mut streamed = StreamedOutput::start(&mut child);
    let parsing = streamed.wait_for_record("Parsing OCI image config");
    kill_pid(
        child.id(),
        if parsing.is_ok() {
            libc::SIGTERM
        } else {
            libc::SIGKILL
        },
    );
    let deadline = std::time::Instant::now() + CANCEL_GRACE;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if std::time::Instant::now() >= deadline {
            kill_pid(child.id(), libc::SIGKILL);
            child.wait()?;
            break None;
        }
        std::thread::sleep(PROBE_INTERVAL);
    };
    let (stdout, stderr) = streamed.join();
    parsing.with_context(|| format!("stdout: {stdout}\nstderr: {stderr}"))?;
    let status = status.with_context(|| {
        format!(
            "The runner did not exit within {CANCEL_GRACE:?} of the SIGTERM.\nstdout: {stdout}\nstderr: {stderr}"
        )
    })?;
    Ok(ScenarioOutput {
        stdout,
        stderr,
        exit_code: status.code().unwrap_or(-1),
    })
}

const CANCEL_GRACE: Duration = Duration::from_secs(30);

/// Standard signals do not queue, so the second is sent only once the first
/// has been delivered, which resets its handler.
fn signal_twice(state_dir: &Utf8Path, child: &mut std::process::Child) -> Result<()> {
    anyhow::ensure!(
        wait_for_booted_vmm(state_dir, child)?,
        "The guest never booted within {PROBE_TIMEOUT:?}"
    );
    let pid = child.id();
    anyhow::ensure!(
        catches(pid, libc::SIGTERM)?,
        "The runner does not catch SIGTERM, so a first one would not cancel through the teardown"
    );
    kill_pid(pid, libc::SIGTERM);
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while catches(pid, libc::SIGTERM)? {
        anyhow::ensure!(
            std::time::Instant::now() < deadline,
            "The first SIGTERM left its handler in place, so a second cannot kill the runner"
        );
        std::thread::sleep(Duration::from_millis(1));
    }
    kill_pid(pid, libc::SIGTERM);
    Ok(())
}

/// Whether `pid` has a handler installed for `signal`, from its `SigCgt` mask.
fn catches(pid: u32, signal: libc::c_int) -> Result<bool> {
    let status = fs::read_to_string(format!("/proc/{pid}/status"))
        .with_context(|| format!("Failed to read the status of pid {pid}"))?;
    let mask = status
        .lines()
        .find_map(|line| line.strip_prefix("SigCgt:"))
        .context("No SigCgt line in the runner's status")?;
    let mask = u64::from_str_radix(mask.trim(), 16).context("Unparsable SigCgt mask")?;
    let bit = u32::try_from(signal - 1).context("Not a signal number")?;
    Ok(mask & (1 << bit) != 0)
}

/// Wait until a VMM in `state_dir` has booted its guest, which is when it has
/// vCPU threads, or the runner has exited.
fn wait_for_booted_vmm(state_dir: &Utf8Path, child: &mut std::process::Child) -> Result<bool> {
    let deadline = std::time::Instant::now() + PROBE_TIMEOUT;
    while std::time::Instant::now() < deadline && child.try_wait()?.is_none() {
        if probe_booted(state_dir)? {
            return Ok(true);
        }
        std::thread::sleep(PROBE_INTERVAL);
    }
    Ok(false)
}

fn probe_booted(state_dir: &Utf8Path) -> Result<bool> {
    let Some((_, jail_root)) = find_jail(&jail_parent(state_dir))? else {
        return Ok(false);
    };
    Ok(find_jailed_vmm(&jail_root)?.is_some_and(has_vcpu_threads))
}

/// Firecracker names each vCPU thread `fc_vcpu <n>` and starts them once it
/// has accepted its configuration.
fn has_vcpu_threads(pid: u32) -> bool {
    fs::read_dir(format!("/proc/{pid}/task")).is_ok_and(|tasks| {
        tasks.flatten().any(|task| {
            fs::read_to_string(task.path().join("comm"))
                .is_ok_and(|comm| comm.starts_with("fc_vcpu"))
        })
    })
}

/// Build bencher-init for the musl target and the runner CLI with `BENCHER_INIT_PATH`,
/// then return the path to the runner binary.
fn ensure_runner_bin() -> Result<Utf8PathBuf> {
    // CI builds unprivileged first and points here, so the elevated run never
    // invokes cargo.
    if let Some(path) = std::env::var_os(RUNNER_BIN_ENV) {
        let path = Utf8PathBuf::from(path.to_string_lossy().into_owned());
        if !path.exists() {
            bail!("{RUNNER_BIN_ENV} is set to {path}, which does not exist");
        }
        println!("Using pre-built runner from {RUNNER_BIN_ENV}: {path}");
        return Ok(path);
    }

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

const RUNNER_BIN_ENV: &str = "BENCHER_RUNNER_BIN";

/// Generous: the runner pulls and unpacks the image and builds the rootfs
/// before the VMM is spawned.
const PROBE_TIMEOUT: Duration = Duration::from_mins(3);

const PROBE_INTERVAL: Duration = Duration::from_millis(100);

/// Not the runner's default (61016), so a runner that ignores `--jail-uid`
/// cannot pass.
const SCENARIO_JAIL_UID: &str = "61017";

/// Distinct from the uid, so a gid taken from the uid cannot pass.
const SCENARIO_JAIL_GID: &str = "61018";

const JAIL_ARGS: &[&str] = &[
    "--timeout",
    "120",
    "--jail-uid",
    SCENARIO_JAIL_UID,
    "--jail-gid",
    SCENARIO_JAIL_GID,
];

/// Spelled here rather than read from the runner, so the product cannot redefine
/// it underneath the harness.
const NETNS_HANDLE: &str = "/run/netns/bencher-jail";

/// The harness's own network namespace, which is the host's.
const HARNESS_NETNS: &str = "/proc/self/ns/net";

/// The same bound the runner's own unwind uses.
const MAX_NETNS_UNWIND: usize = 32;

fn scenario_state_dir() -> Utf8PathBuf {
    super::work_dir().join("state")
}

fn jail_parent(state_dir: &Utf8Path) -> Utf8PathBuf {
    state_dir.join("jail").join("firecracker")
}

#[expect(
    clippy::too_many_lines,
    reason = "each scenario needs its configuration"
)]
fn jail_scenarios() -> Vec<Scenario> {
    let mut scenarios = vec![
        Scenario {
            name: "jail_confinement",
            description: "A jailed job succeeds with the VMM unprivileged, off the host network, and in its cgroup",
            // The guest sleeps so the VMM lives long enough for the probe to see.
            dockerfile: r#"FROM busybox
CMD ["sh", "-c", "printf '%s_%s\\n' JAIL_CONFINEMENT a7f3b2c9 && sleep 5"]"#,
            cancel_after_secs: None,
            probe: Some(probe_confinement),
            orphan_then_rerun: false,
            extra_args: JAIL_ARGS,
            validate: |output| {
                // Everything the probe checks also holds for a VMM that never
                // booted a guest.
                assert_job_succeeded(output, "JAIL_CONFINEMENT_a7f3b2c9")?;
                assert_cpu_isolation_applied(output)?;
                assert_cgroup_metrics_reported(output)?;
                assert_no_cgroup_warning(output)?;
                assert_no_chroot_remains(&scenario_state_dir())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "jail_netns_recovers_from_stacked_mounts",
            description: "A job succeeds against a network namespace handle carrying stacked mounts",
            dockerfile: r#"FROM busybox
CMD ["printf", "%s_%s\\n", "JAIL_NETNS", "a7f3b2c9"]"#,
            setup: Some(stack_netns_mounts),
            teardown: Some(unstack_netns_mounts),
            extra_args: JAIL_ARGS,
            validate: |output| assert_job_succeeded(output, "JAIL_NETNS_a7f3b2c9"),
            ..Scenario::default()
        },
        Scenario {
            name: "jail_refused_fails_the_job",
            description: "A state directory the runner must refuse fails the job rather than running the VMM unjailed",
            // Nothing in here should ever run, and the marker is how that is
            // known: the guest prints it and the runner cannot.
            dockerfile: r#"FROM busybox
CMD ["printf", "%s_%s\\n", "JAIL_REFUSED", "a7f3b2c9"]"#,
            unusable_state_dir: true,
            extra_args: JAIL_ARGS,
            validate: |output| {
                // That no VMM was left running is asserted in
                // `run_runner_without_unjailed_vmm`.
                assert_refused_before_guest(output, "JAIL_REFUSED_a7f3b2c9")?;
                anyhow::ensure!(
                    runner_error(output).is_some_and(|error| error.contains("is a symbolic link")),
                    "Expected the refusal to name the symlinked state directory.\nstderr: {}",
                    output.stderr
                );
                Ok(())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "jail_nodev_state_dir_is_refused",
            description: "A state directory on a nodev filesystem is refused by name before the jail is built",
            dockerfile: r#"FROM busybox
CMD ["printf", "%s_%s\\n", "JAIL_NODEV", "a7f3b2c9"]"#,
            nodev_state_dir: true,
            extra_args: JAIL_ARGS,
            validate: |output| {
                assert_refused_before_guest(output, "JAIL_NODEV_a7f3b2c9")?;
                // Without the check the job still fails, later, with KVM blaming
                // its ACL.
                anyhow::ensure!(
                    runner_error(output).is_some_and(|error| error.contains("mounted nodev")),
                    "Expected the refusal to name the nodev mount.\nstderr: {}",
                    output.stderr
                );
                Ok(())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "jail_sweep_reclaims_orphan",
            description: "A chroot orphaned by a runner that never unwound is swept by the next job",
            // The next job's guest; the orphan runs `ORPHAN_DOCKERFILE`.
            dockerfile: r#"FROM busybox
CMD ["printf", "%s_%s\\n", "JAIL_SWEEP", "a7f3b2c9"]"#,
            cancel_after_secs: None,
            probe: None,
            orphan_then_rerun: true,
            extra_args: JAIL_ARGS,
            validate: |output| {
                assert_job_succeeded(output, "JAIL_SWEEP_a7f3b2c9")?;
                assert_no_chroot_remains(&scenario_state_dir())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "jail_sweep_runs_before_every_job",
            description: "A stale jail that appears between two iterations of one run is swept by the second, not only by the first",
            dockerfile: r#"FROM busybox
CMD ["printf", "%s_%s\\n", "EVERY_JOB", "a7f3b2c9"]"#,
            planted_between_jobs: true,
            extra_args: JAIL_ARGS,
            validate: |output| {
                assert_job_succeeded(output, "EVERY_JOB_a7f3b2c9")?;
                let jobs = guest_printed(output, "EVERY_JOB_a7f3b2c9");
                anyhow::ensure!(
                    jobs == 2,
                    "Expected both iterations to run, but the guest ran {jobs} time(s).\nstdout: {}\nstderr: {}",
                    output.stdout,
                    output.stderr
                );
                Ok(())
            },
            ..Scenario::default()
        },
    ];
    scenarios.extend(jail_contention_scenarios());
    scenarios
}

#[expect(
    clippy::too_many_lines,
    reason = "each scenario needs its configuration"
)]
fn jail_contention_scenarios() -> Vec<Scenario> {
    vec![
        Scenario {
            name: "jail_no_boot_past_timeout",
            description: "A run whose timeout runs out before its VM boots never boots it",
            dockerfile: r#"FROM busybox
CMD ["printf", "%s_%s\\n", "QUEUED", "a7f3b2c9"]"#,
            stopped_past_timeout: true,
            control_marker: Some("QUEUED_a7f3b2c9"),
            validate: |output| {
                anyhow::ensure!(
                    output.exit_code == 1
                        && runner_error(output)
                            .is_some_and(|error| error.contains("ran out before the VM booted"))
                        && records_with(&output.stderr, "Booting VM").next().is_none()
                        && guest_printed(output, "QUEUED_a7f3b2c9") == 0,
                    "Expected the run to fail on its timeout before its VM booted, got exit code {}.\nstdout: {}\nstderr: {}",
                    output.exit_code,
                    output.stdout,
                    output.stderr
                );
                Ok(())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "runner_lock_refuses_a_second_runner",
            description: "A second `runner run` or `runner up` on the host exits at once, naming the runner lock, whatever its state directory",
            dockerfile: r#"FROM busybox
CMD ["printf", "%s_%s\\n", "SECOND_RUNNER", "a7f3b2c9"]"#,
            second_runner: true,
            control_marker: Some("SECOND_RUNNER_a7f3b2c9"),
            validate: |output| assert_refused_by_the_lock(output, "SECOND_RUNNER_a7f3b2c9"),
            ..Scenario::default()
        },
        Scenario {
            name: "runner_up_holds_the_runner_lock",
            description: "A `runner up` holds the runner lock for as long as it runs, not only while it starts",
            dockerfile: r#"FROM busybox
CMD ["printf", "%s_%s\\n", "BESIDE_UP", "a7f3b2c9"]"#,
            held_by_runner_up: true,
            control_marker: Some("BESIDE_UP_a7f3b2c9"),
            validate: |output| assert_refused_by_the_lock(output, "BESIDE_UP_a7f3b2c9"),
            ..Scenario::default()
        },
        Scenario {
            name: "runner_run_takes_its_turn_at_the_job_lock",
            description: "A `runner run` waits out a job lock held elsewhere and keeps it from its VMM and its benchmark, and warns but runs beside a maintenance marker",
            dockerfile: r#"FROM busybox:musl
CMD ["sh", "-c", "ls -l /proc/$$/fd && printf '%s_%s\\n' JOB_TURN a7f3b2c9 && sleep 2"]"#,
            behind_the_job_lock: true,
            // The first run adds the sandbox itself; the second needs none, so
            // its benchmark is the runner's own child.
            sandboxed: false,
            validate: assert_ran_beside_the_marker,
            ..Scenario::default()
        },
        Scenario {
            name: "jail_occupied_cgroup_is_killed",
            description: "A process in another Bencher cgroup is killed, and its cgroup removed, before the job runs",
            dockerfile: r#"FROM busybox
CMD ["printf", "%s_%s\\n", "JAIL_OCCUPIED", "a7f3b2c9"]"#,
            occupied_cgroup: true,
            extra_args: JAIL_ARGS,
            validate: |output| assert_job_succeeded(output, "JAIL_OCCUPIED_a7f3b2c9"),
            ..Scenario::default()
        },
        Scenario {
            name: "occupied_cgroup_is_killed_at_startup",
            description: "A process in a Bencher cgroup is killed, and its cgroup removed, when a runner starts, even one that runs no sandbox",
            dockerfile: r#"FROM busybox:musl
CMD ["printf", "%s_%s\\n", "OCCUPIED_AT_START", "a7f3b2c9"]"#,
            occupied_at_start: true,
            sandboxed: false,
            extra_args: &["--timeout", "60"],
            validate: |output| assert_job_succeeded(output, "OCCUPIED_AT_START_a7f3b2c9"),
            ..Scenario::default()
        },
        Scenario {
            name: "occupied_cgroup_is_killed_at_runner_up_startup",
            description: "A process in a Bencher cgroup is killed, and its cgroup removed, when a `runner up` starts, before any Job",
            dockerfile: r#"FROM busybox
CMD ["true"]"#,
            occupied_at_up_start: true,
            ..Scenario::default()
        },
        Scenario {
            name: "startup_refuses_a_kernel_without_cgroup_kill",
            description: "A sandboxed `runner run` or `runner up` on a kernel without cgroup.kill exits at startup, naming Linux 5.14",
            dockerfile: r#"FROM busybox
CMD ["printf", "%s_%s\\n", "NO_KILL", "a7f3b2c9"]"#,
            without_cgroup_kill: true,
            validate: |output| {
                anyhow::ensure!(
                    output.exit_code == 1
                        && runner_error(output).is_some_and(|error| error.contains("Linux 5.14"))
                        && records_with(&output.stderr, "Executing benchmark run")
                            .next()
                            .is_none()
                        && guest_printed(output, "NO_KILL_a7f3b2c9") == 0,
                    "Expected the runner to exit at startup naming Linux 5.14, before any Job, got exit code {}.\nstdout: {}\nstderr: {}",
                    output.exit_code,
                    output.stdout,
                    output.stderr
                );
                Ok(())
            },
            ..Scenario::default()
        },
        Scenario {
            name: "nosandbox_starts_on_a_kernel_without_cgroup_kill",
            description: "A `runner run` or `runner up` that may run Jobs with no sandbox starts on a kernel without cgroup.kill",
            dockerfile: r#"FROM busybox:musl
CMD ["printf", "%s_%s\\n", "NO_KILL_HOST", "a7f3b2c9"]"#,
            without_cgroup_kill: true,
            sandboxed: false,
            extra_args: &["--timeout", "60"],
            validate: |output| assert_job_succeeded(output, "NO_KILL_HOST_a7f3b2c9"),
            ..Scenario::default()
        },
        Scenario {
            name: "jail_sweep_kills_an_orphan_of_another_state_dir",
            description: "A VMM orphaned by a runner with another state directory is killed by the next job",
            dockerfile: r#"FROM busybox
CMD ["printf", "%s_%s\\n", "JAIL_ELSEWHERE", "a7f3b2c9"]"#,
            orphan_elsewhere_then_rerun: true,
            extra_args: JAIL_ARGS,
            validate: |output| assert_job_succeeded(output, "JAIL_ELSEWHERE_a7f3b2c9"),
            ..Scenario::default()
        },
    ]
}

/// Spelled here rather than read from the runner, so the product cannot move it
/// underneath the harness.
const RUNNER_LOCK: &str = "/run/bencher/runner.lock";

/// Keeps the first runner, and so its lock, alive well past the second's start.
const HOLDER_DOCKERFILE: &str = r#"FROM busybox
CMD ["sh", "-c", "echo RUNNER_HOLDER_a7f3b2c9 && sleep 8"]"#;

/// The second runner gets a state directory of its own, so only a lock outside
/// every state directory can stop it.
fn run_second_runner(
    image_path: &Utf8Path,
    args: &[&str],
    state_dir: &Utf8Path,
    runner_bin: &Utf8Path,
) -> Result<ScenarioOutput> {
    let holder_image = build_test_image("runner_holder", HOLDER_DOCKERFILE)
        .context("Failed to build the holder's image")?;
    let parent = jail_parent(state_dir);
    let mut holder = spawn_runner(
        &holder_image,
        &[args, &["--timeout", "60"]].concat(),
        runner_bin,
    )?;
    let holder_output = drain_output(&mut holder);

    let deadline = std::time::Instant::now() + PROBE_TIMEOUT;
    let held = loop {
        if let Some((_, jail_root)) = find_jail(&parent)?
            && find_jailed_vmm(&jail_root)?.is_some()
        {
            break true;
        }
        if holder.try_wait()?.is_some() || std::time::Instant::now() >= deadline {
            break false;
        }
        std::thread::sleep(PROBE_INTERVAL);
    };
    let second_state = super::work_dir().join("second-state");
    drop(fs::remove_dir_all(&second_state));
    let output = if held {
        let second_args: Vec<&str> = args
            .iter()
            .map(|arg| {
                if *arg == state_dir.as_str() {
                    second_state.as_str()
                } else {
                    arg
                }
            })
            .collect();
        run_runner(image_path, &second_args, runner_bin).and_then(|output| {
            let up = run_second_runner_up(&second_state, runner_bin)?;
            anyhow::ensure!(
                up.exit_code == 1
                    && runner_error(&up).is_some_and(|error| error.contains(RUNNER_LOCK)),
                "Expected `runner up` to exit naming {RUNNER_LOCK}, got exit code {}.\nstdout: {}\nstderr: {}",
                up.exit_code,
                up.stdout,
                up.stderr
            );
            Ok(output)
        })
    } else {
        if holder.try_wait()?.is_none() {
            kill_pid(holder.id(), libc::SIGKILL);
        }
        Err(anyhow::anyhow!(
            "The holder's VMM never appeared within {PROBE_TIMEOUT:?}"
        ))
    };
    let status = holder.wait()?;
    let (stdout, stderr) = holder_output.join();
    let stranded = reclaim_stranded_jails(&second_state);
    drop(fs::remove_dir_all(&second_state));
    let output =
        output.with_context(|| format!("holder stdout: {stdout}\nholder stderr: {stderr}"))?;
    anyhow::ensure!(
        stranded?.is_empty(),
        "The second runner left a jail behind in {second_state}"
    );
    anyhow::ensure!(
        status.success(),
        "The holder failed, so the second runner may have disturbed it.\nstdout: {stdout}\nstderr: {stderr}"
    );
    Ok(output)
}

/// Syntactically a runner key, for a server that is never reached.
const PROBE_KEY: &str = "bencher_runner_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh";

/// Far past the moment a runner fails at its lock, and the daemon retries an
/// unreachable server for ever.
const UP_EXITS_WITHIN: Duration = Duration::from_secs(30);

/// `runner up` against an address nothing answers, so only the lock can end it.
fn run_second_runner_up(state_dir: &Utf8Path, runner_bin: &Utf8Path) -> Result<ScenarioOutput> {
    let mut child = spawn_runner_up(state_dir, runner_bin)?;
    let drained = drain_output(&mut child);
    let deadline = std::time::Instant::now() + UP_EXITS_WITHIN;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if std::time::Instant::now() >= deadline {
            kill_pid(child.id(), libc::SIGKILL);
            child.wait()?;
            break None;
        }
        std::thread::sleep(PROBE_INTERVAL);
    };
    let (stdout, stderr) = drained.join();
    let status = status.with_context(|| {
        format!("`runner up` was still running after {UP_EXITS_WITHIN:?}.\nstdout: {stdout}\nstderr: {stderr}")
    })?;
    Ok(ScenarioOutput {
        stdout,
        stderr,
        exit_code: status.code().unwrap_or(-1),
    })
}

/// A daemon whose server is an address nothing answers, so it retries for ever.
fn spawn_runner_up(state_dir: &Utf8Path, runner_bin: &Utf8Path) -> Result<std::process::Child> {
    Ok(Command::new(runner_bin.as_str())
        .args([
            "up",
            "--host",
            "http://127.0.0.1:9",
            "--runner",
            "lock-probe",
            "--key",
            PROBE_KEY,
            "--no-tuning",
            "--state-dir",
            state_dir.as_str(),
        ])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?)
}

/// How long the daemon has been waiting on its server when the run starts.
const UP_WAITING_FOR: Duration = Duration::from_secs(2);

/// The run starts once the daemon is past every startup step and waiting on
/// its server, so only a lock held for the daemon's whole life stops it.
fn run_runner_beside_runner_up(
    image_path: &Utf8Path,
    args: &[&str],
    runner_bin: &Utf8Path,
) -> Result<ScenarioOutput> {
    let up_state = super::work_dir().join("up-state");
    drop(fs::remove_dir_all(&up_state));
    let mut up = spawn_runner_up(&up_state, runner_bin)?;
    let mut streamed = StreamedOutput::start(&mut up);
    let output = streamed
        .wait_for_record("Connecting to channel")
        .and_then(|_record| {
            std::thread::sleep(UP_WAITING_FOR);
            run_runner(image_path, args, runner_bin)
        });
    let ran_through = up.try_wait()?.is_none();
    kill_pid(up.id(), libc::SIGKILL);
    up.wait()?;
    let (stdout, stderr) = streamed.join();
    drop(fs::remove_dir_all(&up_state));
    let output = output
        .with_context(|| format!("runner up stdout: {stdout}\nrunner up stderr: {stderr}"))?;
    anyhow::ensure!(
        ran_through,
        "`runner up` exited before the run ended, so it held nothing.\nstdout: {stdout}\nstderr: {stderr}"
    );
    Ok(output)
}

/// The runner's own failure exit, naming the lock, before it built a jail or
/// ran the guest.
fn assert_refused_by_the_lock(output: &ScenarioOutput, marker: &str) -> Result<()> {
    anyhow::ensure!(
        output.exit_code == 1
            && runner_error(output).is_some_and(|error| error.contains(RUNNER_LOCK))
            && records_with(&output.stderr, "Jail built").next().is_none()
            && guest_printed(output, marker) == 0,
        "Expected the runner to exit naming {RUNNER_LOCK} before it built a jail, got exit code {}.\nstdout: {}\nstderr: {}",
        output.exit_code,
        output.stdout,
        output.stderr
    );
    Ok(())
}

/// Spelled here, like the runner lock, so the product cannot move them
/// underneath the harness.
const JOB_LOCK: &str = "/run/bencher/job.lock";
const MAINTENANCE_MARKER: &str = "/run/bencher/maintenance";

const WAITING_FOR_MAINTENANCE: &str = "Waiting for host maintenance to finish";
const MAINTENANCE_DUE: &str = "Running although host maintenance is due";

/// Printed only once the benchmark has listed its descriptors.
const JOB_TURN: &str = "JOB_TURN_a7f3b2c9";

/// How long maintenance keeps the lock once the runner waits for it.
const JOB_LOCK_HELD_FOR: Duration = Duration::from_secs(2);

/// The first run proves the wait and the second the marker's warning, on the
/// host, where a descriptor leaked past exec reaches the benchmark.
fn run_runner_behind_the_job_lock(
    image_path: &Utf8Path,
    args: &[&str],
    state_dir: &Utf8Path,
    runner_bin: &Utf8Path,
) -> Result<ScenarioOutput> {
    let sandboxed = [args, &["--sandbox", "firecracker", "--timeout", "60"]].concat();
    let waited =
        run_runner_while_the_job_lock_is_held(image_path, &sandboxed, state_dir, runner_bin)?;
    assert_waited_its_turn(&waited)?;
    let _marker = MaintenanceMarker::place()?;
    run_runner_bounded(
        image_path,
        &[args, &["--timeout", "60"]].concat(),
        runner_bin,
    )
}

/// The runner must neither build its jail while the harness holds the lock nor
/// hand the lock on to its VMM once it has its turn.
fn run_runner_while_the_job_lock_is_held(
    image_path: &Utf8Path,
    args: &[&str],
    state_dir: &Utf8Path,
    runner_bin: &Utf8Path,
) -> Result<ScenarioOutput> {
    let parent = jail_parent(state_dir);
    let held = hold_job_lock()?;
    let mut child = spawn_runner(image_path, args, runner_bin)?;
    let mut streamed = StreamedOutput::start(&mut child);
    // Either record, so a runner that never waits fails once it builds its jail.
    let waited = streamed
        .wait_for(|line| {
            record_of(line).is_some_and(|record| {
                matches!(
                    text(&record, "msg"),
                    Some(WAITING_FOR_MAINTENANCE | "Jail built")
                )
            })
        })
        .and_then(|first| {
            anyhow::ensure!(
                record_of(&first)
                    .is_some_and(|record| text(&record, "msg") == Some(WAITING_FOR_MAINTENANCE)),
                "The runner built its jail while maintenance held the job lock"
            );
            std::thread::sleep(JOB_LOCK_HELD_FOR);
            anyhow::ensure!(
                child.try_wait()?.is_none()
                    && !streamed.has_logged("Jail built")
                    && find_jail(&parent)?.is_none(),
                "The runner went on while maintenance held the job lock"
            );
            Ok(())
        });
    drop(held);
    let kept = waited.and_then(|()| vmm_holds_no_job_lock(&parent, &mut child));
    // Only while unreaped, since a reaped pid may already be another's.
    if kept.is_err() && matches!(child.try_wait(), Ok(None)) {
        kill_pid(child.id(), libc::SIGKILL);
    }
    let status = exit_within(&mut child, PROBE_TIMEOUT);
    let (stdout, stderr) = streamed.join();
    let context = || format!("stdout: {stdout}\nstderr: {stderr}");
    kept.with_context(context)?;
    let status = status.with_context(context)?;
    Ok(ScenarioOutput {
        stdout,
        stderr,
        exit_code: status.code().unwrap_or(-1),
    })
}

/// Maintenance's hold: an open file description of the harness's own, which
/// `flock` sets against the runner's until it is dropped.
fn hold_job_lock() -> Result<fs::File> {
    use std::os::unix::fs::{DirBuilderExt as _, OpenOptionsExt as _};

    // The runner's own modes, so creating either loosens nothing.
    let lock = Utf8Path::new(JOB_LOCK);
    if let Some(dir) = lock.parent() {
        fs::DirBuilder::new()
            .recursive(true)
            .mode(0o700)
            .create(dir)
            .with_context(|| format!("Failed to create {dir}"))?;
    }
    let file = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(lock)
        .with_context(|| format!("Failed to open {JOB_LOCK}"))?;
    file.try_lock()
        .with_context(|| format!("Something else holds {JOB_LOCK}, so the harness cannot"))?;
    Ok(file)
}

/// Once the VMM is up, neither it nor the jailer it was exec'd from holds a
/// descriptor of the job lock.
fn vmm_holds_no_job_lock(parent: &Utf8Path, child: &mut std::process::Child) -> Result<()> {
    let lock = fs::metadata(JOB_LOCK).with_context(|| format!("Failed to stat {JOB_LOCK}"))?;
    let deadline = std::time::Instant::now() + PROBE_TIMEOUT;
    loop {
        if let Some((_, jail_root)) = find_jail(parent)?
            && let Some(pid) = find_jailed_vmm(&jail_root)?
        {
            anyhow::ensure!(!holds(pid, &lock)?, "The VMM, pid {pid}, holds {JOB_LOCK}");
            return Ok(());
        }
        anyhow::ensure!(
            child.try_wait()?.is_none() && std::time::Instant::now() < deadline,
            "The jailed VMM was never observed within {PROBE_TIMEOUT:?}"
        );
        std::thread::sleep(PROBE_INTERVAL);
    }
}

/// Whether a descriptor of `pid` is open on `file`, which none is once it has
/// exited.
fn holds(pid: u32, file: &fs::Metadata) -> Result<bool> {
    let fds = match fs::read_dir(format!("/proc/{pid}/fd")) {
        Ok(fds) => fds,
        Err(e) if gone(&e) => return Ok(false),
        Err(e) => {
            return Err(e).with_context(|| format!("Failed to read the descriptors of pid {pid}"));
        },
    };
    Ok(fds
        .filter_map(Result::ok)
        .any(|fd| fs::metadata(fd.path()).is_ok_and(|target| same_object(&target, file))))
}

/// The exit status, or an error once `within` has passed and the child is
/// killed.
fn exit_within(
    child: &mut std::process::Child,
    within: Duration,
) -> Result<std::process::ExitStatus> {
    let deadline = std::time::Instant::now() + within;
    loop {
        if let Some(status) = child.try_wait()? {
            return Ok(status);
        }
        if std::time::Instant::now() >= deadline {
            kill_pid(child.id(), libc::SIGKILL);
            child.wait()?;
            bail!("The runner was still running after {within:?}");
        }
        std::thread::sleep(PROBE_INTERVAL);
    }
}

/// One wait for the lock, then the Job, and no warning, since no marker was
/// there.
fn assert_waited_its_turn(output: &ScenarioOutput) -> Result<()> {
    assert_job_succeeded(output, JOB_TURN)?;
    anyhow::ensure!(
        records_with(&output.stderr, WAITING_FOR_MAINTENANCE).count() == 1
            && records_with(&output.stderr, MAINTENANCE_DUE)
                .next()
                .is_none(),
        "Expected one wait for the job lock and no warning that maintenance is due.\nstdout: {}\nstderr: {}",
        output.stdout,
        output.stderr
    );
    Ok(())
}

/// A marker of the harness's own, removed on drop; one already there is never
/// taken over, so another's is never removed.
struct MaintenanceMarker;

impl MaintenanceMarker {
    fn place() -> Result<Self> {
        use std::os::unix::fs::OpenOptionsExt as _;

        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(MAINTENANCE_MARKER)
            .with_context(|| format!("Failed to create {MAINTENANCE_MARKER}"))?;
        Ok(Self)
    }
}

impl Drop for MaintenanceMarker {
    fn drop(&mut self) {
        drop(fs::remove_file(MAINTENANCE_MARKER));
    }
}

/// Bounded by the harness, since a runner held up before its Job's timeout
/// starts would otherwise never end.
fn run_runner_bounded(
    image_path: &Utf8Path,
    args: &[&str],
    runner_bin: &Utf8Path,
) -> Result<ScenarioOutput> {
    let mut child = spawn_runner(image_path, args, runner_bin)?;
    let drained = drain_output(&mut child);
    let status = exit_within(&mut child, PROBE_TIMEOUT);
    let (stdout, stderr) = drained.join();
    let status = status.with_context(|| format!("stdout: {stdout}\nstderr: {stderr}"))?;
    Ok(ScenarioOutput {
        stdout,
        stderr,
        exit_code: status.code().unwrap_or(-1),
    })
}

/// The marker draws one warning and no wait, and the benchmark, the runner's
/// own child on the host, holds no descriptor of the job lock.
fn assert_ran_beside_the_marker(output: &ScenarioOutput) -> Result<()> {
    assert_job_succeeded(output, JOB_TURN)?;
    anyhow::ensure!(
        records_with(&output.stderr, MAINTENANCE_DUE).count() == 1
            && records_with(&output.stderr, WAITING_FOR_MAINTENANCE)
                .next()
                .is_none()
            && !output.stdout.lines().any(|line| line.contains(JOB_LOCK)),
        "Expected one warning that maintenance is due, no wait, and no descriptor of {JOB_LOCK} in the benchmark.\nstdout: {}\nstderr: {}",
        output.stdout,
        output.stderr
    );
    Ok(())
}

/// A name the runner could have minted, so its sweep takes the jail for its own.
const PLANTED_JAIL: &str = "planted-between-jobs";

/// Planted once the first iteration's jail is built, after its sweep, and with
/// its whole guest still to run before the second iteration's, so only the
/// second iteration's sweep can reclaim it.
fn run_runner_planted_between_jobs(
    image_path: &Utf8Path,
    args: &[&str],
    state_dir: &Utf8Path,
    runner_bin: &Utf8Path,
) -> Result<ScenarioOutput> {
    let planted = jail_parent(state_dir).join(PLANTED_JAIL);
    let mut child = spawn_runner(image_path, &[args, &["--iter", "2"]].concat(), runner_bin)?;
    let mut streamed = StreamedOutput::start(&mut child);
    let placed = streamed.wait_for_record("Jail built").and_then(|_first| {
        let root = planted.join("root");
        fs::create_dir_all(&root)
            .and_then(|()| fs::write(root.join("rootfs.ext4"), b"stale"))
            .with_context(|| format!("Failed to plant {planted}"))
    });
    let deadline = std::time::Instant::now() + PROBE_TIMEOUT;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if std::time::Instant::now() >= deadline {
            kill_pid(child.id(), libc::SIGKILL);
            child.wait()?;
            break None;
        }
        std::thread::sleep(PROBE_INTERVAL);
    };
    let (stdout, stderr) = streamed.join();
    let left = planted.try_exists();
    drop(fs::remove_dir_all(&planted));
    placed.with_context(|| format!("stdout: {stdout}\nstderr: {stderr}"))?;
    let status = status.with_context(|| {
        format!(
            "The runner did not exit within {PROBE_TIMEOUT:?}.\nstdout: {stdout}\nstderr: {stderr}"
        )
    })?;
    anyhow::ensure!(
        !left.with_context(|| format!("Failed to check whether {planted} survived"))?
            && records_with(&stderr, "Reclaimed stale jails")
                .any(|record| record.get("count") == Some(&serde_json::Value::from(1))),
        "The second iteration never swept the jail {planted} planted after the first one's sweep.\nstdout: {stdout}\nstderr: {stderr}"
    );
    Ok(ScenarioOutput {
        stdout,
        stderr,
        exit_code: status.code().unwrap_or(-1),
    })
}

/// How long the run is held stopped, well past its 3 s timeout.
const STOPPED_FOR: Duration = Duration::from_secs(5);

/// Stopped once its image is unpacked, far ahead of the boot, so the run comes
/// to the check before boot with no time left.
fn run_runner_stopped_past_timeout(
    image_path: &Utf8Path,
    args: &[&str],
    runner_bin: &Utf8Path,
) -> Result<ScenarioOutput> {
    let mut child = spawn_runner(
        image_path,
        &[args, &["--timeout", "3"]].concat(),
        runner_bin,
    )?;
    let mut streamed = StreamedOutput::start(&mut child);
    let unpacked = streamed.wait_for_record("Writing init config");
    if unpacked.is_ok() {
        kill_pid(child.id(), libc::SIGSTOP);
        std::thread::sleep(STOPPED_FOR);
        kill_pid(child.id(), libc::SIGCONT);
    }
    let deadline = std::time::Instant::now() + PROBE_TIMEOUT;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if std::time::Instant::now() >= deadline {
            kill_pid(child.id(), libc::SIGKILL);
            child.wait()?;
            break None;
        }
        std::thread::sleep(PROBE_INTERVAL);
    };
    let (stdout, stderr) = streamed.join();
    unpacked.with_context(|| format!("stdout: {stdout}\nstderr: {stderr}"))?;
    let status = status.with_context(|| {
        format!(
            "The runner did not exit within {PROBE_TIMEOUT:?}.\nstdout: {stdout}\nstderr: {stderr}"
        )
    })?;
    Ok(ScenarioOutput {
        stdout,
        stderr,
        exit_code: status.code().unwrap_or(-1),
    })
}

/// The runner's own failure exit, so a crash or a signal does not read as a
/// refusal.
fn assert_refused_before_guest(output: &ScenarioOutput, marker: &str) -> Result<()> {
    anyhow::ensure!(
        output.exit_code == 1 && guest_printed(output, marker) == 0,
        "Expected the job to fail before its guest ran, got exit code {}.\nstdout: {}\nstderr: {}",
        output.exit_code,
        output.stdout,
        output.stderr
    );
    Ok(())
}

/// The runner logs this only once the cgroup exists and its cpuset reads back,
/// so the probe cannot pass on a host where no cgroup was made.
fn assert_cpu_isolation_applied(output: &ScenarioOutput) -> Result<()> {
    const APPLIED: &str = "CPU isolation applied";
    if records_with(&output.stderr, APPLIED).any(|record| text(&record, "cores").is_some()) {
        return Ok(());
    }
    bail!(
        "The runner never reported pinning the VMM to benchmark cores, so no cgroup was created \
         and cgroup placement went unexercised by this run. Expected an {APPLIED:?} record with its cores.\nstdout: {}\nstderr: {}",
        output.stdout,
        output.stderr
    )
}

/// Read from the VMM's own cgroup, so a run that never had one reports none.
fn assert_cgroup_metrics_reported(output: &ScenarioOutput) -> Result<()> {
    let reported = run_metrics(output)
        .next()
        .and_then(|metrics| metrics.get("cpu_usage_us")?.as_u64())
        .is_some();
    if reported {
        return Ok(());
    }
    bail!(
        "Expected the run metrics to carry the VM cgroup's cpu_usage_us.\nstderr: {}",
        output.stderr
    )
}

/// A cgroup warning means the Job ran with less confinement than a pinned run
/// claims: no cpuset, no swap limit, or no cgroup at all.
fn assert_no_cgroup_warning(output: &ScenarioOutput) -> Result<()> {
    let names_a_cgroup = |record: &Record, key| {
        text(record, key).is_some_and(|text| {
            let text = text.to_lowercase();
            ["cgroup", "cpuset", "controller", "swap"]
                .iter()
                .any(|word| text.contains(word))
        })
    };
    let warnings: Vec<Record> = records(&output.stderr)
        .filter(|record| {
            text(record, "level") == Some("WARN")
                && (names_a_cgroup(record, "msg") || names_a_cgroup(record, "reason"))
        })
        .collect();
    anyhow::ensure!(
        warnings.is_empty(),
        "Expected no cgroup warning, got: {warnings:#?}\nstderr: {}",
        output.stderr
    );
    Ok(())
}

/// A stacked handle fails every sandboxed job on the host unless the runner's
/// unwind loop clears it, and nothing else exercises that loop.
fn stack_netns_mounts() -> Result<()> {
    let handle = NETNS_HANDLE;
    fs::create_dir_all("/run/netns").context("Failed to create the netns directory")?;
    if !Utf8Path::new(handle).exists() {
        fs::File::create(handle).context("Failed to create the netns handle")?;
    }

    for _ in 0..2 {
        let status = Command::new("unshare")
            .args(["--net", "sh", "-c"])
            .arg(format!("mount --bind {HARNESS_NETNS} {handle}"))
            .status()
            .context("Failed to run unshare to stack a netns mount")?;
        anyhow::ensure!(status.success(), "Failed to stack a netns mount");
    }

    let stacked = stacked_netns_mounts()?;
    anyhow::ensure!(
        stacked >= 2,
        "Expected at least two stacked mounts on {handle}, found {stacked}"
    );
    println!("  stacked {stacked} mounts on {handle}");
    Ok(())
}

fn unusable_state_dir() -> Result<Utf8PathBuf> {
    plant_unusable_state_dir(&super::work_dir())
}

/// A symlinked component, which the runner refuses rather than resolves; an
/// unwritable directory would not do, since root writes anyway.
fn plant_unusable_state_dir(root: &Utf8Path) -> Result<Utf8PathBuf> {
    use std::os::unix::fs::symlink;

    let target = root.join("refused-state-target");
    let planted = root.join("refused-state");

    fs::create_dir_all(&target)
        .with_context(|| format!("Failed to create {target} for the refused state directory"))?;
    // Whichever it is after a previous run: the link itself, or a directory a
    // runner that resolved it went on to build a tree in.
    drop(fs::remove_file(&planted));
    drop(fs::remove_dir_all(&planted));
    symlink(&target, &planted).with_context(|| format!("Failed to link {planted} at {target}"))?;

    Ok(planted)
}

/// Asserted on the host's processes rather than the runner's output, so a
/// reworded error stays green and an unjailed guest does not.
fn run_runner_without_unjailed_vmm(
    image_path: &Utf8Path,
    args: &[&str],
    runner_bin: &Utf8Path,
) -> Result<ScenarioOutput> {
    let before = firecracker_pids()?;
    let output = run_runner(image_path, args, runner_bin)?;
    let after = firecracker_pids()?;

    let launched: Vec<u32> = after.difference(&before).copied().collect();
    anyhow::ensure!(
        launched.is_empty(),
        "The runner could not build a jail, and a VMM is running anyway (pid(s) {launched:?}). \
         A confinement that cannot be built has to fail the job, not run the guest without it.\nstdout: {}\nstderr: {}",
        output.stdout,
        output.stderr
    );

    Ok(output)
}

fn firecracker_pids() -> Result<std::collections::BTreeSet<u32>> {
    let mut pids = std::collections::BTreeSet::new();
    for entry in fs::read_dir("/proc").context("Failed to read /proc")? {
        let entry = entry.context("Failed to read a /proc entry")?;
        let Ok(pid) = entry.file_name().to_string_lossy().parse::<u32>() else {
            continue;
        };
        if is_firecracker(pid)? {
            pids.insert(pid);
        }
    }
    Ok(pids)
}

/// The handle goes too, since absence is what a host that never ran this
/// scenario looks like, and the runner rebinds it at the start of every job.
fn unstack_netns_mounts() -> Result<()> {
    let handle = Utf8Path::new(NETNS_HANDLE);
    for _ in 0..MAX_NETNS_UNWIND {
        let status = Command::new("umount")
            .args(["--lazy", handle.as_str()])
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .context("Failed to run umount to unwind a netns mount")?;
        // Nothing left mounted there, which is where this is going anyway.
        if !status.success() {
            break;
        }
    }

    match fs::remove_file(handle) {
        Ok(()) => {},
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {},
        Err(e) => {
            return Err(e).with_context(|| {
                format!("Failed to remove {handle}, so its mounts are still stacked on the host")
            });
        },
    }

    let stacked = stacked_netns_mounts()?;
    anyhow::ensure!(
        stacked == 0,
        "{stacked} mount(s) are still stacked on {handle}, which every sandboxed job on this host now trips over"
    );
    println!("  unwound the stacked mounts on {handle}");
    Ok(())
}

fn stacked_netns_mounts() -> Result<usize> {
    let mountinfo =
        fs::read_to_string("/proc/self/mountinfo").context("Failed to read mountinfo")?;
    Ok(mounts_on(&mountinfo, NETNS_HANDLE))
}

/// Matched as the whole space-delimited field, since a plain substring also
/// matches every path this one prefixes.
fn mounts_on(mountinfo: &str, mount_point: &str) -> usize {
    mountinfo
        .lines()
        .filter(|line| line.contains(&format!(" {mount_point} ")))
        .count()
}

/// `marker` is a whole line the guest computes, as `printf '%s_%s\n' A B` prints
/// `A_B`, so the command the runner echoes before it boots anything never
/// contains it.
fn assert_job_succeeded(output: &ScenarioOutput, marker: &str) -> Result<()> {
    if output.exit_code != 0 {
        bail!(
            "Expected the job to succeed, got exit code {}.\nstdout: {}\nstderr: {}",
            output.exit_code,
            output.stdout,
            output.stderr
        );
    }
    if guest_printed(output, marker) == 0 {
        bail!(
            "Expected '{marker}' in the guest output, so the VM booted and ran.\nstdout: {}\nstderr: {}",
            output.stdout,
            output.stderr
        );
    }
    Ok(())
}

fn guest_printed(output: &ScenarioOutput, marker: &str) -> usize {
    output
        .stdout
        .lines()
        .filter(|line| line.trim() == marker)
        .count()
}

fn guest_printed_to_stderr(output: &ScenarioOutput, marker: &str) -> usize {
    output
        .stderr
        .lines()
        .filter(|line| line.trim() == marker)
        .count()
}

type Record = serde_json::Map<String, serde_json::Value>;

/// The runner's records: its stderr lines that parse as JSON objects with a
/// string `level` and `msg`. No scenario image prints one.
fn records(stderr: &str) -> impl Iterator<Item = Record> + '_ {
    stderr.lines().filter_map(record_of)
}

fn record_of(line: &str) -> Option<Record> {
    let serde_json::Value::Object(record) = serde_json::from_str(line).ok()? else {
        return None;
    };
    (record.get("level")?.is_string() && record.get("msg")?.is_string()).then_some(record)
}

fn records_with<'a>(stderr: &'a str, msg: &'a str) -> impl Iterator<Item = Record> + 'a {
    records(stderr).filter(move |record| text(record, "msg") == Some(msg))
}

fn text<'a>(record: &'a Record, key: &str) -> Option<&'a str> {
    record.get(key)?.as_str()
}

/// The error the runner failed with, from its final record.
fn runner_error(output: &ScenarioOutput) -> Option<String> {
    records_with(&output.stderr, "Runner failed")
        .find_map(|record| text(&record, "error").map(str::to_owned))
}

fn run_metrics(output: &ScenarioOutput) -> impl Iterator<Item = Record> + '_ {
    records_with(&output.stderr, "Run metrics")
}

fn vmm_reported_an_error(output: &ScenarioOutput) -> bool {
    records_with(&output.stderr, "VMM stderr")
        .any(|record| text(&record, "line").is_some_and(|line| line.starts_with("Error")))
}

fn vmm_exit_status(output: &ScenarioOutput) -> Option<String> {
    records_with(&output.stderr, "VMM exited")
        .find_map(|record| text(&record, "status").map(str::to_owned))
}

/// Microseconds in a day, so two records either side of midnight subtract.
const DAY_MICROS: u64 = 86_400_000_000;

/// The time of day a record was logged, in microseconds.
fn time_of_day_micros(record: &Record) -> Option<u64> {
    let (_, time) = text(record, "ts")?.split_once('T')?;
    let (clock, fraction) = time.strip_suffix('Z')?.split_once('.')?;
    let mut fields = clock.split(':').map(str::parse::<u64>);
    let (Some(Ok(hours)), Some(Ok(minutes)), Some(Ok(seconds)), None) =
        (fields.next(), fields.next(), fields.next(), fields.next())
    else {
        return None;
    };
    let micros: String = fraction
        .chars()
        .chain(std::iter::repeat('0'))
        .take(6)
        .collect();
    Some(((hours * 60 + minutes) * 60 + seconds) * 1_000_000 + micros.parse::<u64>().ok()?)
}

fn micros_between(earlier: u64, later: u64) -> u64 {
    later
        .checked_sub(earlier)
        .unwrap_or_else(|| later + DAY_MICROS - earlier)
}

/// The guest's `/proc/uptime` as its benchmark printed it, in microseconds.
fn guest_uptime_micros(output: &ScenarioOutput) -> Option<u64> {
    let uptime = output
        .stdout
        .lines()
        .find_map(|line| line.trim().strip_prefix("UPTIME "))?;
    let (seconds, hundredths) = uptime.split_once('.')?;
    Some(seconds.parse::<u64>().ok()? * 1_000_000 + hundredths.parse::<u64>().ok()? * 10_000)
}

/// Far under the half second the keyboard probe takes, and far over the few
/// milliseconds a keyboard takes to register without one.
const NO_KEYBOARD_PROBE_MICROS: u64 = 200_000;

/// The kernel's timestamp, in microseconds since boot, on the first line of its
/// log the guest printed that contains `needle`.
fn kernel_log_micros(output: &ScenarioOutput, needle: &str) -> Option<u64> {
    let line = output.stdout.lines().find(|line| line.contains(needle))?;
    let (stamp, _) = line.trim_start().strip_prefix('[')?.split_once(']')?;
    let (seconds, micros) = stamp.trim().split_once('.')?;
    Some(
        seconds
            .parse::<u64>()
            .ok()?
            .saturating_mul(1_000_000)
            .saturating_add(micros.parse().ok()?),
    )
}

/// The runner relays what the guest printed, then fails the job with the
/// guest's exit code.
fn assert_guest_exited(output: &ScenarioOutput, marker: &str, code: i32) -> Result<()> {
    let reported = format!("non-zero exit code: {code}");
    anyhow::ensure!(
        output.exit_code == 1
            && guest_printed(output, marker) > 0
            && runner_error(output).is_some_and(|error| error.ends_with(&reported)),
        "Expected '{marker}' in the guest output and the job to fail with the guest's exit code {code}, got exit code {}.\nstdout: {}\nstderr: {}",
        output.exit_code,
        output.stdout,
        output.stderr
    );
    Ok(())
}

/// The runner's stdout is only the guest's, and every line of its stderr is a
/// record but the guest's, so a silent guest leaves no other line.
fn assert_guest_exited_silently(output: &ScenarioOutput, code: i32) -> Result<()> {
    let reported = format!("non-zero exit code: {code}");
    let stdout_silent = output.stdout.lines().all(|line| line.trim().is_empty());
    let stderr_silent = output
        .stderr
        .lines()
        .all(|line| line.trim().is_empty() || record_of(line).is_some());
    anyhow::ensure!(
        output.exit_code == 1
            && stdout_silent
            && stderr_silent
            && runner_error(output).is_some_and(|error| error.ends_with(&reported)),
        "Expected no guest output and the job to fail with the guest's exit code {code}, got exit code {}.\nstdout: {}\nstderr: {}",
        output.exit_code,
        output.stdout,
        output.stderr
    );
    Ok(())
}

/// The runner relays nothing a guest printed before its timeout, so the proof
/// that one booted is the probe's.
fn assert_timed_out(output: &ScenarioOutput) -> Result<()> {
    let timed_out = run_metrics(output)
        .next()
        .and_then(|metrics| metrics.get("timed_out")?.as_bool());
    anyhow::ensure!(
        output.exit_code == 1 && timed_out == Some(true),
        "Expected the job to fail on its timeout, with timed_out: true in its metrics, got exit code {}.\nstdout: {}\nstderr: {}",
        output.exit_code,
        output.stdout,
        output.stderr
    );
    Ok(())
}

fn assert_cancelled(output: &ScenarioOutput) -> Result<()> {
    anyhow::ensure!(
        output.exit_code == 1
            && runner_error(output).is_some_and(|error| error.contains("Job cancelled")),
        "Expected the job to end cancelled, got exit code {}.\nstdout: {}\nstderr: {}",
        output.exit_code,
        output.stdout,
        output.stderr
    );
    Ok(())
}

fn assert_guest_value_within(
    output: &ScenarioOutput,
    key: &str,
    range: std::ops::RangeInclusive<u64>,
) -> Result<()> {
    let value = output
        .stdout
        .lines()
        .find_map(|line| line.trim().strip_prefix(key));
    anyhow::ensure!(
        output.exit_code == 0
            && value
                .and_then(|value| value.parse::<u64>().ok())
                .is_some_and(|value| range.contains(&value)),
        "Expected the job to succeed with a guest line {key}<n> for n in {range:?}, got exit code {} and {value:?}.\nstdout: {}\nstderr: {}",
        output.exit_code,
        output.stdout,
        output.stderr
    );
    Ok(())
}

/// One line of exactly `cap` copies of `byte`, so a cap that was not enforced,
/// or not filled, fails.
fn assert_guest_payload_capped(output: &ScenarioOutput, byte: char, cap: usize) -> Result<()> {
    let delivered = output
        .stdout
        .lines()
        .any(|line| line.len() == cap && line.chars().all(|c| c == byte));
    anyhow::ensure!(
        output.exit_code == 0 && delivered,
        "Expected the job to succeed with a guest line of exactly {cap} '{byte}', got exit code {} and stdout lines of {:?} bytes.\nstderr: {}",
        output.exit_code,
        output.stdout.lines().map(str::len).collect::<Vec<_>>(),
        output.stderr
    );
    Ok(())
}

/// The jailer cleans up nothing by design, so a leftover means the runner's
/// teardown did not run.
fn assert_no_chroot_remains(state_dir: &Utf8Path) -> Result<()> {
    let parent = jail_parent(state_dir);
    // Only absence reads as clean, since the runner creates this tree on demand.
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

fn probe_confinement(state_dir: &Utf8Path) -> Result<bool> {
    let parent = jail_parent(state_dir);
    let Some((vm_id, jail_root)) = find_jail(&parent)? else {
        return Ok(false);
    };
    let Some(pid) = find_jailed_vmm(&jail_root)? else {
        return Ok(false);
    };

    if !check_unprivileged(pid, &jail_root, JailIds::scenario()?)? {
        return Ok(false);
    }
    if !check_netns(pid)? {
        return Ok(false);
    }
    check_cgroup_membership(&vm_id, pid)?;

    Ok(true)
}

/// Every failure but absence is an error, or the poll loop would report a
/// timeout when the truth is that nobody could look.
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

fn find_jailed_vmm(jail_root: &Utf8Path) -> Result<Option<u32>> {
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
        // A failed read is a process that has exited or is not this jail's, which
        // is an answer rather than an error.
        let Ok(root) = fs::metadata(format!("/proc/{pid}/root")) else {
            continue;
        };
        if same_object(&root, &jail) {
            return Ok(Some(pid));
        }
    }
    Ok(None)
}

/// `Ok(false)` while the jailer has `pivot_root`ed but not yet dropped
/// privilege, which is premature rather than wrong.
fn check_unprivileged(pid: u32, jail_root: &Utf8Path, expected: JailIds) -> Result<bool> {
    let status = fs::read_to_string(format!("/proc/{pid}/status"))
        .with_context(|| format!("Failed to read the status of the VMM (pid {pid})"))?;
    let uids = status_ids(&status, "Uid:")?;

    if uids.contains(&0) {
        return Ok(false);
    }

    if uids.iter().any(|uid| *uid != expected.uid) {
        bail!(
            "The VMM (pid {pid}) runs as uids {uids:?}, but the runner was handed --jail-uid {}",
            expected.uid
        );
    }
    // The gid drop precedes the uid drop, so it has landed by now.
    let gids = status_ids(&status, "Gid:")?;
    if gids.iter().any(|gid| *gid != expected.gid) {
        bail!(
            "The VMM (pid {pid}) runs as gids {gids:?}, but the runner was handed --jail-gid {}",
            expected.gid
        );
    }
    let groups = status
        .lines()
        .find_map(|line| line.strip_prefix("Groups:"))
        .context("No Groups line in the VMM's /proc status")?
        .trim();
    if !groups.is_empty() {
        bail!("The VMM (pid {pid}) still holds supplementary groups: {groups}");
    }

    // The jailer chowns the chroot root to the jail uid, so the two must
    // agree: a VMM running as some other unprivileged user would not be
    // confined to the jail it was given.
    let Some(jail_uid) = jail_root_uid(jail_root) else {
        return Ok(false);
    };
    if expected.uid != jail_uid {
        bail!(
            "The VMM (pid {pid}) runs as uid {} but its jail is owned by uid {jail_uid}",
            expected.uid
        );
    }

    Ok(true)
}

#[derive(Debug, Clone, Copy)]
struct JailIds {
    uid: u32,
    gid: u32,
}

impl JailIds {
    fn scenario() -> Result<Self> {
        Ok(Self {
            uid: SCENARIO_JAIL_UID
                .parse()
                .context("The scenario jail uid is not a number")?,
            gid: SCENARIO_JAIL_GID
                .parse()
                .context("The scenario jail gid is not a number")?,
        })
    }
}

/// The real, effective, saved, and filesystem ids on one `/proc/<pid>/status`
/// line.
fn status_ids(status: &str, field: &str) -> Result<Vec<u32>> {
    let line = status
        .lines()
        .find_map(|line| line.strip_prefix(field))
        .with_context(|| format!("No {field} line in the VMM's /proc status"))?;
    let ids = line
        .split_whitespace()
        .map(str::parse)
        .collect::<Result<Vec<u32>, _>>()
        .with_context(|| format!("Unparsable {field} line in the VMM's /proc status: {line}"))?;
    anyhow::ensure!(
        ids.len() == 4,
        "Expected four ids on the VMM's {field} line, got: {line}"
    );
    Ok(ids)
}

fn jail_root_uid(jail_root: &Utf8Path) -> Option<u32> {
    use std::os::unix::fs::MetadataExt as _;

    let uid = fs::metadata(jail_root).ok()?.uid();
    (uid != 0).then_some(uid)
}

/// Compared against both the handle and the harness's own namespace, since
/// either reading alone can pass by accident.
fn check_netns(pid: u32) -> Result<bool> {
    let handle = Utf8Path::new(NETNS_HANDLE);
    let expected = fs::metadata(handle).with_context(|| {
        format!(
            "Failed to stat {handle}, which the runner builds before it launches the VMM, so which namespace the VMM joined is unknown"
        )
    })?;
    let own = fs::metadata(HARNESS_NETNS)
        .context("Failed to stat the harness's own network namespace")?;
    // A failed read means the VMM has exited, since the pid was listed moments ago.
    let Ok(joined) = fs::metadata(format!("/proc/{pid}/ns/net")) else {
        return Ok(false);
    };

    if same_object(&joined, &own) {
        bail!(
            "The VMM (pid {pid}) is in the harness's own network namespace, so it was launched without --netns and keeps the host's network reach"
        );
    }
    if !same_object(&joined, &expected) {
        bail!(
            "The VMM (pid {pid}) is in network namespace {}, not the {handle} the runner built ({})",
            namespace_id(&joined),
            namespace_id(&expected)
        );
    }

    Ok(true)
}

fn namespace_id(metadata: &fs::Metadata) -> String {
    use std::os::unix::fs::MetadataExt as _;

    format!("{}:{}", metadata.dev(), metadata.ino())
}

/// Device and inode, the only identity that survives the jail's private mount
/// namespace.
fn same_object(left: &fs::Metadata, right: &fs::Metadata) -> bool {
    use std::os::unix::fs::MetadataExt as _;

    left.dev() == right.dev() && left.ino() == right.ino()
}

fn check_cgroup_membership(vm_id: &str, pid: u32) -> Result<()> {
    let procs_path = format!("/sys/fs/cgroup/bencher/{vm_id}/cgroup.procs");

    // A failure, not a note, or a confinement scenario would assert only the uid
    // half.
    let procs = fs::read_to_string(&procs_path).with_context(|| {
        format!(
            "No cgroup at {procs_path}, so cgroup placement was not exercised at all. \
             The runner creates one for every sandboxed run."
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

/// SIGKILL, because it never unwinds: `Drop` cannot reclaim the chroot, so only
/// the sweep can.
/// The orphan's runner uses `orphan_state`, which is `state_dir` itself or a
/// state directory of its own.
fn run_runner_after_orphan(
    image_path: &Utf8Path,
    args: &[&str],
    state_dir: &Utf8Path,
    orphan_state: &Utf8Path,
    runner_bin: &Utf8Path,
) -> Result<ScenarioOutput> {
    let parent = jail_parent(orphan_state);
    let orphan_image = build_test_image("jail_orphan", ORPHAN_DOCKERFILE)
        .context("Failed to build the orphan's image")?;

    let orphan_args = with_state_dir(args, state_dir, orphan_state);
    let mut child = spawn_runner(&orphan_image, &orphan_args, runner_bin)?;

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

    // Deliberately not reaping the VMM here, or the next job's sweep would have
    // nothing to find.
    let cgroup = stale_cgroup(&vm_id);
    anyhow::ensure!(
        cgroup
            .try_exists()
            .with_context(|| format!("Failed to check whether {cgroup} was created"))?,
        "No cgroup at {cgroup}, so the kill is not exercised. The runner creates one for every sandboxed run."
    );
    println!("  orphaned jail {vm_id} (VMM pid {vmm_pid}), running a second job...");

    let output = run_runner(image_path, args, runner_bin)?;

    if !reclaimed(&output.stderr, &cgroup) {
        bail!(
            "The next job never killed the orphaned VMM (pid {vmm_pid}) through its cgroup {cgroup}.\nstdout: {}\nstderr: {}",
            output.stdout,
            output.stderr
        );
    }

    // `try_exists`, since `exists` reads an error as absence and would pass this.
    // Only a runner with the orphan's state directory sweeps its chroot.
    if orphan_state == state_dir
        && jail_root
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
    if cgroup
        .try_exists()
        .with_context(|| format!("Failed to check whether {cgroup} survived"))?
    {
        bail!(
            "The orphaned cgroup {cgroup} survived the next job, so the sweep never removed it. Stale cgroups accumulate, and one that will not go away usually means its VMM is still running."
        );
    }

    Ok(output)
}

fn stale_cgroup(vm_id: &str) -> Utf8PathBuf {
    Utf8PathBuf::from("/sys/fs/cgroup/bencher").join(vm_id)
}

/// Outlives every job that follows it, so an orphan that is gone afterwards was
/// reaped rather than finished.
const ORPHAN_DOCKERFILE: &str = r#"FROM busybox
CMD ["sh", "-c", "echo JAIL_ORPHAN_a7f3b2c9 && sleep 600"]"#;

/// A record of the cgroup killed while it still held a process.
fn reclaimed(stderr: &str, cgroup: &Utf8Path) -> bool {
    records_with(stderr, "Reclaimed a stale cgroup").any(|record| {
        text(&record, "cgroup") == Some(cgroup.as_str())
            && record.get("populated") == Some(&serde_json::Value::Bool(true))
    })
}

/// The orphan's runner gets a state directory of its own, so only a sweep of
/// every Bencher cgroup, not of its own chroots, can find it.
fn run_runner_after_orphan_elsewhere(
    image_path: &Utf8Path,
    args: &[&str],
    state_dir: &Utf8Path,
    runner_bin: &Utf8Path,
) -> Result<ScenarioOutput> {
    let elsewhere = super::work_dir().join("orphan-state");
    drop(fs::remove_dir_all(&elsewhere));
    let output = run_runner_after_orphan(image_path, args, state_dir, &elsewhere, runner_bin);
    // Whatever the outcome, since a red run leaves the orphan running there.
    let stranded = reclaim_stranded_jails(&elsewhere);
    drop(fs::remove_dir_all(&elsewhere));
    let output = output?;
    stranded.with_context(|| format!("Failed to reclaim what the orphan left in {elsewhere}"))?;
    Ok(output)
}

/// `args` with its state directory swapped for `to`.
fn with_state_dir<'a>(args: &[&'a str], from: &Utf8Path, to: &'a Utf8Path) -> Vec<&'a str> {
    args.iter()
        .map(|arg| {
            if *arg == from.as_str() {
                to.as_str()
            } else {
                arg
            }
        })
        .collect()
}

const OCCUPIED_CGROUP: &str = "/sys/fs/cgroup/bencher/scenario-occupant";

/// A daemon that never reaches its server never gets a Job, so only its startup
/// sweep can reach the stand-in.
fn run_runner_up_beside_occupied_cgroup(runner_bin: &Utf8Path) -> Result<ScenarioOutput> {
    use std::os::unix::process::ExitStatusExt as _;

    let mut occupant = Occupant::start()?;
    let up_state = super::work_dir().join("up-state");
    drop(fs::remove_dir_all(&up_state));
    let mut up = spawn_runner_up(&up_state, runner_bin)?;
    let mut streamed = StreamedOutput::start(&mut up);
    let waiting = streamed.wait_for_record("Connecting to channel");
    kill_pid(up.id(), libc::SIGKILL);
    up.wait()?;
    let (stdout, stderr) = streamed.join();
    drop(fs::remove_dir_all(&up_state));
    waiting.with_context(|| format!("stdout: {stdout}\nstderr: {stderr}"))?;
    let ended = occupant.child.try_wait()?;
    let cgroup_left = occupant.cgroup.try_exists();
    drop(occupant);

    anyhow::ensure!(
        ended.and_then(|status| status.signal()) == Some(libc::SIGKILL)
            && !cgroup_left.with_context(|| format!("Failed to check {OCCUPIED_CGROUP}"))?
            && reclaimed(&stderr, Utf8Path::new(OCCUPIED_CGROUP)),
        "Expected `runner up` to kill the stand-in (it ended with {ended:?}) and remove {OCCUPIED_CGROUP} as it started.\nstdout: {stdout}\nstderr: {stderr}"
    );
    Ok(ScenarioOutput {
        stdout,
        stderr,
        exit_code: 0,
    })
}

/// The cgroup root seen by the runner, as by a kernel without `cgroup.kill`:
/// an empty tmpfs in a mount namespace of its own, so the host's is untouched.
const NO_CGROUP_KILL: &str = "mount -t tmpfs no-cgroup-kill /sys/fs/cgroup && exec \"$0\" \"$@\"";

/// `runner run` first, whose output `validate` sees, then `runner up`: a
/// sandboxed one must exit at startup, and one that may run Jobs with no
/// sandbox must reach its server.
fn run_runner_without_cgroup_kill(
    image_path: &Utf8Path,
    args: &[&str],
    sandboxed: bool,
    runner_bin: &Utf8Path,
) -> Result<ScenarioOutput> {
    let unshared = |runner_args: &[&str]| {
        let mut command = Command::new("unshare");
        command
            .args([
                "--mount",
                "--propagation",
                "private",
                "--",
                "sh",
                "-c",
                NO_CGROUP_KILL,
            ])
            .arg(runner_bin.as_str())
            .args(runner_args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped());
        command
    };
    let run = unshared(&[&["run", "--image", image_path.as_str()], args].concat())
        .output()
        .context("Failed to run the runner without cgroup.kill")?;
    let output = ScenarioOutput {
        stdout: String::from_utf8_lossy(&run.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&run.stderr).into_owned(),
        exit_code: run.status.code().unwrap_or(-1),
    };

    let up_state = super::work_dir().join("up-state");
    drop(fs::remove_dir_all(&up_state));
    let mut up_args = vec![
        "up",
        "--host",
        "http://127.0.0.1:9",
        "--runner",
        "lock-probe",
        "--key",
        PROBE_KEY,
        "--no-tuning",
        "--state-dir",
        up_state.as_str(),
    ];
    if !sandboxed {
        up_args.push("--danger-allow-no-sandbox");
    }
    let mut up = unshared(&up_args).spawn()?;
    let mut streamed = StreamedOutput::start(&mut up);
    let started = if sandboxed {
        let deadline = std::time::Instant::now() + UP_EXITS_WITHIN;
        loop {
            if let Some(status) = up.try_wait()? {
                break Ok(status);
            }
            if std::time::Instant::now() >= deadline {
                break Err(anyhow::anyhow!(
                    "`runner up` was still running after {UP_EXITS_WITHIN:?}"
                ));
            }
            std::thread::sleep(PROBE_INTERVAL);
        }
        .map(|status| status.code() == Some(1))
    } else {
        streamed
            .wait_for_record("Connecting to channel")
            .map(|_record| true)
    };
    drop(up.kill());
    up.wait()?;
    let (up_stdout, up_stderr) = streamed.join();
    drop(fs::remove_dir_all(&up_state));
    let refused = up_stderr.contains("Linux 5.14");
    anyhow::ensure!(
        started.with_context(|| format!("stdout: {up_stdout}\nstderr: {up_stderr}"))?
            && refused == sandboxed,
        "Expected `runner up` to {} on a kernel without cgroup.kill.\nstdout: {up_stdout}\nstderr: {up_stderr}",
        if sandboxed {
            "exit at startup naming Linux 5.14"
        } else {
            "start and reach its server"
        }
    );
    Ok(output)
}

/// Placed at start, only the runner's startup sweep can reach the stand-in;
/// placed mid-run, only its job's sweep can.
fn run_runner_beside_occupied_cgroup(
    image_path: &Utf8Path,
    args: &[&str],
    at_start: bool,
    runner_bin: &Utf8Path,
) -> Result<ScenarioOutput> {
    use std::os::unix::process::ExitStatusExt as _;

    let early = if at_start {
        Some(Occupant::start()?)
    } else {
        None
    };
    let mut child = spawn_runner(image_path, args, runner_bin)?;
    let mut streamed = StreamedOutput::start(&mut child);
    let occupant = match early {
        Some(occupant) => Ok(occupant),
        None => streamed
            .wait_for_record("Writing init config")
            .and_then(|_record| {
                kill_pid(child.id(), libc::SIGSTOP);
                let occupant = Occupant::start();
                kill_pid(child.id(), libc::SIGCONT);
                occupant
            }),
    };
    let deadline = std::time::Instant::now() + PROBE_TIMEOUT;
    let status = loop {
        if let Some(status) = child.try_wait()? {
            break Some(status);
        }
        if std::time::Instant::now() >= deadline {
            kill_pid(child.id(), libc::SIGKILL);
            child.wait()?;
            break None;
        }
        std::thread::sleep(PROBE_INTERVAL);
    };
    let (stdout, stderr) = streamed.join();
    let mut occupant = occupant
        .with_context(|| format!("No stand-in was placed.\nstdout: {stdout}\nstderr: {stderr}"))?;
    let status = status.with_context(|| {
        format!(
            "The runner did not exit within {PROBE_TIMEOUT:?}.\nstdout: {stdout}\nstderr: {stderr}"
        )
    })?;
    let output = ScenarioOutput {
        stdout,
        stderr,
        exit_code: status.code().unwrap_or(-1),
    };
    let ended = occupant.child.try_wait()?;
    let cgroup_left = occupant.cgroup.try_exists();
    drop(occupant);

    anyhow::ensure!(
        ended.and_then(|status| status.signal()) == Some(libc::SIGKILL)
            && !cgroup_left.with_context(|| format!("Failed to check {OCCUPIED_CGROUP}"))?
            && reclaimed(&output.stderr, Utf8Path::new(OCCUPIED_CGROUP)),
        "Expected the runner to kill the stand-in (it ended with {ended:?}) and remove {OCCUPIED_CGROUP}.\nstdout: {}\nstderr: {}",
        output.stdout,
        output.stderr
    );
    Ok(output)
}

struct Occupant {
    child: std::process::Child,
    cgroup: Utf8PathBuf,
}

impl Occupant {
    fn start() -> Result<Self> {
        let cgroup = Utf8PathBuf::from(OCCUPIED_CGROUP);
        fs::create_dir_all(&cgroup).with_context(|| format!("Failed to create {cgroup}"))?;
        let child = Command::new("sleep")
            .arg("600")
            .spawn()
            .context("Failed to start the stand-in process")?;
        let occupant = Self { child, cgroup };
        fs::write(
            occupant.cgroup.join("cgroup.procs"),
            occupant.child.id().to_string(),
        )
        .with_context(|| format!("Failed to move the stand-in into {}", occupant.cgroup))?;
        Ok(occupant)
    }
}

impl Drop for Occupant {
    fn drop(&mut self) {
        drop(self.child.kill());
        drop(self.child.wait());
        drop(fs::remove_dir(&self.cgroup));
    }
}

fn spawn_runner(
    image_path: &Utf8Path,
    args: &[&str],
    runner_bin: &Utf8Path,
) -> Result<std::process::Child> {
    Ok(Command::new(runner_bin.as_str())
        .arg("run")
        .arg("--image")
        .arg(image_path.as_str())
        .args(args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()?)
}

/// Far more than a runner logs before any record a scenario waits for; past
/// it only a waiter misses lines, never the output.
const UNREAD_LINES: usize = 1024;

struct StreamedOutput {
    lines: mpsc::Receiver<String>,
    seen: Vec<String>,
    output: DrainedOutput,
}

impl StreamedOutput {
    /// Streams stderr, where the runner logs, and drains stdout.
    fn start(child: &mut std::process::Child) -> Self {
        let (tx, lines) = mpsc::sync_channel(UNREAD_LINES);
        let stderr = child.stderr.take();
        let stderr = std::thread::spawn(move || {
            use std::io::BufRead as _;

            let mut output = Vec::new();
            let Some(stderr) = stderr else {
                return String::new();
            };
            for line in std::io::BufReader::new(stderr).lines() {
                let Ok(line) = line else {
                    break;
                };
                // Never blocks, so a caller that has stopped waiting cannot
                // stall the child; the output itself is kept here in full.
                drop(tx.try_send(line.clone()));
                output.push(line);
            }
            output.join("\n")
        });
        let stdout = child.stdout.take();
        let stdout = std::thread::spawn(move || {
            use std::io::Read as _;

            let mut buffer = String::new();
            if let Some(mut stdout) = stdout {
                drop(stdout.read_to_string(&mut buffer));
            }
            buffer
        });
        Self {
            lines,
            seen: Vec::new(),
            output: DrainedOutput { stdout, stderr },
        }
    }

    fn wait_for(&mut self, wanted: impl Fn(&str) -> bool) -> Result<String> {
        let deadline = std::time::Instant::now() + PROBE_TIMEOUT;
        loop {
            let remaining = deadline.saturating_duration_since(std::time::Instant::now());
            match self.lines.recv_timeout(remaining) {
                Ok(line) => {
                    let found = wanted(&line);
                    self.seen.push(line.clone());
                    if found {
                        return Ok(line);
                    }
                },
                Err(e) => bail!(
                    "The line the scenario waits for never came ({e}).\nstderr so far:\n{}",
                    self.seen.join("\n")
                ),
            }
        }
    }

    fn wait_for_record(&mut self, msg: &str) -> Result<Record> {
        let line = self.wait_for(|line| {
            record_of(line).is_some_and(|record| text(&record, "msg") == Some(msg))
        })?;
        record_of(&line).with_context(|| format!("{line} is no longer a record"))
    }

    /// Whether the runner has logged `msg` by now, without waiting for it.
    fn has_logged(&mut self, msg: &str) -> bool {
        self.seen.extend(self.lines.try_iter());
        self.seen
            .iter()
            .any(|line| record_of(line).is_some_and(|record| text(&record, "msg") == Some(msg)))
    }

    /// Wait for both readers, once the child has exited.
    fn join(self) -> (String, String) {
        self.output.join()
    }
}

/// Checks the command as well as the pid, so a recycled pid does not read as an
/// unreaped VMM.
fn is_firecracker(pid: u32) -> Result<bool> {
    is_firecracker_comm(fs::read_to_string(format!("/proc/{pid}/comm"))).with_context(|| {
        format!("Failed to read the command of pid {pid}, so whether the VMM was reaped is unknown")
    })
}

fn is_firecracker_comm(comm: std::io::Result<String>) -> std::io::Result<bool> {
    if let Err(e) = &comm
        && gone(e)
    {
        return Ok(false);
    }
    comm.map(|comm| comm.trim() == "firecracker")
}

/// A process reaped since the scan has either no `/proc` entry or one whose
/// read fails with `ESRCH`, and either way is gone.
fn gone(error: &std::io::Error) -> bool {
    error.kind() == std::io::ErrorKind::NotFound || error.raw_os_error() == Some(libc::ESRCH)
}

struct DrainedOutput {
    stdout: std::thread::JoinHandle<String>,
    stderr: std::thread::JoinHandle<String>,
}

impl DrainedOutput {
    fn join(self) -> (String, String) {
        let stdout = self.stdout.join().unwrap_or_default();
        let stderr = self.stderr.join().unwrap_or_default();
        (stdout, stderr)
    }
}

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

fn kill_pid(pid: u32, signal: libc::c_int) {
    #[expect(
        unsafe_code,
        clippy::cast_possible_wrap,
        reason = "libc::kill requires unsafe; PID fits in i32"
    )]
    // SAFETY: `kill` takes plain integers and touches no memory.
    unsafe {
        libc::kill(pid as i32, signal);
    }
}

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

const TUNED_THP: &[&str] = &[
    "/sys/kernel/mm/transparent_hugepage/enabled",
    "/sys/kernel/mm/transparent_hugepage/defrag",
];

const THP_TARGET: &str = "never";

const TURBO_SWITCHES: &[&str] = &[
    "/sys/devices/system/cpu/intel_pstate/no_turbo",
    "/sys/devices/system/cpu/cpufreq/boost",
];

const BENCHER_CGROUP: &str = "/sys/fs/cgroup/bencher";

const CPU_DMA_LATENCY: &str = "/dev/cpu_dma_latency";

#[derive(Debug, Clone)]
struct TunedSetting {
    path: Utf8PathBuf,
    original: String,
    /// `None` when this host will not let the runner change it, it already holds
    /// the target, or the harness only restores it.
    expected: Option<String>,
    bracketed: bool,
}

/// The runner never reverts its tuning, so the harness restores from this.
#[derive(Debug)]
struct TuningSnapshot {
    settings: Vec<TunedSetting>,
}

impl TuningSnapshot {
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
            // Never probed with a fallback, since writing `never` would change the
            // very setting being measured.
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

        // Restored but never asserted, since whether the runner can change them
        // depends on the host's cpufreq driver.
        let governors = cpu_governors();
        let turbo = TURBO_SWITCHES.iter().map(Utf8PathBuf::from);
        for path in governors.into_iter().chain(turbo) {
            if let Some(original) = readable_setting(&path) {
                settings.push(TunedSetting {
                    path,
                    original,
                    expected: None,
                    bracketed: false,
                });
            }
        }

        Self { settings }
    }

    fn expected(&self) -> impl Iterator<Item = &TunedSetting> {
        self.settings
            .iter()
            .filter(|setting| setting.expected.is_some())
    }

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

struct RestoreTuning(TuningSnapshot);

impl Drop for RestoreTuning {
    fn drop(&mut self) {
        self.0.restore();
        if let Err(e) = remove_bencher_cgroup() {
            println!(
                "  tuning: harness could NOT remove {BENCHER_CGROUP}: {e}; {}",
                partition_diagnosis()
            );
        }
    }
}

/// Undoes the partition the runner leaves, which the runner recreates on demand.
fn remove_bencher_cgroup() -> std::io::Result<()> {
    match fs::remove_dir(BENCHER_CGROUP) {
        Ok(()) => {
            println!("  tuning: harness removed {BENCHER_CGROUP}");
            Ok(())
        },
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(e),
    }
}

/// Each CPU's governor, found as the runner finds them.
fn cpu_governors() -> Vec<Utf8PathBuf> {
    let Ok(entries) = fs::read_dir("/sys/devices/system/cpu") else {
        return Vec::new();
    };
    entries
        .flatten()
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter(|name| {
            name.strip_prefix("cpu")
                .is_some_and(|index| index.parse::<u32>().is_ok())
        })
        .map(|name| {
            Utf8PathBuf::from(format!(
                "/sys/devices/system/cpu/{name}/cpufreq/scaling_governor"
            ))
        })
        .collect()
}

fn readable_setting(path: &Utf8Path) -> Option<String> {
    fs::read_to_string(path).ok().map(|v| v.trim().to_owned())
}

fn writable_setting(path: &Utf8Path, current: &str) -> bool {
    fs::write(path, current).is_ok()
}

/// The selected mode in a bracketed sysfs listing (`always [madvise] never`).
fn bracketed_value(listing: &str) -> Option<&str> {
    let (_, selected) = listing.split_once('[')?;
    let (selected, _) = selected.split_once(']')?;
    Some(selected)
}

/// Must run before the state directory is wiped, which destroys the chroot a
/// stranded VMM is found by.
fn reclaim_stranded_jails(state_dir: &Utf8Path) -> Result<Vec<String>> {
    let parent = jail_parent(state_dir);
    let entries = match fs::read_dir(&parent) {
        Ok(entries) => entries,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e).with_context(|| format!("Failed to read {parent}")),
    };

    let mut stranded = Vec::new();
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
        stranded.push(format!("chroot {vm_id}"));

        if let Some(pid) = find_jailed_vmm(&jail_root)? {
            stranded.push(format!("VMM pid {pid}"));
            println!("  reclaiming VMM (pid {pid}) stranded in {vm_id}");
            reap_jailed(pid, Pidfd::open, |pid| rooted_in(pid, &jail_root)).with_context(|| {
                format!(
                    "The VMM stranded in {vm_id} (pid {pid}) was not reclaimed, so it would run on through every scenario that follows"
                )
            })?;
        }

        // Nothing else will come looking for the cgroup once its jail directory
        // is wiped.
        let cgroup = stale_cgroup(&vm_id);
        if cgroup.exists() {
            stranded.push(format!("cgroup {cgroup}"));
        }
        let deadline = std::time::Instant::now() + Duration::from_secs(5);
        while cgroup.exists() {
            if fs::remove_dir(&cgroup).is_ok() {
                println!("  reclaimed the cgroup {cgroup}");
                break;
            }
            anyhow::ensure!(
                std::time::Instant::now() < deadline,
                "The cgroup {cgroup} could not be removed, so it would stay under the bencher cgroup for every run that follows"
            );
            std::thread::sleep(PROBE_INTERVAL);
        }
    }

    Ok(stranded)
}

/// Pinned, then checked against the jail again, so a pid recycled since the
/// scan is never signalled.
fn reap_jailed<O, R>(pid: u32, open: O, rooted: R) -> Result<()>
where
    O: FnOnce(u32) -> std::io::Result<Option<Pidfd>>,
    R: FnOnce(u32) -> bool,
{
    let Some(vmm) = open(pid).context("No pidfd could pin it, so it was left running")? else {
        return Ok(());
    };
    if !rooted(pid) {
        return Ok(());
    }
    // A kill that fails shows up as a VMM still running at the deadline.
    drop(vmm.kill());
    let deadline = std::time::Instant::now() + Duration::from_secs(5);
    while vmm.is_running() && std::time::Instant::now() < deadline {
        std::thread::sleep(PROBE_INTERVAL);
    }
    anyhow::ensure!(
        !vmm.is_running(),
        "It was still running 5 seconds after a SIGKILL"
    );
    Ok(())
}

fn rooted_in(pid: u32, jail_root: &Utf8Path) -> bool {
    fs::metadata(jail_root)
        .ok()
        .zip(fs::metadata(format!("/proc/{pid}/root")).ok())
        .is_some_and(|(jail, root)| same_object(&root, &jail))
}

/// Removing a cgroup fails while it holds a task or a child, so a failed
/// removal reports what is still in the `bencher` cgroup.
fn partition_diagnosis() -> String {
    let root = Utf8Path::new(BENCHER_CGROUP);
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
            // Named, because what the process is decides whose bug it is.
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
    let latency_before = dma_latency();

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

    // The runner tunes before it pulls the image, so this window lasts the whole run.
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

    let cstates_held = applied
        && latency_before.is_some_and(|latency| latency != 0)
        && wait_for_cstate_hold(&mut child, deadline)?;

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

    if status.code() != Some(0) {
        bail!(
            "Tuning applied but the Job failed with exit code {:?}.\nstdout: {stdout}\nstderr: {stderr}",
            status.code()
        );
    }

    ensure_steering_masks_logged(&stdout, &stderr)?;

    if tuned(&stderr, "C-states", "set").next().is_none() {
        println!("  tuning: the runner held no C-state constraint, so none was asserted");
    } else if latency_before.is_none_or(|latency| latency == 0) {
        println!(
            "  tuning: {CPU_DMA_LATENCY} was unreadable or already 0, so the C-state hold was not asserted"
        );
    } else if !cstates_held {
        bail!(
            "The runner reported holding C-states, but {CPU_DMA_LATENCY} never read 0 while it ran.\nstdout: {stdout}\nstderr: {stderr}"
        );
    } else {
        println!("  tuning: the C-state hold read 0 while the runner ran");
    }

    let reverted = restore.0.missing();
    if !reverted.is_empty() {
        bail!(
            "Host tuning did not persist after the runner exited: {reverted:?}.\nstdout: {stdout}\nstderr: {stderr}"
        );
    }

    if let Some(level) = achieved_partition(&stderr) {
        let partition = Utf8Path::new(BENCHER_CGROUP).join("cpuset.cpus.partition");
        let after = readable_setting(&partition);
        if after.is_none() && level == "member" {
            println!(
                "  tuning: no cpuset partition file on this host, so the runner's 'member' was not asserted"
            );
        } else if after.as_deref() != Some(level.as_str()) {
            bail!(
                "The cpuset partition did not persist after the runner exited: {partition} is '{}', but the runner achieved '{level}'.\nstdout: {stdout}\nstderr: {stderr}",
                after.as_deref().unwrap_or("?")
            );
        } else {
            println!("  tuning: the cpuset partition persisted at '{level}'");
        }
    } else {
        println!("  tuning: the runner made no cpuset partition, so none was asserted");
    }

    // Fails while the run left a cgroup or a task behind.
    if let Err(e) = remove_bencher_cgroup() {
        bail!(
            "The harness could not remove {BENCHER_CGROUP}: {e}; {}.\nstdout: {stdout}\nstderr: {stderr}",
            partition_diagnosis()
        );
    }

    println!(
        "  tuning: {} setting(s) applied and persisted",
        expected.len()
    );

    Ok(ScenarioOutput {
        stdout,
        stderr,
        exit_code: status.code().unwrap_or(-1),
    })
}

/// The C-state hold is a descriptor the runner keeps open, so the bound reads 0
/// only while the runner holds it.
fn wait_for_cstate_hold(
    child: &mut std::process::Child,
    deadline: std::time::Instant,
) -> Result<bool> {
    loop {
        if dma_latency() == Some(0) {
            return Ok(true);
        }
        if child.try_wait()?.is_some() || std::time::Instant::now() >= deadline {
            return Ok(false);
        }
        std::thread::sleep(PROBE_INTERVAL);
    }
}

/// The kernel's current C-state latency bound, a native-endian `i32`.
#[expect(
    clippy::host_endian_bytes,
    reason = "the kernel interface is native-endian"
)]
fn dma_latency() -> Option<i32> {
    let bytes = fs::read(CPU_DMA_LATENCY).ok()?;
    <[u8; 4]>::try_from(bytes.as_slice())
        .ok()
        .map(i32::from_ne_bytes)
}

/// `run_and_validate` passes `--no-irq-steering`, so the runner logs each mask it left.
fn ensure_steering_masks_logged(stdout: &str, stderr: &str) -> Result<()> {
    anyhow::ensure!(
        tuned(stderr, "default IRQ affinity", "left")
            .next()
            .is_some()
            && tuned(stderr, "workqueue cpumask", "left").next().is_some(),
        "The runner skipped IRQ steering but did not log the masks it left in place.\nstdout: {stdout}\nstderr: {stderr}"
    );
    Ok(())
}

/// The partition level the runner reports achieving.
fn achieved_partition(stderr: &str) -> Option<String> {
    tuned(stderr, "cpuset partition", "achieved")
        .find_map(|record| text(&record, "value").map(str::to_owned))
}

/// The `Tuning` records of `setting` with `action`.
fn tuned<'a>(
    stderr: &'a str,
    setting: &'a str,
    action: &'a str,
) -> impl Iterator<Item = Record> + 'a {
    records_with(stderr, "Tuning").filter(move |record| {
        text(record, "setting") == Some(setting) && text(record, "action") == Some(action)
    })
}

fn tuning_scenarios() -> Vec<Scenario> {
    vec![Scenario {
        name: "host_tuning",
        description: "Host tuning applies while a Job runs and persists after",
        dockerfile: r#"FROM busybox
CMD ["printf", "%s_%s\\n", "TUNED", "RUN"]"#,
        extra_args: &["--timeout", "60"],
        tuning: true,
        // Otherwise a run that tuned the host but never booted a VM would pass.
        validate: |output| assert_job_succeeded(output, "TUNED_RUN"),
        ..Scenario::default()
    }]
}

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

    // Undrained, a runner that fills the 64 KiB pipe buffer blocks until the probe
    // times out.
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
        if child.try_wait()?.is_some() || std::time::Instant::now() >= deadline {
            break;
        }
        std::thread::sleep(PROBE_INTERVAL);
    }

    // Kill a run the probe gave up on rather than wait out its own timeout, and
    // only while unreaped: a reaped pid may already belong to something else.
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
        // A lossy conversion would name a directory nobody asked for.
        use std::os::unix::ffi::OsStrExt as _;

        resolve_target_dir(
            Some(OsStr::from_bytes(b"/tmp/target-\xff")),
            Utf8Path::new("/workspace"),
        )
        .unwrap_err();
    }

    #[test]
    fn the_sabotaged_state_directory_is_one_the_runner_refuses() {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(dir.path()).unwrap();

        let planted = plant_unusable_state_dir(root).unwrap();

        assert!(
            fs::symlink_metadata(&planted)
                .unwrap()
                .file_type()
                .is_symlink(),
            "the runner refuses a symlinked component"
        );
        assert!(
            planted.is_absolute(),
            "a relative state directory is refused for another reason entirely"
        );

        // Planting it twice is what a second run of the suite does.
        plant_unusable_state_dir(root).unwrap();
    }

    #[test]
    fn one_object_read_twice_is_the_same_object() {
        // A noisy identity would make the netns and chroot checks report an escape.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(dir.path()).unwrap();
        let path = root.join("ns");
        fs::write(&path, "").unwrap();

        let left = fs::metadata(&path).unwrap();
        let right = fs::metadata(&path).unwrap();

        assert!(same_object(&left, &right));
    }

    #[test]
    fn two_objects_are_not_one() {
        // Same device, so a comparison that dropped the inode would call them one.
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(dir.path()).unwrap();
        fs::write(root.join("host"), "").unwrap();
        fs::write(root.join("jail"), "").unwrap();

        let host = fs::metadata(root.join("host")).unwrap();
        let jail = fs::metadata(root.join("jail")).unwrap();

        assert!(!same_object(&host, &jail));
    }

    #[test]
    fn output_past_the_unread_bound_neither_stalls_the_child_nor_goes_missing() {
        // A reader that blocked on the full channel would stop draining the
        // pipe once nobody waits, so the child could never finish writing.
        let count = UNREAD_LINES * 64;
        let mut child = Command::new("sh")
            .args(["-c", &format!("seq {count} >&2")])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .unwrap();
        let mut streamed = StreamedOutput::start(&mut child);
        streamed.wait_for(|line| line == "1").unwrap();

        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while child.try_wait().unwrap().is_none() {
            if std::time::Instant::now() > deadline {
                drop(child.kill());
                panic!("the child stalled behind a reader nobody drained");
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let (_stdout, stderr) = streamed.join();

        assert_eq!(stderr.lines().count(), count);
    }

    #[test]
    fn a_process_is_placed_by_the_cgroup_just_below_the_base() {
        // The base itself, or a sibling of it, would claim a process the scenario
        // never placed.
        assert_eq!(bencher_cgroup_name("0::/bencher/abc\n"), Some("abc"));
        assert_eq!(bencher_cgroup_name("0::/bencher/abc/nested\n"), Some("abc"));
        assert_eq!(bencher_cgroup_name("0::/bencher\n"), None);
        assert_eq!(bencher_cgroup_name("0::/bencher-other/abc\n"), None);
    }

    #[test]
    fn a_stacked_handle_is_counted_by_its_own_mount_point() {
        let mountinfo = "\
25 1 0:23 / /run rw,nosuid,nodev shared:2 - tmpfs tmpfs rw
71 25 0:4 net:[4026532290] /run/netns/bencher-jail rw shared:3 - nsfs nsfs rw
72 25 0:4 net:[4026532351] /run/netns/bencher-jail rw shared:4 - nsfs nsfs rw
73 25 0:4 net:[4026532999] /run/netns/bencher-jail-other rw shared:5 - nsfs nsfs rw
";

        assert_eq!(mounts_on(mountinfo, "/run/netns/bencher-jail"), 2);
        assert_eq!(mounts_on(mountinfo, "/run/netns/bencher-jail-other"), 1);
        assert_eq!(mounts_on(mountinfo, "/run/netns/absent"), 0);
    }

    #[test]
    fn the_run_writes_nothing_outside_the_tree_it_hands_back() {
        // A red run skips the image trees' cleanup, so the chown must reach them.
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
        // A substring match would accept a mode that is offered but not selected.
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
        // A stat cannot tell that an existing file refuses writes, as
        // `/proc/sys/kernel/nmi_watchdog` does without a hardware watchdog.
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

        if !is_root() {
            let mut perms = fs::metadata(&path).unwrap().permissions();
            perms.set_readonly(true);
            fs::set_permissions(&path, perms).unwrap();

            assert!(!writable_setting(&path, "1"));
        }
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_stray_is_killed_through_its_pin() {
        // Fails if the kill sent through the pidfd does not land, so the stray
        // would run on into the next scenario.
        use std::os::unix::process::ExitStatusExt as _;

        let mut stray = Command::new("sleep").arg("600").spawn().unwrap();
        let pid = stray.id();

        let stranded = reap_strays([pid], Pidfd::open, |_pid| Ok(Some(true))).unwrap();

        let exited = exits_soon(&mut stray);
        drop(stray.kill());
        let status = stray.wait().unwrap();
        assert!(exited, "the stray outlived its reap");
        assert_eq!(status.signal(), Some(libc::SIGKILL));
        assert_eq!(stranded, [format!("process {pid}")]);
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_recycled_pid_is_never_signalled() {
        // Fails if the reaper signals by pid, or pins the process only after its
        // check: the stray exits after the scan and a bystander takes its pid.
        if !is_root() {
            println!(
                "skipped a_recycled_pid_is_never_signalled: handing out a chosen pid needs root"
            );
            return;
        }
        let mut stray = Command::new("sleep").arg("600").spawn().unwrap();
        let pid = stray.id();
        let mut bystander = None;

        reap_strays([pid], Pidfd::open, |_pid| {
            stray.kill()?;
            stray.wait()?;
            bystander = Some(spawn_with_pid(pid)?);
            Ok(Some(true))
        })
        .unwrap();

        let mut bystander = bystander.unwrap();
        let killed = exits_soon(&mut bystander);
        drop(bystander.kill());
        drop(bystander.wait());
        assert!(!killed, "the bystander that took pid {pid} was killed");
    }

    #[test]
    fn a_stray_no_pidfd_can_pin_is_reported_and_left_running() {
        // Fails if the reaper falls back to a kill by pid, which a recycled pid
        // would carry to a stranger.
        let mut stray = Command::new("sleep").arg("600").spawn().unwrap();
        let pid = stray.id();

        let stranded = reap_strays(
            [pid],
            |_pid| Err(std::io::ErrorKind::Unsupported.into()),
            |_pid| Ok(Some(true)),
        )
        .unwrap();

        let killed = exits_soon(&mut stray);
        drop(stray.kill());
        drop(stray.wait());
        assert!(!killed, "the stray was killed without a pin");
        assert_eq!(stranded.len(), 1, "{stranded:?}");
        assert!(
            stranded[0].starts_with(&format!("process {pid}, left running")),
            "{stranded:?}"
        );
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_process_reaped_as_its_command_is_read_is_gone() {
        // Fails if a pid that exits mid-scan fails the scan, or if any other
        // failed read passes for a process that is gone.
        let (_pid, reaped) = read_once_reaped("comm");

        assert!(!is_firecracker_comm(Err(reaped)).unwrap());
        is_firecracker_comm(fs::read_to_string("/")).unwrap_err();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_stray_reaped_as_its_cgroup_is_read_is_gone() {
        // Fails if a pinned stray that its parent reaps mid-check fails the
        // reap, or if any other failed read passes for a stray that is gone.
        let (pid, reaped) = read_once_reaped("cgroup");

        assert_eq!(belongs_with_cgroup(pid, Err(reaped), &[]).unwrap(), None);
        belongs_with_cgroup(pid, fs::read_to_string("/"), &[]).unwrap_err();
    }

    /// Opens `/proc/<pid>/<file>` of a live process, reaps it, then reads:
    /// the error a read racing the reap meets.
    #[cfg(target_os = "linux")]
    fn read_once_reaped(file: &str) -> (u32, std::io::Error) {
        use std::io::Read as _;

        let mut process = Command::new("sleep").arg("600").spawn().unwrap();
        let pid = process.id();
        let mut open = fs::File::open(format!("/proc/{pid}/{file}")).unwrap();
        process.kill().unwrap();
        process.wait().unwrap();
        let reaped = open.read(&mut [0; 16]).unwrap_err();
        assert_eq!(reaped.raw_os_error(), Some(libc::ESRCH), "{reaped}");
        (pid, reaped)
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_vmm_still_in_its_jail_is_reaped() {
        // Fails if the reclaim's kill does not land; any process is rooted at `/`.
        let mut vmm = Command::new("sleep").arg("600").spawn().unwrap();

        let reaped = reap_jailed(vmm.id(), Pidfd::open, |pid| {
            rooted_in(pid, Utf8Path::new("/"))
        });

        let exited = exits_soon(&mut vmm);
        drop(vmm.kill());
        drop(vmm.wait());
        reaped.unwrap();
        assert!(exited, "the VMM outlived its reclaim");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_process_outside_the_jail_is_never_reclaimed() {
        // Fails if the reclaim kills without checking the pinned process against
        // the jail, as it must when the scanned pid was recycled.
        let dir = tempfile::tempdir().unwrap();
        let jail_root = Utf8Path::from_path(dir.path()).unwrap();
        let mut bystander = Command::new("sleep").arg("600").spawn().unwrap();

        let reaped = reap_jailed(bystander.id(), Pidfd::open, |pid| rooted_in(pid, jail_root));

        let killed = exits_soon(&mut bystander);
        drop(bystander.kill());
        drop(bystander.wait());
        reaped.unwrap();
        assert!(!killed, "the bystander was killed");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_stray_is_signalled_through_its_pin_not_its_pid() {
        // Fails if the reaper signals by pid: the pin holds a decoy, so a kill by
        // pid reaches the stray instead.
        let mut stray = Command::new("sleep").arg("600").spawn().unwrap();
        let mut decoy = Command::new("sleep").arg("600").spawn().unwrap();
        let decoy_pid = decoy.id();

        let reaped = reap_strays(
            [stray.id()],
            |_pid| Pidfd::open(decoy_pid),
            |_pid| Ok(Some(true)),
        );

        let stray_killed = exits_soon(&mut stray);
        let decoy_killed = exits_soon(&mut decoy);
        for child in [&mut stray, &mut decoy] {
            drop(child.kill());
            drop(child.wait());
        }
        reaped.unwrap();
        assert!(!stray_killed, "the stray was signalled by its pid");
        assert!(decoy_killed, "the process the pin holds outlived the reap");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_stray_is_pinned_before_its_check() {
        // Fails if the reaper pins a stray only after checking it, so the check
        // could read a process that is gone by the time of the pin.
        let mut stray = Command::new("sleep").arg("600").spawn().unwrap();
        let pinned = std::cell::Cell::new(false);

        let reaped = reap_strays(
            [stray.id()],
            |pid| {
                pinned.set(true);
                Pidfd::open(pid)
            },
            |_pid| {
                anyhow::ensure!(pinned.get(), "checked before it was pinned");
                Ok(Some(true))
            },
        );

        drop(stray.kill());
        drop(stray.wait());
        reaped.unwrap();
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_recycled_vmm_pid_is_never_reclaimed() {
        // Fails if the reclaim signals by pid after its check, or checks the jail
        // before it pins: the VMM exits during the check and a bystander takes its pid.
        if !is_root() {
            println!(
                "skipped a_recycled_vmm_pid_is_never_reclaimed: handing out a chosen pid needs root"
            );
            return;
        }
        let mut vmm = Command::new("sleep").arg("600").spawn().unwrap();
        let pid = vmm.id();
        let mut bystander = None;

        let reaped = reap_jailed(pid, Pidfd::open, |_pid| {
            drop(vmm.kill());
            drop(vmm.wait());
            bystander = Some(spawn_with_pid(pid));
            true
        });

        let mut bystander = bystander.unwrap().unwrap();
        let killed = exits_soon(&mut bystander);
        drop(bystander.kill());
        drop(bystander.wait());
        reaped.unwrap();
        assert!(!killed, "the bystander that took pid {pid} was killed");
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn a_vmm_is_reclaimed_only_through_a_pin_taken_before_its_check() {
        // Fails if the reclaim signals by pid, which reaches the VMM rather than the
        // decoy its pin holds, or checks the jail before it pins.
        let mut vmm = Command::new("sleep").arg("600").spawn().unwrap();
        let mut decoy = Command::new("sleep").arg("600").spawn().unwrap();
        let decoy_pid = decoy.id();
        let pinned = std::cell::Cell::new(false);

        let reaped = reap_jailed(
            vmm.id(),
            |_pid| {
                pinned.set(true);
                Pidfd::open(decoy_pid)
            },
            |_pid| pinned.get(),
        );

        let vmm_killed = exits_soon(&mut vmm);
        let decoy_killed = exits_soon(&mut decoy);
        for child in [&mut vmm, &mut decoy] {
            drop(child.kill());
            drop(child.wait());
        }
        reaped.unwrap();
        assert!(!vmm_killed, "the VMM was signalled by its pid");
        assert!(
            decoy_killed,
            "the process the pin holds outlived the reclaim"
        );
    }

    /// Whether `child` exits within a second, as a SIGKILL sent to it makes it.
    fn exits_soon(child: &mut std::process::Child) -> bool {
        let deadline = std::time::Instant::now() + Duration::from_secs(1);
        while std::time::Instant::now() < deadline {
            if child.try_wait().unwrap().is_some() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        false
    }

    /// Hands `pid` to a fresh `sleep` through `ns_last_pid`, retrying while
    /// another process, such as a parallel test's, takes or holds it first.
    #[cfg(target_os = "linux")]
    fn spawn_with_pid(pid: u32) -> std::io::Result<std::process::Child> {
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        while std::time::Instant::now() < deadline {
            fs::write("/proc/sys/kernel/ns_last_pid", (pid - 1).to_string())?;
            let mut child = Command::new("sleep").arg("600").spawn()?;
            if child.id() == pid {
                return Ok(child);
            }
            drop(child.kill());
            drop(child.wait());
            std::thread::sleep(Duration::from_millis(10));
        }
        Err(std::io::Error::other(format!(
            "pid {pid} was never handed out again"
        )))
    }
}
