//! Jailed Firecracker process management.
#![expect(clippy::print_stderr, reason = "process management prints diagnostics")]

use std::fs::File;
use std::os::unix::process::CommandExt as _;
use std::process::{Child, Command};
use std::time::Duration;

use camino::Utf8Path;

use crate::firecracker::client::FirecrackerClient;
use crate::firecracker::config::{Action, ActionType};
use crate::firecracker::error::{FirecrackerError, PreExec};
use crate::jail::{JailFile, JailUser, VmId};

/// How long to wait for the Firecracker API socket to appear.
///
/// This budget used to cover Firecracker starting up on its own. It now also
/// has to cover the jailer building a chroot, copying a multi-megabyte exec
/// file into it, creating device nodes, chowning, `pivot_root`, and `setns`,
/// on a host that may be busy running someone else's benchmark. Five seconds
/// left no margin for that.
///
/// Widening costs nothing on the failure path: a jailer that dies is detected
/// the moment it exits rather than at the deadline, so the only thing this
/// affects is how patient the runner is with a slow host.
const API_SOCKET_TIMEOUT: Duration = Duration::from_secs(30);

/// Everything needed to spawn the VMM under the jailer.
#[derive(Debug)]
pub struct JailedSpawn<'a> {
    /// The jailer binary, which runs as root and execs Firecracker in place.
    pub jailer_bin: &'a Utf8Path,
    /// The staged Firecracker binary, outside the jail.
    ///
    /// The jailer copies this into the chroot itself, and refuses to write
    /// over a multiply linked destination, so it is neither placed in the
    /// chroot by hand nor hardlinked there. Its base name determines the
    /// chroot layout, so it is fixed rather than incidental.
    pub exec_file: &'a Utf8Path,
    /// The jailer `--id`, which is also the chroot name and the cgroup name.
    pub vm_id: &'a VmId,
    /// The unprivileged uid and gid the VMM drops to.
    pub jail_user: JailUser,
    /// The jailer `--chroot-base-dir`.
    pub chroot_base_dir: &'a Utf8Path,
    /// Handle of the empty network namespace the VMM joins.
    pub netns: &'a Utf8Path,
    /// The REST API socket, in both views.
    pub api_socket: &'a JailFile,
    /// Firecracker process log level.
    pub log_level: &'a str,
    /// Cores the stderr reader thread is pinned to.
    pub housekeeping_cores: Vec<usize>,
    /// Pre-opened `cgroup.procs`, when a cgroup exists to place the VMM in.
    pub cgroup_procs: Option<File>,
}

/// A running, jailed Firecracker process.
pub struct FirecrackerProcess {
    child: Child,
    api_socket: JailFile,
    stderr_thread: Option<std::thread::JoinHandle<()>>,
}

impl FirecrackerProcess {
    /// Start Firecracker under the jailer and wait for its API socket.
    ///
    /// The jailer builds the chroot, creates `/dev/kvm`, drops to the
    /// unprivileged jail uid, joins the empty network namespace, and execs
    /// Firecracker. Neither `--daemonize` nor `--new-pid-ns` is passed:
    /// both make the jailer fork, which would break the pid identity that
    /// [`Self::pid`] and [`Self::kill_after_grace_period`] rely on. Without
    /// them the jailer execs in place and the child pid stays the VMM.
    ///
    /// No cgroup flags are passed either. The runner owns the cgroup end to
    /// end because it has to create, verify, read metrics from, and remove it,
    /// and the cpuset partition specifically needs read-back verification that
    /// the jailer's write-once interface cannot provide.
    ///
    /// A background thread reads stderr and prints lines prefixed with
    /// `[firecracker]`. The jailer inherits that stdio and its own diagnostics
    /// appear under the same prefix. It inherits no environment: see
    /// [`jailer_command`].
    pub fn start(spawn: JailedSpawn<'_>) -> Result<Self, FirecrackerError> {
        let args = jailer_args(&spawn);

        // Destructured rather than read field by field so that adding a field
        // without deciding what it does here is a build error.
        let JailedSpawn {
            jailer_bin,
            exec_file: _,
            vm_id: _,
            jail_user: _,
            chroot_base_dir: _,
            netns: _,
            api_socket,
            log_level: _,
            housekeeping_cores,
            cgroup_procs,
        } = spawn;

        let mut command = jailer_command(jailer_bin, &args);

        // Remembered before the descriptor is moved into the closure, because
        // a spawn that failed cannot say whether it was the exec or the write
        // that ran first, and this is what tells the operator to look at the
        // cgroup at all.
        let pre_exec = if cgroup_procs.is_some() {
            PreExec::CgroupPlacement
        } else {
            PreExec::Nothing
        };

        if let Some(procs) = cgroup_procs {
            // Cgroup membership is inherited across `fork` and survives
            // `execve`, so writing to the pre-opened descriptor here places
            // the child before it execs the jailer, and Firecracker inherits
            // the membership through the jailer's own exec. Doing it here
            // rather than after spawn also means the VMM never boots its API
            // or touches memory on the wrong cores first.
            #[expect(
                unsafe_code,
                reason = "cgroup placement must happen between fork and exec"
            )]
            // SAFETY: the closure runs in the forked child before `execve`,
            // where only async-signal-safe work is permitted. It performs a
            // single `write` of a fixed one-byte buffer on a descriptor that
            // was opened before the fork: no allocation, no path resolution,
            // and no locks. A failed write is reported to the parent over the
            // CLOEXEC pipe and surfaces as a failed `spawn`.
            unsafe {
                command.pre_exec(move || place_in_cgroup(&procs));
            }
        }

        let mut child = command.spawn().map_err(|e| FirecrackerError::Spawn {
            path: jailer_bin.to_owned(),
            pre_exec,
            source: e,
        })?;

        // Spawn a thread to read stderr line-by-line
        let stderr = child.stderr.take().ok_or(FirecrackerError::Stdio(
            "stderr was piped but not available",
        ))?;
        let stderr_thread = std::thread::spawn(move || {
            use std::io::BufRead as _;

            // Pin to housekeeping cores to avoid benchmark interference
            if let Err(e) = crate::cpu::pin_current_thread(&housekeeping_cores) {
                eprintln!("Warning: failed to pin stderr reader thread: {e}");
            }
            let reader = std::io::BufReader::new(stderr);
            for line in reader.lines() {
                match line {
                    Ok(line) => eprintln!("[firecracker] {line}"),
                    Err(_) => break,
                }
            }
        });

        let mut process = Self {
            child,
            api_socket: api_socket.clone(),
            stderr_thread: Some(stderr_thread),
        };

        process.wait_for_ready(API_SOCKET_TIMEOUT)?;

        Ok(process)
    }

    /// Wait for the API socket, giving up the moment the jailer dies.
    ///
    /// Watching the child is what keeps a jailer that failed outright from
    /// presenting as a socket timeout. A bad `--netns`, an unwritable
    /// `--chroot-base-dir`, or a refused `mknod` makes it exit immediately,
    /// and polling for a socket that will never appear would report
    /// `SocketNotReady` and point at Firecracker instead of at the jailer's
    /// own diagnostics, which are already on stderr.
    fn wait_for_ready(&mut self, timeout: Duration) -> Result<(), FirecrackerError> {
        let start = std::time::Instant::now();
        let poll_interval = Duration::from_millis(50);

        while start.elapsed() < timeout {
            if self.client().try_ready()? {
                return Ok(());
            }
            if let Some(status) = self.exited() {
                return Err(FirecrackerError::JailedProcessExited { status });
            }
            std::thread::sleep(poll_interval);
        }

        // Once more before giving up. The loop sleeps between polls, so a
        // process that exits during the last sleep would otherwise be reported
        // as a socket that never became ready, which points at Firecracker
        // taking too long when the truth is that it is gone. That confusion is
        // the entire reason this error exists.
        if let Some(status) = self.exited() {
            return Err(FirecrackerError::JailedProcessExited { status });
        }

        Err(FirecrackerError::SocketNotReady(timeout))
    }

    /// The status of the jailed process, if it has already exited.
    ///
    /// `try_wait` can fail in its own right, and that failure is dropped on
    /// purpose. The question is only whether the process is already gone, and a
    /// question that could not be answered is not a death: reporting
    /// [`FirecrackerError::JailedProcessExited`] would name a status nobody
    /// read, and reporting the `try_wait` error itself would replace a verdict
    /// about the VMM with one about the runner's own bookkeeping. Treating an
    /// unpollable child as still running costs nothing, because every caller
    /// asks inside a bounded loop that ends in its own error.
    fn exited(&mut self) -> Option<std::process::ExitStatus> {
        // No status, rather than no answer: the distinction is the doc comment
        // above, and it is a decision rather than a discarded `Result`.
        self.child.try_wait().unwrap_or(None)
    }

    /// Get a client for the Firecracker REST API.
    pub fn client(&self) -> FirecrackerClient {
        FirecrackerClient::new(self.api_socket.socket())
    }

    /// Get the PID of the Firecracker process.
    pub fn pid(&self) -> u32 {
        self.child.id()
    }

    /// Send Ctrl+Alt+Del and wait for graceful shutdown, then SIGKILL.
    pub fn kill_after_grace_period(&mut self, grace: Duration) {
        // Try graceful shutdown via API
        let action = Action {
            action_type: ActionType::SendCtrlAltDel,
        };
        drop(self.client().put_action(&action));

        // Wait for the process to exit gracefully
        let start = std::time::Instant::now();
        let poll_interval = Duration::from_millis(100);
        while start.elapsed() < grace {
            if let Ok(Some(_)) = self.child.try_wait() {
                self.join_stderr_thread();
                return;
            }
            std::thread::sleep(poll_interval);
        }

        // Force kill if still running. Unlike the readiness wait above, a child
        // that exits during the final sleep is not a missed case here: nothing
        // was reaped, so the pid is still reserved by the child and cannot have
        // been recycled, `kill` sends a signal that a zombie simply ignores, and
        // `kill` then reaps it and joins the reader. That is precisely what the
        // loop would have done, so there is no verdict to get wrong.
        self.kill();
    }

    /// Force-kill the Firecracker process.
    pub fn kill(&mut self) {
        drop(self.child.kill());
        drop(self.child.wait());
        self.join_stderr_thread();
    }

    /// Clean up socket files.
    ///
    /// The chroot itself is reclaimed wholesale by the jail teardown; this
    /// only keeps the socket from outliving the process within a job.
    ///
    /// Unlinks through the host view. Unlinking has no `sun_path` limit, so
    /// the socket view buys nothing here and costs a dependency on a
    /// descriptor still being open. This runs from `Drop`, where a future
    /// reordering could close that descriptor first, and where the failure
    /// would not be an error but the deletion of whatever file inherited the
    /// number.
    pub fn cleanup(&self) {
        drop(std::fs::remove_file(self.api_socket.host().as_path()));
    }

    /// Join the stderr reader thread if it exists.
    fn join_stderr_thread(&mut self) {
        if let Some(handle) = self.stderr_thread.take() {
            drop(handle.join());
        }
    }
}

impl Drop for FirecrackerProcess {
    fn drop(&mut self) {
        self.kill();
        self.cleanup();
    }
}

/// Build the jailer's argument vector.
///
/// Split out from the spawn so it can be asserted without a host that can
/// boot a VM. Getting this wrong fails every job at startup and, before the
/// VM boots, produces errors that point at Firecracker rather than at the
/// command line that caused them.
fn jailer_args(spawn: &JailedSpawn<'_>) -> Vec<String> {
    vec![
        "--id".to_owned(),
        spawn.vm_id.to_string(),
        "--exec-file".to_owned(),
        spawn.exec_file.to_string(),
        "--uid".to_owned(),
        spawn.jail_user.uid().to_string(),
        "--gid".to_owned(),
        spawn.jail_user.gid().to_string(),
        "--chroot-base-dir".to_owned(),
        spawn.chroot_base_dir.to_string(),
        "--netns".to_owned(),
        spawn.netns.to_string(),
        // No cgroup flags of any kind, and neither --daemonize nor
        // --new-pid-ns: see `FirecrackerProcess::start`.
        "--".to_owned(),
        // `--id` is deliberately not forwarded: the jailer already passes it
        // to Firecracker, which rejects the duplicate with DuplicateArgument
        // and fails every job at startup.
        "--api-sock".to_owned(),
        // The chroot view. Firecracker binds this after it has been confined,
        // so the host path would name a directory it cannot reach.
        spawn.api_socket.chroot().as_str().to_owned(),
        "--level".to_owned(),
        spawn.log_level.to_owned(),
    ]
}

/// Build the jailer invocation, with the environment the VMM is entitled to.
///
/// Which is none of it. Everything either binary needs arrives as an argument:
/// the exec file, the chroot base, the netns handle, the uid and gid, the API
/// socket, and the log level. Neither reads `PATH` (the jailer is named by a
/// path here, and it execs `--exec-file` by path inside the chroot), a locale,
/// `TMPDIR`, or `RUST_LOG`, so an empty environment costs nothing.
///
/// What it buys is that the runner's own environment stops at this line. The
/// runner takes its API key from `BENCHER_RUNNER_KEY`, and the VMM writes to
/// vsock channels the guest reads, so a Firecracker that inherited the
/// environment would hold a credential inside the sandbox with a way back out.
/// The bundled jailer scrubs the environment itself, but the runner falls back
/// to whatever `jailer` the host has installed and checks no version, so
/// confinement cannot depend on which binary was found.
fn jailer_command(jailer_bin: &Utf8Path, args: &[String]) -> Command {
    let mut command = Command::new(jailer_bin);
    command
        .args(args)
        .env_clear()
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::piped());
    command
}

/// Join the calling task to the cgroup behind a pre-opened `cgroup.procs`.
///
/// The kernel reads `0` as the calling task, which is why no pid has to be
/// formatted (and no allocation performed) inside the forked child.
fn place_in_cgroup(mut procs: &File) -> std::io::Result<()> {
    use std::io::Write as _;

    procs.write_all(b"0")
}

#[cfg(test)]
mod tests {
    use camino::Utf8Path;

    use super::*;
    use crate::jail::JailPaths;

    fn spawn_for<'a>(jail: &'a JailPaths, vm_id: &'a VmId) -> JailedSpawn<'a> {
        JailedSpawn {
            jailer_bin: Utf8Path::new("/tmp/work/jailer"),
            exec_file: Utf8Path::new("/tmp/work/firecracker"),
            vm_id,
            jail_user: JailUser::default(),
            chroot_base_dir: Utf8Path::new("/var/lib/bencher-runner/jail"),
            netns: Utf8Path::new("/run/netns/bencher-jail"),
            api_socket: jail.api_socket(),
            log_level: "Warning",
            housekeeping_cores: Vec::new(),
            cgroup_procs: None,
        }
    }

    fn args() -> Vec<String> {
        let (_dir, jail) = jail_in_tmpdir();
        jailer_args(&spawn_for(&jail, &vm_id()))
    }

    /// A stand-in identity for tests.
    fn vm_id() -> VmId {
        VmId::from_chroot_name("vm-1".to_owned()).unwrap()
    }

    /// The jail root has to exist: the paths hold a descriptor on it.
    fn jail_in_tmpdir() -> (tempfile::TempDir, JailPaths) {
        let dir = tempfile::tempdir().unwrap();
        let root = Utf8Path::from_path(dir.path()).unwrap();
        let jail = JailPaths::new(root).unwrap();
        (dir, jail)
    }

    /// A process around a plain child, for the readiness verdict.
    ///
    /// The jail is what the socket view names, so it has to outlive the process
    /// this returns.
    fn process_around(child: Child, jail: &JailPaths) -> FirecrackerProcess {
        FirecrackerProcess {
            child,
            api_socket: jail.api_socket().clone(),
            stderr_thread: None,
        }
    }

    /// The value following `flag`, if the flag is present.
    fn value_of<'a>(args: &'a [String], flag: &str) -> Option<&'a str> {
        let index = args.iter().position(|arg| arg == flag)?;
        args.get(index + 1).map(String::as_str)
    }

    #[test]
    fn confinement_flags_are_all_present() {
        let args = args();

        assert_eq!(value_of(&args, "--id"), Some("vm-1"));
        assert_eq!(
            value_of(&args, "--exec-file"),
            Some("/tmp/work/firecracker")
        );
        assert_eq!(value_of(&args, "--uid"), Some("61016"));
        assert_eq!(value_of(&args, "--gid"), Some("61016"));
        assert_eq!(
            value_of(&args, "--chroot-base-dir"),
            Some("/var/lib/bencher-runner/jail")
        );
        assert_eq!(value_of(&args, "--netns"), Some("/run/netns/bencher-jail"));
    }

    #[test]
    fn id_appears_exactly_once() {
        // The jailer passes `--id` to Firecracker itself. Forwarding it again
        // after the separator makes Firecracker reject the duplicate and fail
        // every job at startup.
        let args = args();

        assert_eq!(
            args.iter().filter(|arg| *arg == "--id").count(),
            1,
            "--id must be given to the jailer only: {args:?}"
        );
    }

    #[test]
    fn only_the_api_socket_and_level_are_forwarded() {
        let args = args();
        let separator = args
            .iter()
            .position(|arg| arg == "--")
            .expect("the jailer needs a -- separator before Firecracker's own arguments");

        assert_eq!(
            args.get(separator + 1..),
            Some(
                [
                    "--api-sock".to_owned(),
                    "/api.sock".to_owned(),
                    "--level".to_owned(),
                    "Warning".to_owned(),
                ]
                .as_slice()
            )
        );
    }

    #[test]
    fn the_api_socket_is_the_chroot_view() {
        // Firecracker binds the socket after it has been confined, so it must
        // receive the path as it will exist inside the chroot. The host view
        // names a directory the jailed process cannot reach.
        let (_dir, jail) = jail_in_tmpdir();
        let args = jailer_args(&spawn_for(&jail, &vm_id()));

        assert_eq!(value_of(&args, "--api-sock"), Some("/api.sock"));
        let jail_root = jail.root().as_str();
        assert!(
            !args.iter().any(|arg| arg.contains(jail_root)),
            "no host-side jail path may reach the jailed process: {args:?}"
        );
    }

    #[test]
    fn a_child_that_is_still_running_is_not_reported_exited() {
        // The readiness wait gives up the moment this says the process is gone,
        // so an inverted verdict turns every slow boot into
        // `JailedProcessExited`.
        let (_dir, jail) = jail_in_tmpdir();
        let child = Command::new("/bin/sh")
            .args(["-c", "sleep 30"])
            .spawn()
            .unwrap();

        let mut process = process_around(child, &jail);

        assert!(process.exited().is_none());
        // `Drop` kills and reaps the child.
    }

    #[test]
    fn a_child_that_exited_is_reported_exited() {
        let (_dir, jail) = jail_in_tmpdir();
        let mut child = Command::new("/bin/sh")
            .args(["-c", "exit 3"])
            .spawn()
            .unwrap();
        child.wait().unwrap();

        let mut process = process_around(child, &jail);

        assert_eq!(process.exited().and_then(|status| status.code()), Some(3));
    }

    #[test]
    fn the_jailed_process_inherits_no_environment() {
        // The failure this prevents: the runner takes its API key from
        // `BENCHER_RUNNER_KEY`, so an inherited environment puts that key in
        // Firecracker's own `environ`, and a compromised VMM writes it out over
        // the vsock channels it is supposed to write results to. The bundled
        // jailer scrubs the environment itself; a jailer found on the host is
        // taken without a version check, so this cannot rely on it.
        //
        // `/usr/bin/env` stands in for the jailer: it prints the environment it
        // was given, and its own run with the environment left alone is the
        // control that keeps this from passing on a binary that prints nothing.
        let env_bin = Utf8Path::new("/usr/bin/env");
        let inherited = Command::new(env_bin).output().unwrap();
        assert!(
            !inherited.stdout.is_empty(),
            "the test process has no environment to inherit, so nothing here is proven"
        );

        let cleared = jailer_command(env_bin, &[])
            .stdout(std::process::Stdio::piped())
            .output()
            .unwrap();

        assert!(
            cleared.stdout.is_empty(),
            "the jailed process must inherit nothing, got: {}",
            String::from_utf8_lossy(&cleared.stdout)
        );
    }

    #[test]
    fn no_cgroup_or_forking_flags_are_passed() {
        // The runner owns the cgroup end to end, and both --daemonize and
        // --new-pid-ns make the jailer fork, which breaks the pid identity the
        // process management relies on.
        let args = args();

        for forbidden in [
            "--cgroup",
            "--parent-cgroup",
            "--cgroup-version",
            "--daemonize",
            "--new-pid-ns",
        ] {
            assert!(
                !args.iter().any(|arg| arg == forbidden),
                "{forbidden} must not be passed: {args:?}"
            );
        }
    }
}
