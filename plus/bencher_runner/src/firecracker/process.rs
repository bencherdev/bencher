//! Jailed Firecracker process management.

use std::fs::File;
use std::net::Shutdown;
use std::os::fd::OwnedFd;
use std::os::unix::net::UnixStream;
use std::os::unix::process::CommandExt as _;
use std::process::{Child, Command};
use std::sync::Arc;
use std::time::Duration;

use camino::Utf8Path;
use slog::{Logger, info, warn};

use crate::firecracker::client::FirecrackerClient;
use crate::firecracker::config::{Action, ActionType};
use crate::firecracker::error::{FirecrackerError, PreExec};
use crate::jail::{JailFile, JailUser, PinnedSocket, VmId};

/// Generous because it covers the jailer building the chroot on a busy host; a
/// jailer that dies is caught when it exits, not at this deadline.
const API_SOCKET_TIMEOUT: Duration = Duration::from_secs(30);

/// Far above any line an honest VMM or jailer writes, yet short enough that a
/// cut line's record stays one journald line: 7,168 bytes escaped at up to 6
/// bytes each is 43,008, and the rest of the record about 140, under 49,152.
const STDERR_LINE_CAP: usize = bencher_logger::FIELD_CAP;

#[derive(Debug)]
pub struct JailedSpawn<'a> {
    pub log: Logger,
    /// The jailer binary, which runs as root and execs Firecracker in place.
    pub jailer_bin: &'a Utf8Path,
    /// Outside the jail, since the jailer copies it in itself and refuses a
    /// hardlinked destination.
    pub exec_file: &'a Utf8Path,
    /// The jailer `--id`, which is also the chroot name and the cgroup name.
    pub vm_id: &'a VmId,
    pub jail_user: JailUser,
    pub chroot_base_dir: &'a Utf8Path,
    /// Handle of the empty network namespace the VMM joins.
    pub netns: &'a Utf8Path,
    pub api_socket: &'a JailFile,
    pub log_level: &'a str,
    /// Cores the stderr reader thread is pinned to.
    pub housekeeping_cores: Vec<usize>,
    /// Pre-opened `cgroup.procs`, when a cgroup exists to place the VMM in.
    pub cgroup_procs: Option<File>,
}

/// A running, jailed Firecracker process.
pub struct FirecrackerProcess {
    jailed: JailedChild,
    /// Pinned before any guest code ran, so every API call after that reaches
    /// Firecracker's own socket whatever the jail has done to its name.
    api: PinnedSocket,
}

/// Kills and reaps the jailer's child on drop, guarding it before there is an
/// API socket to pin.
struct JailedChild {
    log: Logger,
    child: Child,
    api_socket: JailFile,
    stderr_thread: Option<StderrReader>,
}

/// Logs the VMM's stderr from a thread that dropping this stops and joins, so
/// it must outlive the VMM.
struct StderrReader {
    socket: Arc<UnixStream>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl FirecrackerProcess {
    /// Start Firecracker under the jailer and wait for its API socket.
    pub fn start(spawn: JailedSpawn<'_>) -> Result<Self, FirecrackerError> {
        let args = jailer_args(&spawn);

        // Destructured rather than read field by field so that adding a field
        // without deciding what it does here is a build error.
        let JailedSpawn {
            log,
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

        let pre_exec = if cgroup_procs.is_some() {
            PreExec::CgroupPlacement
        } else {
            PreExec::Nothing
        };

        let command = jailer_command(jailer_bin, &args, cgroup_procs);
        // Guarded at once, since `Child::drop` neither kills nor reaps and any
        // error below would otherwise leave the VMM running.
        let mut jailed = JailedChild::spawn(&log, command, api_socket, housekeeping_cores)
            .map_err(|e| FirecrackerError::Spawn {
                path: jailer_bin.to_owned(),
                pre_exec,
                source: e,
            })?;

        jailed.wait_for_ready(API_SOCKET_TIMEOUT)?;
        // Now, while only Firecracker has run: readiness proves it bound this
        // socket, and no guest code runs before `InstanceStart`.
        let api = PinnedSocket::pin(api_socket.socket()).map_err(FirecrackerError::PinApiSocket)?;

        Ok(Self { jailed, api })
    }

    /// Get a client for the Firecracker REST API.
    pub fn client(&self) -> FirecrackerClient<'_> {
        FirecrackerClient::new(&self.jailed.log, self.api.path())
    }

    /// Get the PID of the Firecracker process.
    pub fn pid(&self) -> u32 {
        self.jailed.child.id()
    }

    /// Send Ctrl+Alt+Del and wait for graceful shutdown, then SIGKILL.
    ///
    /// The grace covers the request too, so a VMM that never answers cannot
    /// stretch it.
    pub fn kill_after_grace_period(&mut self, grace: Duration) {
        let deadline = std::time::Instant::now() + grace;
        // Try graceful shutdown via API
        let action = Action {
            action_type: ActionType::SendCtrlAltDel,
        };
        drop(self.client().put_action_until(&action, deadline));

        // Wait for the process to exit gracefully
        let poll_interval = Duration::from_millis(100);
        while std::time::Instant::now() < deadline {
            if let Ok(Some(_)) = self.jailed.child.try_wait() {
                self.jailed.join_stderr_thread();
                return;
            }
            std::thread::sleep(poll_interval);
        }

        // Force kill if still running
        self.jailed.kill();
    }
}

impl JailedChild {
    fn spawn(
        log: &Logger,
        command: Command,
        api_socket: &JailFile,
        housekeeping_cores: Vec<usize>,
    ) -> std::io::Result<Self> {
        let vmm = log.clone();
        // A VMM does not choose its records' level.
        Self::spawn_with(
            log,
            command,
            api_socket,
            housekeeping_cores,
            move |line, cut| {
                if cut {
                    info!(vmm, "VMM stderr"; "line" => line, "cut" => true);
                } else {
                    info!(vmm, "VMM stderr"; "line" => line);
                }
            },
        )
    }

    /// Hands the child one end of a socket pair as its stderr and reads the
    /// other on a thread, since a pipe gives the owner no way to end that read.
    fn spawn_with<F>(
        log: &Logger,
        mut command: Command,
        api_socket: &JailFile,
        housekeeping_cores: Vec<usize>,
        on_line: F,
    ) -> std::io::Result<Self>
    where
        F: FnMut(String, bool) + Send + 'static,
    {
        let (writer, stderr) = StderrReader::spawn(log, housekeeping_cores, on_line)?;
        command.stderr(OwnedFd::from(writer));
        let child = command.spawn()?;
        Ok(Self {
            log: log.clone(),
            child,
            api_socket: api_socket.clone(),
            stderr_thread: Some(stderr),
        })
    }

    /// Gives up the moment the jailer dies, so its failure is not reported as a
    /// socket timeout.
    fn wait_for_ready(&mut self, timeout: Duration) -> Result<(), FirecrackerError> {
        let start = std::time::Instant::now();
        let poll_interval = Duration::from_millis(50);

        while start.elapsed() < timeout {
            if FirecrackerClient::new(&self.log, self.api_socket.socket()).try_ready()? {
                return Ok(());
            }
            if let Some(status) = self.exited() {
                return Err(FirecrackerError::JailedProcessExited { status });
            }
            std::thread::sleep(poll_interval);
        }

        // Once more, since a process that exited during the last sleep would
        // otherwise be reported as a timeout.
        if let Some(status) = self.exited() {
            return Err(FirecrackerError::JailedProcessExited { status });
        }

        Err(FirecrackerError::SocketNotReady(timeout))
    }

    /// A failed `try_wait` counts as still running, since every caller polls in
    /// a bounded loop that ends in its own error.
    fn exited(&mut self) -> Option<std::process::ExitStatus> {
        self.child.try_wait().unwrap_or(None)
    }

    /// Force-kill the Firecracker process.
    fn kill(&mut self) {
        drop(self.child.kill());
        drop(self.child.wait());
        self.join_stderr_thread();
    }

    /// Clean up socket files.
    ///
    /// Unlinks through the host view, since from `Drop` the socket view may name
    /// a closed descriptor and delete an unrelated file.
    fn cleanup(&self) {
        drop(std::fs::remove_file(self.api_socket.host().as_path()));
    }

    /// Call only once the VMM is reaped, since stopping the reader cuts a live
    /// VMM's log.
    fn join_stderr_thread(&mut self) {
        drop(self.stderr_thread.take());
    }
}

impl StderrReader {
    /// Returns the end the child writes to.
    fn spawn<F>(
        log: &Logger,
        housekeeping_cores: Vec<usize>,
        mut on_line: F,
    ) -> std::io::Result<(UnixStream, Self)>
    where
        F: FnMut(String, bool) + Send + 'static,
    {
        let (writer, reader) = UnixStream::pair()?;
        let socket = Arc::new(reader);
        let read = Arc::clone(&socket);
        let log = log.clone();
        let thread = std::thread::spawn(move || {
            let _stop = StopReading(&read);
            // Pin to housekeeping cores to avoid benchmark interference
            if let Err(e) = crate::cpu::pin_current_thread(&housekeeping_cores) {
                warn!(log, "Thread not pinned to housekeeping cores"; "thread" => "VMM stderr reader", "error" => %e);
            }
            forward_capped_lines(&*read, &mut on_line);
        });
        Ok((
            writer,
            Self {
                socket,
                thread: Some(thread),
            },
        ))
    }
}

impl Drop for StderrReader {
    /// Ends the read once what the VMM wrote has been read, even if a process
    /// it left behind still holds its end, so the join cannot hang.
    fn drop(&mut self) {
        drop(self.socket.shutdown(Shutdown::Read));
        if let Some(thread) = self.thread.take() {
            drop(thread.join());
        }
    }
}

/// Shuts down the read side when the reader thread ends, by a break or a
/// panic, so the VMM's later writes fail with `EPIPE` rather than block.
struct StopReading<'a>(&'a UnixStream);

impl Drop for StopReading<'_> {
    fn drop(&mut self) {
        drop(self.0.shutdown(Shutdown::Read));
    }
}

impl Drop for JailedChild {
    fn drop(&mut self) {
        self.kill();
        self.cleanup();
    }
}

/// Split out from the spawn so it can be tested without a host that boots VMs.
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
        // No cgroup flags, since the runner owns the cgroup, and no --daemonize
        // or --new-pid-ns, since both fork and break the pid identity.
        "--".to_owned(),
        // `--id` is not forwarded: the jailer already passes it, and
        // Firecracker rejects the duplicate.
        "--api-sock".to_owned(),
        spawn.api_socket.chroot().as_str().to_owned(),
        "--level".to_owned(),
        spawn.log_level.to_owned(),
    ]
}

/// Clears the environment so `BENCHER_RUNNER_KEY` never reaches the VMM,
/// whichever jailer binary was found.
fn jailer_command(jailer_bin: &Utf8Path, args: &[String], cgroup_procs: Option<File>) -> Command {
    let mut command = Command::new(jailer_bin);
    command
        .args(args)
        .env_clear()
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null());
    if let Some(procs) = cgroup_procs {
        #[expect(
            unsafe_code,
            reason = "cgroup placement must happen between fork and exec"
        )]
        // SAFETY: between fork and exec the closure only writes a fixed byte to
        // a descriptor opened before the fork, which is async-signal-safe.
        unsafe {
            command.pre_exec(move || place_in_cgroup(&procs));
        }
    }
    command
}

/// Writes `0`, which the kernel reads as the calling task, so nothing is
/// formatted or allocated in the forked child.
fn place_in_cgroup(mut procs: &File) -> std::io::Result<()> {
    use std::io::Write as _;

    procs.write_all(b"0")
}

/// Hands each line to `on_line` without its newline until EOF, keeping at most
/// `STDERR_LINE_CAP` bytes of a line, with whether it was cut.
fn forward_capped_lines<R, F>(stderr: R, mut on_line: F)
where
    R: std::io::Read,
    F: FnMut(String, bool),
{
    use std::io::{BufRead as _, Read as _};

    let cap = STDERR_LINE_CAP;
    let mut stderr = std::io::BufReader::new(stderr);
    let mut line = Vec::new();
    while let Ok(1..) = stderr
        .by_ref()
        .take(cap as u64 + 1)
        .read_until(b'\n', &mut line)
    {
        let cut = line.len() > cap && !line.ends_with(b"\n");
        if cut {
            line.truncate(cap);
        } else if line.ends_with(b"\n") {
            line.pop();
            if line.ends_with(b"\r") {
                line.pop();
            }
        }
        // Lossy, since the cut can split a character and a field is UTF-8.
        on_line(String::from_utf8_lossy(&line).into_owned(), cut);
        if cut {
            // Only now, so a line that never ends still shows its start.
            drop(stderr.skip_until(b'\n'));
        }
        line.clear();
    }
}

#[cfg(test)]
mod tests {
    use camino::{Utf8Path, Utf8PathBuf};

    use super::*;
    use crate::jail::JailPaths;
    use crate::log::discard;

    fn spawn_for<'a>(jail: &'a JailPaths, vm_id: &'a VmId) -> JailedSpawn<'a> {
        JailedSpawn {
            log: discard(),
            jailer_bin: Utf8Path::new("/tmp/work/jailer"),
            exec_file: Utf8Path::new("/tmp/work/firecracker"),
            vm_id,
            // Distinct, so a gid fed from the uid cannot pass.
            jail_user: JailUser::new(4242, 4243).unwrap(),
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

    /// The jail has to outlive the guard this returns, since the socket view
    /// names its descriptor.
    fn process_around(child: Child, jail: &JailPaths) -> JailedChild {
        JailedChild {
            log: discard(),
            child,
            api_socket: jail.api_socket().clone(),
            stderr_thread: None,
        }
    }

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
        assert_eq!(value_of(&args, "--uid"), Some("4242"));
        assert_eq!(value_of(&args, "--gid"), Some("4243"));
        assert_eq!(
            value_of(&args, "--chroot-base-dir"),
            Some("/var/lib/bencher-runner/jail")
        );
        assert_eq!(value_of(&args, "--netns"), Some("/run/netns/bencher-jail"));
    }

    #[test]
    fn id_appears_exactly_once() {
        // Prevents forwarding `--id`, which Firecracker rejects as a duplicate, failing every job.
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
        // Prevents passing the host view, which the confined Firecracker cannot reach.
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
    fn a_swapped_api_socket_name_is_never_followed() {
        // Prevents the runner, as root, following an `api.sock` swapped for a
        // link to a host socket.
        use std::io::{Read as _, Write as _};
        use std::os::unix::fs::symlink;
        use std::os::unix::net::{UnixListener, UnixStream};

        let (dir, jail) = jail_in_tmpdir();
        let api_sock = jail.api_socket().host().as_path();
        let vmm = UnixListener::bind(api_sock).unwrap();
        let trap_path = Utf8Path::from_path(dir.path()).unwrap().join("trap.sock");
        let trap = UnixListener::bind(&trap_path).unwrap();
        trap.set_nonblocking(true).unwrap();
        let child = Command::new("/bin/sh")
            .args(["-c", "sleep 30"])
            .spawn()
            .unwrap();
        let process = FirecrackerProcess {
            jailed: process_around(child, &jail),
            api: PinnedSocket::pin(jail.api_socket().socket()).unwrap(),
        };

        std::fs::remove_file(api_sock).unwrap();
        symlink(&trap_path, api_sock).unwrap();

        let request = std::thread::scope(|scope| {
            let answered = scope.spawn(|| {
                let (mut stream, _) = vmm.accept().unwrap();
                let mut request = [0u8; 512];
                let read = stream.read(&mut request).unwrap();
                if read > 0 {
                    stream
                        .write_all(b"HTTP/1.1 204 No Content\r\n\r\n")
                        .unwrap();
                }
                String::from_utf8_lossy(request.get(..read).unwrap_or_default()).into_owned()
            });
            let sent = process.client().put_action(&Action {
                action_type: ActionType::SendCtrlAltDel,
            });
            if sent.is_err() {
                drop(UnixStream::connect(process.api.path().as_str()));
            }
            answered.join().unwrap()
        });

        assert!(
            request.starts_with("PUT /actions "),
            "the request must reach the socket that was pinned, got: {request:?}"
        );
        assert_eq!(
            trap.accept().map(|_| ()).unwrap_err().kind(),
            std::io::ErrorKind::WouldBlock,
            "nothing may connect through the swapped name"
        );
    }

    #[test]
    fn a_child_that_is_still_running_is_not_reported_exited() {
        // Prevents an inverted verdict that turns every slow boot into `JailedProcessExited`.
        let (_dir, jail) = jail_in_tmpdir();
        let child = Command::new("/bin/sh")
            .args(["-c", "sleep 30"])
            .spawn()
            .unwrap();

        let mut process = process_around(child, &jail);

        assert!(process.exited().is_none());
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
    fn a_process_left_holding_stderr_cannot_hold_the_teardown() {
        // Prevents the teardown waiting on a stderr that a process the VMM left
        // behind keeps open.
        let (_dir, jail) = jail_in_tmpdir();
        let mut command = Command::new("/bin/sh");
        // The shell exits at once, and the sleep it leaves behind keeps stderr.
        command
            .args(["-c", "sleep 8 &"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null());
        let reading = Arc::new(());
        let token = Arc::clone(&reading);
        let mut jailed = JailedChild::spawn_with(
            &discard(),
            command,
            jail.api_socket(),
            Vec::new(),
            move |_line, _cut| {
                drop(Arc::clone(&token));
            },
        )
        .unwrap();
        let started = std::time::Instant::now();
        while jailed.exited().is_none() && started.elapsed() < Duration::from_secs(3) {
            std::thread::sleep(Duration::from_millis(10));
        }

        let started = std::time::Instant::now();
        jailed.kill();
        let held = started.elapsed();

        assert!(
            held < Duration::from_secs(3),
            "the teardown must not wait on stderr, held {held:?}"
        );
        assert_eq!(
            Arc::strong_count(&reading),
            1,
            "the reader must have finished when the teardown returns"
        );
    }

    #[test]
    fn stderr_the_vmm_wrote_is_still_read() {
        // Prevents the VMM's stderr bypassing the reader, which loses its log
        // or mixes it into the runner's own.
        let (_dir, jail) = jail_in_tmpdir();
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "echo one >&2; echo two >&2"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null());
        let lines = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = Arc::clone(&lines);
        let mut jailed = JailedChild::spawn_with(
            &discard(),
            command,
            jail.api_socket(),
            Vec::new(),
            move |line, cut| {
                sink.lock().unwrap().push((line, cut));
            },
        )
        .unwrap();
        let started = std::time::Instant::now();
        while jailed.exited().is_none() && started.elapsed() < Duration::from_secs(3) {
            std::thread::sleep(Duration::from_millis(10));
        }

        jailed.kill();

        assert_eq!(
            Arc::strong_count(&lines),
            1,
            "the reader must have finished"
        );
        assert_eq!(
            *lines.lock().unwrap(),
            [("one".to_owned(), false), ("two".to_owned(), false)]
        );
    }

    #[test]
    fn a_reader_that_panics_fails_the_vmm_writes() {
        // Prevents a reader that panics leaving the VMM blocked on a full
        // socket instead of failing its writes.
        let (_dir, jail) = jail_in_tmpdir();
        let mut command = Command::new("/bin/sh");
        command
            .args(["-c", "echo first >&2; head -c 4194304 /dev/zero >&2"])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null());
        let mut jailed = JailedChild::spawn_with(
            &discard(),
            command,
            jail.api_socket(),
            Vec::new(),
            |_line, _cut| {
                panic!("the reader fails");
            },
        )
        .unwrap();
        let started = std::time::Instant::now();
        while jailed.exited().is_none() && started.elapsed() < Duration::from_secs(3) {
            std::thread::sleep(Duration::from_millis(10));
        }
        let exited = jailed.exited();

        jailed.kill();

        assert!(
            exited.is_some(),
            "the VMM must not block on a stderr nobody reads"
        );
    }

    #[test]
    fn a_stderr_line_over_the_cap_is_cut_and_the_next_line_kept() {
        // Prevents a cut that keeps more than the cap or drops the lines after
        // it, and a line that is not UTF-8 ending the read.
        let (_dir, jail) = jail_in_tmpdir();
        let mut command = Command::new("/bin/sh");
        command
            .args([
                "-c",
                "{ head -c 1048576 /dev/zero | tr '\\0' a; echo; printf '\\377\\n'; echo two; } >&2",
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null());
        let lines = Arc::new(std::sync::Mutex::new(Vec::new()));
        let sink = Arc::clone(&lines);
        let mut jailed = JailedChild::spawn_with(
            &discard(),
            command,
            jail.api_socket(),
            Vec::new(),
            move |line, cut| {
                sink.lock().unwrap().push((line, cut));
            },
        )
        .unwrap();
        let started = std::time::Instant::now();
        while jailed.exited().is_none() && started.elapsed() < Duration::from_secs(3) {
            std::thread::sleep(Duration::from_millis(10));
        }

        jailed.kill();

        let lines = lines.lock().unwrap();
        let [(cut, true), (invalid, false), (next, false)] = lines.as_slice() else {
            panic!(
                "expected the cut line and the next 2 lines, got lines of {:?} bytes",
                lines
                    .iter()
                    .map(|(line, cut)| (line.len(), cut))
                    .collect::<Vec<_>>()
            );
        };
        assert_eq!(cut.len(), STDERR_LINE_CAP, "the line keeps its first bytes");
        assert_eq!(*invalid, char::REPLACEMENT_CHARACTER.to_string());
        assert_eq!(next, "two");
    }

    #[test]
    fn a_vmm_line_is_one_record() {
        // Fails if a VMM line is printed rather than logged, cut past what one
        // journald line holds once escaped, or followed by a record of its own.
        let (_dir, jail) = jail_in_tmpdir();
        let mut command = Command::new("/bin/sh");
        command
            .args([
                "-c",
                "{ printf 'head\\000<3>forged\\n\\r<2>x\\n'; head -c 20480 /dev/zero | tr '\\0' '\\001'; echo; } >&2",
            ])
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null());
        let sink = Sink::default();
        let log = bencher_logger::runner_logger_to(sink.clone());
        let mut jailed = JailedChild::spawn(&log, command, jail.api_socket(), Vec::new()).unwrap();
        let started = std::time::Instant::now();
        while jailed.exited().is_none() && started.elapsed() < Duration::from_secs(3) {
            std::thread::sleep(Duration::from_millis(10));
        }

        jailed.kill();

        let written = sink.0.lock().unwrap().clone();
        let records: Vec<serde_json::Value> = written
            .split(|&byte| byte == b'\n')
            .filter(|line| !line.is_empty())
            .map(|line| {
                assert!(line.len() < 49_152, "a record of {} bytes", line.len());
                serde_json::from_slice(line).unwrap()
            })
            .collect();
        let [head, carriage, cut] = records.as_slice() else {
            panic!("expected 3 records, got {records:?}");
        };
        assert_eq!(head["msg"], "VMM stderr");
        assert_eq!(head["line"], "head\0<3>forged");
        assert_eq!(carriage["line"], "\r<2>x");
        assert_eq!(head.get("cut"), None);
        assert_eq!(cut["cut"], true);
        assert_eq!(cut["line"], "\u{1}".repeat(STDERR_LINE_CAP));
    }

    /// Shared, so the test can read what the logger wrote.
    #[derive(Clone, Default)]
    struct Sink(Arc<std::sync::Mutex<Vec<u8>>>);

    impl std::io::Write for Sink {
        fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
            self.0
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    #[test]
    fn a_cut_line_is_handed_on_before_the_line_ends() {
        // Prevents a reader that holds a whole line before it cuts it, which a
        // line that never ends grows without bound.
        let sent = std::cell::Cell::new(0);
        let handed_on = std::cell::Cell::new(false);
        let mut first_after = None;
        let mut lines = Vec::new();
        forward_capped_lines(
            EndlessLine {
                sent: &sent,
                handed_on: &handed_on,
                tail: b"\ntwo\n",
            },
            |line, _cut| {
                handed_on.set(true);
                first_after.get_or_insert(sent.get());
                lines.push(line);
            },
        );

        assert!(
            first_after.is_some_and(|sent| sent < 1024 * 1024),
            "the cut line waited for {first_after:?} bytes"
        );
        assert_eq!(lines.first().map(String::len), Some(STDERR_LINE_CAP));
    }

    /// Writes one line until a line is handed on, or for 64 MiB, then ends it
    /// with `tail`.
    struct EndlessLine<'a> {
        sent: &'a std::cell::Cell<usize>,
        handed_on: &'a std::cell::Cell<bool>,
        tail: &'static [u8],
    }

    impl std::io::Read for EndlessLine<'_> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if self.handed_on.get() || self.sent.get() >= 64 * 1024 * 1024 {
                let n = self.tail.len().min(buf.len());
                buf[..n].copy_from_slice(&self.tail[..n]);
                self.tail = &self.tail[n..];
                return Ok(n);
            }
            buf.fill(b'a');
            self.sent.set(self.sent.get() + buf.len());
            Ok(buf.len())
        }
    }

    #[test]
    fn the_jailed_process_inherits_no_environment() {
        // Prevents `BENCHER_RUNNER_KEY` reaching a VMM that could write it out over vsock.
        let env_bin = Utf8Path::new("/usr/bin/env");
        let inherited = Command::new(env_bin).output().unwrap();
        assert!(
            !inherited.stdout.is_empty(),
            "the test process has no environment to inherit, so nothing here is proven"
        );

        let cleared = jailer_command(env_bin, &[], None)
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
    #[expect(clippy::print_stderr, reason = "a skipped test says why")]
    fn the_vmm_is_in_its_cgroup_before_it_execs() {
        // Prevents placing the VMM after it execs, when it has already started on the wrong cores.
        use std::os::unix::fs::PermissionsExt as _;

        if crate::jail::current_euid() != 0 {
            eprintln!("skipped the_vmm_is_in_its_cgroup_before_it_execs: a cgroup needs root");
            return;
        }
        let scratch = ScratchCgroup::new();
        let (dir, jail) = jail_in_tmpdir();
        let root = Utf8Path::from_path(dir.path()).unwrap();
        let reported = root.join("cgroup");
        let stand_in = root.join("jailer");
        std::fs::write(
            &stand_in,
            format!("#!/bin/sh\n/bin/cat /proc/self/cgroup > {reported}\n"),
        )
        .unwrap();
        std::fs::set_permissions(&stand_in, std::fs::Permissions::from_mode(0o755)).unwrap();
        let vm_id = vm_id();
        let mut spawn = spawn_for(&jail, &vm_id);
        spawn.jailer_bin = &stand_in;
        spawn.cgroup_procs = Some(
            std::fs::OpenOptions::new()
                .write(true)
                .open(scratch.0.join("cgroup.procs"))
                .unwrap(),
        );

        let Err(err) = FirecrackerProcess::start(spawn) else {
            panic!("the stand-in serves no API");
        };

        assert!(
            matches!(err, FirecrackerError::JailedProcessExited { .. }),
            "{err}"
        );
        let relative = scratch.0.strip_prefix(CGROUP_ROOT).unwrap();
        assert_eq!(
            std::fs::read_to_string(&reported).unwrap().trim_end(),
            format!("0::/{relative}"),
            "the child must start in its cgroup"
        );
    }

    const CGROUP_ROOT: &str = "/sys/fs/cgroup";

    struct ScratchCgroup(Utf8PathBuf);

    impl ScratchCgroup {
        fn new() -> Self {
            let path = Utf8Path::new(CGROUP_ROOT)
                .join(format!("bencher-runner-test-{}", std::process::id()));
            std::fs::create_dir(&path).unwrap();
            Self(path)
        }
    }

    impl Drop for ScratchCgroup {
        fn drop(&mut self) {
            drop(std::fs::remove_dir(&self.0));
        }
    }

    #[test]
    fn no_cgroup_or_forking_flags_are_passed() {
        // Prevents handing the cgroup to the jailer, or a fork that breaks the
        // pid the runner tracks.
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

    #[test]
    fn the_teardown_request_counts_against_the_kill_grace() {
        // Prevents a VMM that paces its answer to Ctrl+Alt+Del holding the
        // teardown that follows a timed out job.
        use std::io::{Read as _, Write as _};
        use std::os::unix::net::UnixListener;
        use std::time::Instant;

        let (_dir, jail) = jail_in_tmpdir();
        let vmm = UnixListener::bind(jail.api_socket().host().as_path()).unwrap();
        let child = Command::new("/bin/sleep").arg("30").spawn().unwrap();
        let mut process = FirecrackerProcess {
            jailed: process_around(child, &jail),
            api: PinnedSocket::pin(jail.api_socket().socket()).unwrap(),
        };

        let started = Instant::now();
        let held = std::thread::scope(|scope| {
            scope.spawn(|| {
                let (mut stream, _) = vmm.accept().unwrap();
                drop(stream.read(&mut [0u8; 512]));
                drop(stream.write_all(b"HTTP/1.1 204 No Content\r\n"));
                while started.elapsed() < Duration::from_secs(8) && stream.write_all(b"X").is_ok() {
                    std::thread::sleep(Duration::from_millis(20));
                }
            });
            process.kill_after_grace_period(Duration::from_millis(300));
            started.elapsed()
        });

        assert!(
            held < Duration::from_secs(3),
            "the request must end with the grace, held {held:?}"
        );
    }
}
