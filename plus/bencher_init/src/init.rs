//! Linux init implementation.

#![expect(unsafe_code)]

use std::ffi::CString;
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::unix::io::RawFd;
use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Deserialize;

/// Vsock ports for result communication.
mod ports {
    pub const STDOUT: u32 = 5000;
    pub const STDERR: u32 = 5001;
    pub const EXIT_CODE: u32 = 5002;
    pub const OUTPUT_FILE: u32 = 5005;
}

/// Vsock CID for host.
const VSOCK_CID_HOST: u32 = 2;

/// Config file path.
const CONFIG_PATH: &str = "/etc/bencher/config.json";

/// Signal flag for graceful shutdown.
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Benchmark configuration read from config file.
#[derive(Debug, Deserialize)]
struct Config {
    /// Command to execute (first element is the program).
    command: Vec<String>,
    /// Working directory.
    #[serde(default = "default_workdir")]
    workdir: String,
    /// Environment variables.
    #[serde(default)]
    env: Vec<(String, String)>,
    /// Optional output file to send back.
    output_file: Option<String>,
}

fn default_workdir() -> String {
    "/".to_owned()
}

/// Write a message to the console for debugging.
fn console_log(msg: &str) {
    use std::io::Write;
    let formatted = format!("[bencher-init] {msg}\n");
    let bytes = formatted.as_bytes();

    // First, try writing directly to stdout (fd 1) which kernel sets up to /dev/console
    // This is the most reliable early output method
    let written = unsafe { libc::write(libc::STDOUT_FILENO, bytes.as_ptr().cast(), bytes.len()) };
    if written > 0 {
        return;
    }

    // Try /dev/ttyS0 (serial console the kernel uses)
    if let Ok(mut f) = fs::OpenOptions::new().write(true).open("/dev/ttyS0") {
        let _ = f.write_all(bytes);
        let _ = f.flush();
        return;
    }

    // Try /dev/console (kernel-provided)
    if let Ok(mut f) = fs::OpenOptions::new().write(true).open("/dev/console") {
        let _ = f.write_all(bytes);
        let _ = f.flush();
        return;
    }

    // Fall back to stderr
    eprint!("{formatted}");
}

/// Main entry point for the init process.
pub fn run() -> ExitCode {
    console_log("starting...");

    // Ensure we're PID 1
    if std::process::id() != 1 {
        console_log("warning: not running as PID 1");
    }

    if let Err(e) = run_init() {
        console_log(&format!("fatal error: {e}"));
        // Try to send error via vsock before dying
        let error_msg = format!("init error: {e}");
        let _ = send_vsock(ports::STDERR, error_msg.as_bytes());
        let _ = send_vsock(ports::EXIT_CODE, b"1");
        poweroff();
        return ExitCode::FAILURE;
    }

    console_log("completed successfully");
    ExitCode::SUCCESS
}

fn run_init() -> Result<(), InitError> {
    console_log("mounting filesystems...");
    // Step 1: Mount essential filesystems
    mount_filesystems()?;
    console_log("filesystems mounted");

    // Step 2: Set up signal handlers
    setup_signal_handlers()?;
    console_log("signal handlers set up");

    // Step 3: Read config
    console_log("reading config...");
    let config = read_config()?;
    console_log(&format!("config loaded: command={:?}", config.command));

    // Step 4: Change to working directory
    std::env::set_current_dir(&config.workdir)
        .map_err(|e| InitError::Io(format!("chdir to {}: {e}", config.workdir)))?;
    console_log(&format!("changed to workdir: {}", config.workdir));

    // Step 5: Set environment variables
    for (key, value) in &config.env {
        // SAFETY: We're the init process, no other threads exist yet
        unsafe { std::env::set_var(key, value) };
    }

    // Step 6: Run the benchmark
    console_log(&format!("running benchmark: {}", config.command.join(" ")));
    let result = run_benchmark(&config)?;
    console_log(&format!(
        "benchmark finished: exit_code={}, stdout_len={}, stderr_len={}",
        result.exit_code,
        result.stdout.len(),
        result.stderr.len()
    ));

    // Step 7: Send results via vsock
    console_log("sending results via vsock...");
    send_results(&result, config.output_file.as_deref())?;
    console_log("results sent");

    // Step 8: Shutdown
    console_log("shutting down...");
    poweroff();

    Ok(())
}

/// Mount essential filesystems.
fn mount_filesystems() -> Result<(), InitError> {
    // Create mount points if they don't exist
    let _ = fs::create_dir_all("/proc");
    let _ = fs::create_dir_all("/sys");
    let _ = fs::create_dir_all("/dev");
    let _ = fs::create_dir_all("/tmp");
    let _ = fs::create_dir_all("/run");

    // Mount proc (if not already mounted)
    if !is_mounted("/proc") {
        mount("proc", "/proc", "proc", 0, None)?;
    } else {
        console_log("proc already mounted");
    }

    // Mount sysfs (if not already mounted)
    if !is_mounted("/sys") {
        mount("sysfs", "/sys", "sysfs", 0, None)?;
    } else {
        console_log("sysfs already mounted");
    }

    // Mount devtmpfs (if not already mounted)
    if !is_mounted("/dev") {
        mount("devtmpfs", "/dev", "devtmpfs", 0, None)?;
    } else {
        console_log("devtmpfs already mounted");
    }

    // Mount tmpfs on /tmp and /run
    if !is_mounted("/tmp") {
        mount("tmpfs", "/tmp", "tmpfs", 0, Some("mode=1777"))?;
    }
    if !is_mounted("/run") {
        mount("tmpfs", "/run", "tmpfs", 0, Some("mode=755"))?;
    }

    Ok(())
}

/// Check if a path is already a mount point.
fn is_mounted(path: &str) -> bool {
    // Check /proc/mounts to see if the path is mounted
    if let Ok(contents) = fs::read_to_string("/proc/self/mounts") {
        for line in contents.lines() {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 && parts[1] == path {
                return true;
            }
        }
    }
    // If we can't read /proc/mounts (e.g., /proc not mounted yet),
    // check if the path looks like it has a filesystem mounted
    // by comparing device IDs of the path and its parent
    if let (Ok(path_stat), Ok(parent_stat)) = (
        std::fs::metadata(path),
        std::fs::metadata(Path::new(path).parent().unwrap_or(Path::new("/")))
    ) {
        use std::os::unix::fs::MetadataExt;
        // Different device ID means different filesystem = mounted
        return path_stat.dev() != parent_stat.dev();
    }
    false
}

/// Wrapper around mount(2).
fn mount(
    source: &str,
    target: &str,
    fstype: &str,
    flags: libc::c_ulong,
    data: Option<&str>,
) -> Result<(), InitError> {
    let source = CString::new(source).unwrap();
    let target = CString::new(target).unwrap();
    let fstype = CString::new(fstype).unwrap();
    let data = data.map(|d| CString::new(d).unwrap());

    let ret = unsafe {
        libc::mount(
            source.as_ptr(),
            target.as_ptr(),
            fstype.as_ptr(),
            flags,
            data.as_ref()
                .map(|d| d.as_ptr().cast())
                .unwrap_or(std::ptr::null()),
        )
    };

    if ret != 0 {
        return Err(InitError::Mount(format!(
            "mount {} on {}: {}",
            source.to_string_lossy(),
            target.to_string_lossy(),
            io::Error::last_os_error()
        )));
    }

    Ok(())
}

/// Set up signal handlers for graceful shutdown.
fn setup_signal_handlers() -> Result<(), InitError> {
    unsafe {
        // SIGTERM - graceful shutdown request
        if libc::signal(libc::SIGTERM, handle_signal as libc::sighandler_t) == libc::SIG_ERR {
            return Err(InitError::Signal("failed to set SIGTERM handler".into()));
        }
        // SIGINT - also graceful shutdown
        if libc::signal(libc::SIGINT, handle_signal as libc::sighandler_t) == libc::SIG_ERR {
            return Err(InitError::Signal("failed to set SIGINT handler".into()));
        }
        // Ignore SIGCHLD - we handle child reaping explicitly
        if libc::signal(libc::SIGCHLD, libc::SIG_IGN) == libc::SIG_ERR {
            return Err(InitError::Signal("failed to ignore SIGCHLD".into()));
        }
    }

    Ok(())
}

/// Signal handler function.
extern "C" fn handle_signal(_sig: libc::c_int) {
    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
}

/// Read benchmark configuration from config file.
fn read_config() -> Result<Config, InitError> {
    let content = fs::read_to_string(CONFIG_PATH)
        .map_err(|e| InitError::Config(format!("read {CONFIG_PATH}: {e}")))?;

    serde_json::from_str(&content)
        .map_err(|e| InitError::Config(format!("parse {CONFIG_PATH}: {e}")))
}

/// Benchmark execution result.
struct BenchmarkResult {
    stdout: Vec<u8>,
    stderr: Vec<u8>,
    exit_code: i32,
}

/// Run the benchmark command.
fn run_benchmark(config: &Config) -> Result<BenchmarkResult, InitError> {
    if config.command.is_empty() {
        return Err(InitError::Config("empty command".into()));
    }

    // Create pipes for stdout/stderr
    let (stdout_read, stdout_write) = pipe()?;
    let (stderr_read, stderr_write) = pipe()?;

    // Fork
    let pid = unsafe { libc::fork() };

    match pid {
        -1 => Err(InitError::Fork(io::Error::last_os_error().to_string())),
        0 => {
            // Child process
            // Close read ends
            unsafe {
                libc::close(stdout_read);
                libc::close(stderr_read);
            }

            // Redirect stdout/stderr
            unsafe {
                libc::dup2(stdout_write, libc::STDOUT_FILENO);
                libc::dup2(stderr_write, libc::STDERR_FILENO);
                libc::close(stdout_write);
                libc::close(stderr_write);
            }

            // Exec the command
            let program = CString::new(config.command[0].as_str()).unwrap();
            let args: Vec<CString> = config
                .command
                .iter()
                .map(|s| CString::new(s.as_str()).unwrap())
                .collect();
            let argv: Vec<*const libc::c_char> = args
                .iter()
                .map(|s| s.as_ptr())
                .chain(std::iter::once(std::ptr::null()))
                .collect();

            unsafe {
                libc::execvp(program.as_ptr(), argv.as_ptr());
            }

            // If we get here, exec failed
            eprintln!("exec failed: {}", io::Error::last_os_error());
            unsafe { libc::_exit(127) };
        }
        child_pid => {
            // Parent process
            // Close write ends
            unsafe {
                libc::close(stdout_write);
                libc::close(stderr_write);
            }

            // Wait for child while collecting output and reaping zombies
            wait_for_child(child_pid, stdout_read, stderr_read)
        }
    }
}

/// Create a pipe.
fn pipe() -> Result<(RawFd, RawFd), InitError> {
    let mut fds = [0i32; 2];
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return Err(InitError::Io(format!(
            "pipe: {}",
            io::Error::last_os_error()
        )));
    }
    Ok((fds[0], fds[1]))
}

/// Wait for child process, collecting output and reaping zombies.
fn wait_for_child(
    child_pid: libc::pid_t,
    stdout_fd: RawFd,
    stderr_fd: RawFd,
) -> Result<BenchmarkResult, InitError> {
    use std::os::unix::io::FromRawFd;

    // Set non-blocking on the pipes
    unsafe {
        let flags = libc::fcntl(stdout_fd, libc::F_GETFL);
        libc::fcntl(stdout_fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
        let flags = libc::fcntl(stderr_fd, libc::F_GETFL);
        libc::fcntl(stderr_fd, libc::F_SETFL, flags | libc::O_NONBLOCK);
    }

    let mut stdout_file = unsafe { File::from_raw_fd(stdout_fd) };
    let mut stderr_file = unsafe { File::from_raw_fd(stderr_fd) };

    let mut stdout_buf = Vec::new();
    let mut stderr_buf = Vec::new();
    let mut exit_code: Option<i32> = None;

    loop {
        // Check for shutdown signal
        if SHUTDOWN_REQUESTED.load(Ordering::SeqCst) {
            // Send SIGTERM to child
            unsafe { libc::kill(child_pid, libc::SIGTERM) };
        }

        // Try to read from pipes
        let mut buf = [0u8; 4096];
        match stdout_file.read(&mut buf) {
            Ok(0) => {}
            Ok(n) => stdout_buf.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => eprintln!("stdout read error: {e}"),
        }
        match stderr_file.read(&mut buf) {
            Ok(0) => {}
            Ok(n) => stderr_buf.extend_from_slice(&buf[..n]),
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {}
            Err(e) => eprintln!("stderr read error: {e}"),
        }

        // Reap zombies and check for our child
        let mut status: libc::c_int = 0;
        let waited = unsafe { libc::waitpid(-1, &mut status, libc::WNOHANG) };

        if waited == child_pid {
            // Our child exited
            exit_code = Some(if libc::WIFEXITED(status) {
                libc::WEXITSTATUS(status)
            } else if libc::WIFSIGNALED(status) {
                128 + libc::WTERMSIG(status)
            } else {
                1
            });
        } else if waited > 0 {
            // Reaped some other zombie, continue
        } else if waited == 0 {
            // No children ready, sleep briefly
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        // waited == -1 means no children or error, continue

        // If we have exit code, do one more read to drain pipes
        if exit_code.is_some() {
            // Drain remaining output
            loop {
                match stdout_file.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => stdout_buf.extend_from_slice(&buf[..n]),
                }
            }
            loop {
                match stderr_file.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => stderr_buf.extend_from_slice(&buf[..n]),
                }
            }
            break;
        }
    }

    Ok(BenchmarkResult {
        stdout: stdout_buf,
        stderr: stderr_buf,
        exit_code: exit_code.unwrap_or(1),
    })
}

/// Send benchmark results via vsock.
fn send_results(result: &BenchmarkResult, output_file: Option<&str>) -> Result<(), InitError> {
    // Send stdout
    send_vsock(ports::STDOUT, &result.stdout)?;

    // Send stderr
    send_vsock(ports::STDERR, &result.stderr)?;

    // Send exit code
    let exit_code_str = result.exit_code.to_string();
    send_vsock(ports::EXIT_CODE, exit_code_str.as_bytes())?;

    // Send output file if specified and exists
    if let Some(path) = output_file {
        if Path::new(path).exists() {
            match fs::read(path) {
                Ok(content) => send_vsock(ports::OUTPUT_FILE, &content)?,
                Err(e) => eprintln!("failed to read output file {path}: {e}"),
            }
        }
    }

    Ok(())
}

/// Send data via vsock to the host.
fn send_vsock(port: u32, data: &[u8]) -> Result<(), InitError> {
    // Create vsock socket
    let fd = unsafe { libc::socket(libc::AF_VSOCK, libc::SOCK_STREAM, 0) };
    if fd < 0 {
        return Err(InitError::Vsock(format!(
            "socket: {}",
            io::Error::last_os_error()
        )));
    }

    // Connect to host
    let addr = libc::sockaddr_vm {
        svm_family: libc::AF_VSOCK as libc::sa_family_t,
        svm_reserved1: 0,
        svm_port: port,
        svm_cid: VSOCK_CID_HOST,
        svm_zero: [0; 4],
    };

    let ret = unsafe {
        libc::connect(
            fd,
            std::ptr::from_ref(&addr).cast(),
            size_of::<libc::sockaddr_vm>() as u32,
        )
    };

    if ret != 0 {
        unsafe { libc::close(fd) };
        return Err(InitError::Vsock(format!(
            "connect to port {port}: {}",
            io::Error::last_os_error()
        )));
    }

    // Send data
    let mut sent = 0;
    while sent < data.len() {
        let n = unsafe { libc::write(fd, data[sent..].as_ptr().cast(), data.len() - sent) };
        if n <= 0 {
            unsafe { libc::close(fd) };
            return Err(InitError::Vsock(format!(
                "write to port {port}: {}",
                io::Error::last_os_error()
            )));
        }
        sent += n as usize;
    }

    unsafe { libc::close(fd) };
    Ok(())
}

/// Shut down the system.
fn poweroff() {
    // Sync filesystems
    unsafe { libc::sync() };

    // reboot(2) with RB_POWER_OFF
    unsafe {
        libc::reboot(libc::RB_POWER_OFF);
    }
}

/// Init errors.
#[derive(Debug)]
enum InitError {
    Mount(String),
    Signal(String),
    Config(String),
    Fork(String),
    Io(String),
    Vsock(String),
}

impl std::fmt::Display for InitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Mount(s) => write!(f, "mount: {s}"),
            Self::Signal(s) => write!(f, "signal: {s}"),
            Self::Config(s) => write!(f, "config: {s}"),
            Self::Fork(s) => write!(f, "fork: {s}"),
            Self::Io(s) => write!(f, "io: {s}"),
            Self::Vsock(s) => write!(f, "vsock: {s}"),
        }
    }
}
