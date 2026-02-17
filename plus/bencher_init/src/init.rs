//! Linux init implementation.

#![expect(unsafe_code)]

use std::ffi::CString;
use std::fs::{self, File};
use std::io::{self, Read};
use std::os::unix::io::RawFd;
use std::path::Path;
use std::process::ExitCode;
use std::sync::atomic::{AtomicBool, Ordering};

use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;

/// Vsock ports for result communication.
mod ports {
    pub const STDOUT: u32 = 5000;
    pub const STDERR: u32 = 5001;
    pub const EXIT_CODE: u32 = 5002;
    pub const OUTPUT_FILES: u32 = 5005;
}

/// Vsock CID for host.
const VSOCK_CID_HOST: u32 = 2;

/// Config file path.
const CONFIG_PATH: &str = "/etc/bencher/config.json";

/// Signal flag for graceful shutdown.
static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Default maximum output size: 25 MiB, matching the host-side vsock `MAX_DATA_SIZE`.
const fn default_max_output_size() -> usize {
    25 * 1024 * 1024
}

/// Benchmark configuration read from config file.
#[derive(Debug, Deserialize)]
struct Config {
    /// Command to execute (first element is the program).
    command: Vec<String>,
    /// Working directory.
    #[serde(default = "default_workdir")]
    workdir: Utf8PathBuf,
    /// Environment variables.
    #[serde(default)]
    env: Vec<(String, String)>,
    /// Optional output file paths to send back.
    file_paths: Option<Vec<Utf8PathBuf>>,
    /// Maximum size in bytes for collected stdout/stderr.
    #[serde(default = "default_max_output_size")]
    max_output_size: usize,
}

fn default_workdir() -> Utf8PathBuf {
    Utf8PathBuf::from("/")
}

/// Write a message to the console for debugging.
/// Uses direct serial port I/O on x86_64 for reliable early boot output.
fn console_log(msg: &str) {
    let formatted = format!("[bencher-init] {msg}\n");
    let bytes = formatted.as_bytes();

    // Use direct serial port I/O for reliable output even before /dev is mounted
    #[cfg(target_arch = "x86_64")]
    {
        serial_write(bytes);
        return;
    }

    // Fallback for non-x86_64 architectures
    #[cfg(not(target_arch = "x86_64"))]
    {
        use std::io::Write;
        // Try stdout first
        let written =
            unsafe { libc::write(libc::STDOUT_FILENO, bytes.as_ptr().cast(), bytes.len()) };
        if written > 0 {
            return;
        }

        // Try /dev/ttyS0 (serial console the kernel uses)
        if let Ok(mut f) = fs::OpenOptions::new().write(true).open("/dev/ttyS0") {
            let _ = f.write_all(bytes);
            let _ = f.flush();
            return;
        }

        // Fall back to stderr
        eprint!("{}", String::from_utf8_lossy(bytes));
    }
}

/// Write bytes directly to the serial port (COM1 at 0x3F8).
/// This works even before /dev is mounted.
#[cfg(target_arch = "x86_64")]
fn serial_write(data: &[u8]) {
    const COM1_DATA: u16 = 0x3F8;
    const COM1_LSR: u16 = 0x3FD;
    const LSR_THRE: u8 = 0x20; // Transmit Holding Register Empty

    // Try to get I/O port access (requires root, which init has)
    unsafe {
        let _ = libc::iopl(3);
    }

    for &byte in data {
        // Wait for transmit holding register to be empty
        unsafe {
            loop {
                let status: u8;
                std::arch::asm!(
                    "in al, dx",
                    in("dx") COM1_LSR,
                    out("al") status,
                    options(nostack, nomem, preserves_flags)
                );
                if status & LSR_THRE != 0 {
                    break;
                }
            }
            // Write byte to data register
            std::arch::asm!(
                "out dx, al",
                in("dx") COM1_DATA,
                in("al") byte,
                options(nostack, nomem, preserves_flags)
            );
        }
    }
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

    // Step 7: Send results via vsock, fall back to serial
    console_log("sending results via vsock...");
    match send_results(&result, config.file_paths.as_deref()) {
        Ok(()) => console_log("results sent via vsock"),
        Err(e) => {
            console_log(&format!(
                "vsock failed ({e}), falling back to serial output"
            ));
            output_results_serial(&result);
        },
    }

    // Step 8: Shutdown
    // Exit the init process. Since we're PID 1, this causes a kernel panic:
    //   "Attempted to kill init!"
    // With panic=1 in cmdline, the kernel reboots after 1 second.
    // With reboot=t, the reboot triggers a triple-fault → VcpuExit::Shutdown.
    console_log("exiting (will trigger kernel panic → reboot → VM shutdown)...");
    // Sync filesystems before exit
    unsafe { libc::sync() };
    // Return Ok to let main() exit with ExitCode::SUCCESS
    Ok(())
}

/// Remount the root filesystem read-write.
///
/// The kernel always mounts root read-only initially. The init process
/// must remount it read-write so we can create directories like /proc, /sys, etc.
fn remount_root_rw() -> Result<(), InitError> {
    let source = c"none";
    let target = c"/";

    let ret = unsafe {
        libc::mount(
            source.as_ptr(),
            target.as_ptr(),
            std::ptr::null(),
            libc::MS_REMOUNT,
            std::ptr::null(),
        )
    };

    if ret != 0 {
        return Err(InitError::Mount(format!(
            "remount / rw: {}",
            io::Error::last_os_error()
        )));
    }

    console_log("remounted / read-write");
    Ok(())
}

/// Mount essential filesystems.
fn mount_filesystems() -> Result<(), InitError> {
    // Remount root filesystem read-write first.
    // The kernel initially mounts root read-only even with 'rw' in cmdline;
    // init is responsible for remounting it read-write.
    remount_root_rw()?;

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
        std::fs::metadata(Path::new(path).parent().unwrap_or(Path::new("/"))),
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
    let source =
        CString::new(source).map_err(|e| InitError::Mount(format!("invalid source: {e}")))?;
    let target =
        CString::new(target).map_err(|e| InitError::Mount(format!("invalid target: {e}")))?;
    let fstype =
        CString::new(fstype).map_err(|e| InitError::Mount(format!("invalid fstype: {e}")))?;
    let data = data
        .map(CString::new)
        .transpose()
        .map_err(|e| InitError::Mount(format!("invalid data: {e}")))?;

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
        // Use default SIGCHLD handler - we reap children explicitly via waitpid.
        // Do NOT use SIG_IGN, as that causes children to be auto-reaped and
        // waitpid(-1) to return ECHILD, preventing exit code collection.
        libc::signal(libc::SIGCHLD, libc::SIG_DFL);
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
            let Ok(program) = CString::new(config.command[0].as_str()) else {
                eprintln!("invalid command: contains NUL byte");
                unsafe { libc::_exit(127) };
            };
            let Ok(args): Result<Vec<CString>, _> = config
                .command
                .iter()
                .map(|s| CString::new(s.as_str()))
                .collect()
            else {
                eprintln!("invalid argument: contains NUL byte");
                unsafe { libc::_exit(127) };
            };
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
        },
        child_pid => {
            // Parent process
            // Close write ends
            unsafe {
                libc::close(stdout_write);
                libc::close(stderr_write);
            }

            // Wait for child while collecting output and reaping zombies
            wait_for_child(child_pid, stdout_read, stderr_read, config.max_output_size)
        },
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
    max_output_size: usize,
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
            Ok(0) => {},
            Ok(n) => {
                let remaining = max_output_size.saturating_sub(stdout_buf.len());
                stdout_buf.extend_from_slice(&buf[..n.min(remaining)]);
            },
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {},
            Err(e) => eprintln!("stdout read error: {e}"),
        }
        match stderr_file.read(&mut buf) {
            Ok(0) => {},
            Ok(n) => {
                let remaining = max_output_size.saturating_sub(stderr_buf.len());
                stderr_buf.extend_from_slice(&buf[..n.min(remaining)]);
            },
            Err(e) if e.kind() == io::ErrorKind::WouldBlock => {},
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
        } else {
            // waited == -1: error or no children
            // ECHILD means our child was already reaped (e.g., SIGCHLD was SIG_IGN)
            let err = io::Error::last_os_error();
            if err.raw_os_error() == Some(libc::ECHILD) {
                // Child already reaped, we can't get exit code
                exit_code = Some(1);
            }
        }

        // If we have exit code, do one more read to drain pipes
        if exit_code.is_some() {
            // Drain remaining output
            loop {
                match stdout_file.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let remaining = max_output_size.saturating_sub(stdout_buf.len());
                        stdout_buf.extend_from_slice(&buf[..n.min(remaining)]);
                    },
                }
            }
            loop {
                match stderr_file.read(&mut buf) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        let remaining = max_output_size.saturating_sub(stderr_buf.len());
                        stderr_buf.extend_from_slice(&buf[..n.min(remaining)]);
                    },
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
fn send_results(
    result: &BenchmarkResult,
    file_paths: Option<&[Utf8PathBuf]>,
) -> Result<(), InitError> {
    // Send stdout
    send_vsock(ports::STDOUT, &result.stdout)?;

    // Send stderr
    send_vsock(ports::STDERR, &result.stderr)?;

    // Send exit code
    let exit_code_str = result.exit_code.to_string();
    send_vsock(ports::EXIT_CODE, exit_code_str.as_bytes())?;

    // Send output files if specified, using the length-prefixed binary protocol
    if let Some(paths) = file_paths {
        if !paths.is_empty() {
            let encoded = encode_output_files(paths);
            if !encoded.is_empty() {
                send_vsock(ports::OUTPUT_FILES, &encoded)?;
            }
        }
    }

    Ok(())
}

/// Encode output files using the length-prefixed binary protocol.
///
/// Files that don't exist or fail to read are silently skipped.
/// Returns an empty `Vec` if no files were successfully read.
fn encode_output_files(paths: &[Utf8PathBuf]) -> Vec<u8> {
    // First pass: collect successfully read files
    let mut files: Vec<(&Utf8Path, Vec<u8>)> = Vec::new();
    for path in paths {
        if Path::new(path.as_str()).exists() {
            match fs::read(path.as_str()) {
                Ok(content) => files.push((path, content)),
                Err(e) => eprintln!("failed to read output file {path}: {e}"),
            }
        }
    }

    if files.is_empty() {
        return Vec::new();
    }

    // Second pass: encode using shared protocol
    let encode_input: Vec<(&Utf8Path, &[u8])> =
        files.iter().map(|(p, c)| (*p, c.as_slice())).collect();
    match bencher_output_protocol::encode(&encode_input) {
        Ok(encoded) => encoded,
        Err(e) => {
            eprintln!("failed to encode output files: {e}");
            Vec::new()
        },
    }
}

/// Close a file descriptor, logging any error.
fn close_fd(fd: RawFd) {
    let ret = unsafe { libc::close(fd) };
    if ret != 0 {
        console_log(&format!(
            "warning: close(fd={fd}) failed: {}",
            io::Error::last_os_error()
        ));
    }
}

/// Vsock connect/send timeout in seconds.
const VSOCK_TIMEOUT_SECS: i64 = 2;

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

    // Set send timeout to prevent blocking indefinitely on connect and write.
    // On Linux, SO_SNDTIMEO also affects connect() timeout.
    let timeout = libc::timeval {
        tv_sec: VSOCK_TIMEOUT_SECS,
        tv_usec: 0,
    };
    unsafe {
        libc::setsockopt(
            fd,
            libc::SOL_SOCKET,
            libc::SO_SNDTIMEO,
            std::ptr::from_ref(&timeout).cast(),
            size_of::<libc::timeval>() as u32,
        );
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
        close_fd(fd);
        return Err(InitError::Vsock(format!(
            "connect to port {port}: {}",
            io::Error::last_os_error()
        )));
    }

    // Send data with retry for EINTR
    let mut sent = 0;
    while sent < data.len() {
        let n = unsafe { libc::write(fd, data[sent..].as_ptr().cast(), data.len() - sent) };
        if n < 0 {
            let err = io::Error::last_os_error();
            // Retry on EINTR (signal interrupted)
            if err.raw_os_error() == Some(libc::EINTR) {
                continue;
            }
            close_fd(fd);
            return Err(InitError::Vsock(format!("write to port {port}: {err}")));
        }
        if n == 0 {
            close_fd(fd);
            return Err(InitError::Vsock(format!(
                "write to port {port}: connection closed"
            )));
        }
        sent += n as usize;
    }

    close_fd(fd);
    Ok(())
}

/// Output benchmark results via serial port.
///
/// This is the fallback when vsock is unavailable. The output format uses
/// markers so the VMM can parse results from the serial stream:
///
/// ```text
/// ---BENCHER_STDOUT_BEGIN---
/// <stdout content>
/// ---BENCHER_STDOUT_END---
/// ---BENCHER_STDERR_BEGIN---
/// <stderr content>
/// ---BENCHER_STDERR_END---
/// ---BENCHER_EXIT_CODE:<code>---
/// ```
#[cfg(target_arch = "x86_64")]
fn output_results_serial(result: &BenchmarkResult) {
    serial_write(b"---BENCHER_STDOUT_BEGIN---\n");
    serial_write(&result.stdout);
    serial_write(b"\n---BENCHER_STDOUT_END---\n");

    serial_write(b"---BENCHER_STDERR_BEGIN---\n");
    serial_write(&result.stderr);
    serial_write(b"\n---BENCHER_STDERR_END---\n");

    let exit_line = format!("---BENCHER_EXIT_CODE:{}---\n", result.exit_code);
    serial_write(exit_line.as_bytes());

    // Write the done marker LAST. The VMM uses this (not the exit code marker)
    // as the shutdown trigger, ensuring all preceding data is captured.
    serial_write(b"---BENCHER_DONE---\n");
}

/// Output benchmark results via stderr (non-x86_64 fallback).
///
/// On non-x86_64, we don't have direct serial port access, so fall back
/// to stderr with the same marker format.
#[cfg(not(target_arch = "x86_64"))]
fn output_results_serial(result: &BenchmarkResult) {
    eprintln!("---BENCHER_STDOUT_BEGIN---");
    eprint!("{}", String::from_utf8_lossy(&result.stdout));
    eprintln!("\n---BENCHER_STDOUT_END---");
    eprintln!("---BENCHER_STDERR_BEGIN---");
    eprint!("{}", String::from_utf8_lossy(&result.stderr));
    eprintln!("\n---BENCHER_STDERR_END---");
    eprintln!("---BENCHER_EXIT_CODE:{}---", result.exit_code);
    eprintln!("---BENCHER_DONE---");
}

/// Shut down the system.
///
/// Writes to I/O port 0x604 (QEMU/Firecracker exit port) to signal the VMM
/// that the guest is done. This triggers `VcpuExit::IoOut` which the VMM
/// handles as a shutdown. Falls back to `reboot(RB_POWER_OFF)` if the
/// port write doesn't cause an exit.
fn poweroff() {
    // Sync filesystems
    unsafe { libc::sync() };

    // Write to I/O port 0x604 to signal shutdown to the VMM.
    // This is the standard exit port used by Firecracker and QEMU.
    #[cfg(target_arch = "x86_64")]
    unsafe {
        // Get I/O port access (requires iopl >= 1)
        let _ = libc::iopl(3);
        std::arch::asm!(
            "out dx, al",
            in("dx") 0x604u16,
            in("al") 0x00u8,
            options(nostack, nomem, preserves_flags)
        );
    }

    // Fallback: use reboot syscall
    unsafe {
        libc::reboot(libc::RB_POWER_OFF);
    }
}

/// Init errors.
#[derive(Debug, thiserror::Error)]
enum InitError {
    #[error("mount: {0}")]
    Mount(String),
    #[error("signal: {0}")]
    Signal(String),
    #[error("config: {0}")]
    Config(String),
    #[error("fork: {0}")]
    Fork(String),
    #[error("io: {0}")]
    Io(String),
    #[error("vsock: {0}")]
    Vsock(String),
}
