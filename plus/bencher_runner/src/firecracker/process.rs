//! Jailed Firecracker process management.
#![expect(clippy::print_stderr, reason = "process management prints diagnostics")]

use std::fs::File;
use std::os::unix::process::CommandExt as _;
use std::process::{Child, Command};
use std::time::Duration;

use camino::Utf8Path;

use crate::firecracker::client::FirecrackerClient;
use crate::firecracker::config::{Action, ActionType};
use crate::firecracker::error::FirecrackerError;
use crate::jail::{HostPath, JAIL_GID, JAIL_UID, JailFile};

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
    pub vm_id: &'a str,
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
    api_socket_path: HostPath,
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
    /// appear under the same prefix.
    pub fn start(spawn: JailedSpawn<'_>) -> Result<Self, FirecrackerError> {
        let JailedSpawn {
            jailer_bin,
            exec_file,
            vm_id,
            chroot_base_dir,
            netns,
            api_socket,
            log_level,
            housekeeping_cores,
            cgroup_procs,
        } = spawn;

        let mut command = Command::new(jailer_bin);
        command
            .arg("--id")
            .arg(vm_id)
            .arg("--exec-file")
            .arg(exec_file)
            .arg("--uid")
            .arg(JAIL_UID.to_string())
            .arg("--gid")
            .arg(JAIL_GID.to_string())
            .arg("--chroot-base-dir")
            .arg(chroot_base_dir)
            .arg("--netns")
            .arg(netns)
            .arg("--")
            // `--id` is deliberately not forwarded: the jailer already passes
            // it to Firecracker, and Firecracker rejects a duplicate argument.
            .arg("--api-sock")
            .arg(api_socket.chroot().as_str())
            .arg("--level")
            .arg(log_level)
            .stdin(std::process::Stdio::null())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::piped());

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

        let mut child = command.spawn().map_err(|e| {
            FirecrackerError::ProcessStart(format!("failed to spawn {jailer_bin}: {e}"))
        })?;

        // Spawn a thread to read stderr line-by-line
        let stderr = child.stderr.take().ok_or_else(|| {
            FirecrackerError::ProcessStart("stderr was piped but not available".into())
        })?;
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

        let process = Self {
            child,
            api_socket_path: api_socket.host().clone(),
            stderr_thread: Some(stderr_thread),
        };

        // Wait for the API socket to become ready
        process.client().wait_for_ready(Duration::from_secs(5))?;

        Ok(process)
    }

    /// Get a client for the Firecracker REST API.
    pub fn client(&self) -> FirecrackerClient {
        FirecrackerClient::new(&self.api_socket_path)
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

        // Force kill if still running
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
    pub fn cleanup(&self) {
        drop(std::fs::remove_file(self.api_socket_path.as_path()));
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

/// Join the calling task to the cgroup behind a pre-opened `cgroup.procs`.
///
/// The kernel reads `0` as the calling task, which is why no pid has to be
/// formatted (and no allocation performed) inside the forked child.
fn place_in_cgroup(mut procs: &File) -> std::io::Result<()> {
    use std::io::Write as _;

    procs.write_all(b"0")
}
