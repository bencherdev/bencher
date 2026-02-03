//! VM event loop.
//!
//! This module handles the main execution loop for the VM, processing
//! vCPU exits and device I/O.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use kvm_ioctls::VcpuExit;

use crate::devices::DeviceManager;
use crate::error::VmmError;
use crate::vcpu::Vcpu;

/// Shared state for signaling vCPU threads to stop.
///
/// Each vCPU thread registers its Linux thread ID (TID) on startup.
/// When shutdown is triggered, SIGALRM is sent to each registered TID
/// using `tgkill`, ensuring all threads are interrupted even if they
/// are blocked inside a KVM ioctl.
struct VcpuSignaler {
    /// Linux thread IDs of vCPU threads.
    tids: Mutex<Vec<i32>>,
    /// Process ID for tgkill.
    pid: i32,
}

impl VcpuSignaler {
    fn new() -> Self {
        Self {
            tids: Mutex::new(Vec::new()),
            pid: std::process::id() as i32,
        }
    }

    /// Register the current thread as a vCPU thread.
    fn register_current_thread(&self) {
        // SAFETY: gettid() is always safe to call.
        let tid = unsafe { libc::syscall(libc::SYS_gettid) } as i32;
        if let Ok(mut tids) = self.tids.lock() {
            tids.push(tid);
        }
    }

    /// Send SIGALRM to all registered vCPU threads.
    ///
    /// This interrupts any blocking KVM ioctls, causing them to return EINTR.
    /// Uses `tgkill` to target each vCPU thread individually, ensuring ALL
    /// threads are interrupted even when there are multiple vCPUs.
    fn signal_all(&self) {
        if let Ok(tids) = self.tids.lock() {
            for &tid in &*tids {
                // SAFETY: Sending SIGALRM to threads in our own process is safe.
                unsafe {
                    libc::syscall(libc::SYS_tgkill, self.pid, tid, libc::SIGALRM);
                }
            }
        }
    }
}

/// Maximum size for serial output buffer (10 MiB).
///
/// This prevents malicious workloads from exhausting host memory by
/// generating excessive output. Output beyond this limit is discarded.
const MAX_SERIAL_OUTPUT_SIZE: usize = 10 * 1024 * 1024;

/// Serial marker indicating the guest has finished and results are ready.
/// This marker is written AFTER all result data (stdout, stderr, HMAC, exit code)
/// to ensure all preceding data is fully captured before the VMM shuts down.
const SERIAL_EXIT_MARKER: &[u8] = b"---BENCHER_DONE---";

/// Grace period after timeout before forceful termination (in milliseconds).
///
/// After the timeout expires, we signal shutdown and wait this long for
/// graceful termination. If the VM is still running, we return a timeout error.
const TIMEOUT_GRACE_PERIOD_MS: u64 = 1000;

/// Run the VM event loop.
///
/// This runs the vCPUs and handles exits until the VM shuts down or times out.
///
/// # Arguments
///
/// * `vcpus` - The virtual CPUs to run
/// * `devices` - The device manager for handling I/O
/// * `timeout_secs` - Maximum execution time in seconds (0 = no timeout)
/// * `nonce` - Optional nonce for HMAC result integrity verification
///
/// # Returns
///
/// The benchmark results collected from the guest serial output or vsock,
/// along with HMAC verification status and transport type.
pub fn run(
    mut vcpus: Vec<Vcpu>,
    devices: Arc<Mutex<DeviceManager>>,
    timeout_secs: u64,
    nonce: Option<&str>,
) -> Result<crate::VmResults, VmmError> {
    // Install a no-op SIGALRM handler so the signal interrupts blocking KVM
    // ioctls (causing EINTR) without terminating the process.
    // SAFETY: Setting a signal handler is safe; we use a no-op handler.
    unsafe {
        libc::signal(libc::SIGALRM, noop_signal_handler as libc::sighandler_t);
    }

    // Flag to signal all vCPUs to stop
    let shutdown = Arc::new(AtomicBool::new(false));
    // Flag to track if we timed out
    let timed_out = Arc::new(AtomicBool::new(false));
    // Signaler for interrupting vCPU threads via tgkill
    let signaler = Arc::new(VcpuSignaler::new());

    // Start timeout thread if timeout is set.
    // The timeout thread signals shutdown, then sends SIGALRM to each vCPU
    // thread individually using tgkill to ensure all are interrupted.
    let timeout_handle = if timeout_secs > 0 {
        let shutdown_clone = Arc::clone(&shutdown);
        let timed_out_clone = Arc::clone(&timed_out);
        let signaler_clone = Arc::clone(&signaler);
        Some(thread::spawn(move || {
            // Sleep in 100ms increments so we can detect early shutdown
            let total_ms = timeout_secs * 1000;
            let mut elapsed_ms = 0u64;
            while elapsed_ms < total_ms {
                if shutdown_clone.load(Ordering::SeqCst) {
                    return; // VM already shut down, no timeout needed
                }
                thread::sleep(Duration::from_millis(100));
                elapsed_ms += 100;
            }
            // Only set timeout if we haven't already shut down
            if !shutdown_clone.swap(true, Ordering::SeqCst) {
                timed_out_clone.store(true, Ordering::SeqCst);

                // Send SIGALRM to each vCPU thread individually using tgkill.
                // Send multiple rounds to ensure all threads are interrupted,
                // even if some are between checking the shutdown flag and
                // entering the KVM ioctl.
                for _ in 0..3 {
                    signaler_clone.signal_all();
                    thread::sleep(Duration::from_millis(200));
                }
            }
        }))
    } else {
        None
    };

    // Shared serial output buffer for all vCPU threads.
    // This prevents data loss when multiple vCPUs handle serial I/O.
    let serial_output = Arc::new(Mutex::new(Vec::new()));

    // For single vCPU (common case), run in the current thread
    let result = if vcpus.len() == 1 {
        signaler.register_current_thread();
        run_vcpu_loop(
            &mut vcpus[0],
            Arc::clone(&devices),
            Arc::clone(&shutdown),
            Arc::clone(&signaler),
            Arc::clone(&serial_output),
        )
    } else {
        // For multiple vCPUs, spawn threads
        let handles: Vec<_> = vcpus
            .into_iter()
            .map(|mut vcpu| {
                let devices = Arc::clone(&devices);
                let shutdown = Arc::clone(&shutdown);
                let signaler = Arc::clone(&signaler);
                let serial_output = Arc::clone(&serial_output);

                thread::spawn(move || {
                    signaler.register_current_thread();
                    run_vcpu_loop(&mut vcpu, devices, shutdown, signaler, serial_output)
                })
            })
            .collect();

        // Wait for all vCPU threads to complete.
        // Use a polling join with grace period to avoid hanging if a
        // thread is stuck despite signaling.
        let mut result = Ok(());
        for handle in handles {
            match handle.join() {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    result = Err(e);
                    // Signal remaining threads to stop
                    shutdown.store(true, Ordering::SeqCst);
                    signaler.signal_all();
                    break;
                }
                Err(_) => {
                    result = Err(VmmError::Vcpu("vCPU thread panicked".to_owned()));
                    shutdown.store(true, Ordering::SeqCst);
                    signaler.signal_all();
                    break;
                }
            }
        }
        result
    };

    // Clean up timeout thread properly
    if let Some(handle) = timeout_handle {
        // Signal shutdown to wake up the timeout thread if it's still waiting
        shutdown.store(true, Ordering::SeqCst);

        // Wait for the timeout thread to finish
        let join_result = handle.join();

        // If join failed, the thread panicked - but we still want to check timeout
        if join_result.is_err() {
            // Thread panicked, but continue with result handling
        }
    }

    // Extract any captured output (serial or vsock) regardless of outcome.
    // This must happen before the timeout/error checks so partial output is preserved.
    let captured_output = extract_output(&devices, &serial_output, nonce);

    // Check if we timed out (use SeqCst to ensure we see the latest value)
    if timed_out.load(Ordering::SeqCst) {
        let partial = captured_output
            .map(|r| r.output)
            .unwrap_or_default();
        return Err(VmmError::Timeout {
            timeout_secs,
            partial_output: partial,
        });
    }

    // Propagate any vCPU errors
    result?;

    // Return the captured output
    captured_output.map_err(|e| VmmError::Device(format!("Failed to extract output: {e}")))
}

/// Run the main loop for a single vCPU.
fn run_vcpu_loop(
    vcpu: &mut Vcpu,
    devices: Arc<Mutex<DeviceManager>>,
    shutdown: Arc<AtomicBool>,
    signaler: Arc<VcpuSignaler>,
    serial_output: Arc<Mutex<Vec<u8>>>,
) -> Result<(), VmmError> {
    loop {
        // Check if we should stop (use SeqCst to ensure visibility from timeout thread)
        if shutdown.load(Ordering::SeqCst) {
            break;
        }

        // Run the vCPU until it exits
        match vcpu.fd.run() {
            Ok(exit_reason) => {
                let action =
                    handle_vcpu_exit(exit_reason, &devices, &serial_output)?;
                match action {
                    VmExitAction::Continue => continue,
                    VmExitAction::Shutdown => {
                        shutdown.store(true, Ordering::SeqCst);
                        // Signal all other vCPU threads to wake up
                        signaler.signal_all();
                        break;
                    }
                }
            }
            Err(e) => {
                // EAGAIN means we should retry
                if e.errno() == libc::EAGAIN {
                    continue;
                }
                // EINTR means we were interrupted (signal)
                if e.errno() == libc::EINTR {
                    continue;
                }
                return Err(VmmError::Kvm(e));
            }
        }
    }

    Ok(())
}

/// Handle a vCPU exit.
fn handle_vcpu_exit(
    exit: VcpuExit,
    devices: &Arc<Mutex<DeviceManager>>,
    serial_output: &Arc<Mutex<Vec<u8>>>,
) -> Result<VmExitAction, VmmError> {
    match exit {
        VcpuExit::IoIn(port, data) => {
            let mut dm = devices.lock().map_err(|_| {
                VmmError::Device("Failed to lock device manager".to_owned())
            })?;
            dm.handle_io_read(port, data);
            dm.check_timer();
            Ok(VmExitAction::Continue)
        }

        VcpuExit::IoOut(port, data) => {
            // Detect i8042 keyboard controller reset (port 0x64, value 0xFE).
            // The Linux kernel uses this for reboots with reboot=k.
            // Also detect writes to port 0x64 with pulse CPU reset bit.
            if port == 0x64 && !data.is_empty() && (data[0] & 0x02) != 0 {
                return Ok(VmExitAction::Shutdown);
            }
            // Detect ACPI/QEMU reset (port 0x604, value 0x00).
            if port == 0x604 {
                return Ok(VmExitAction::Shutdown);
            }
            // Detect Fast A20/Reset (port 0x92, bit 0 = reset)
            if port == 0x92 && !data.is_empty() && (data[0] & 0x01) != 0 {
                return Ok(VmExitAction::Shutdown);
            }

            let mut dm = devices.lock().map_err(|_| {
                VmmError::Device("Failed to lock device manager".to_owned())
            })?;

            let should_shutdown = dm.handle_io_write(port, data);

            // Collect serial output (with size limit to prevent memory exhaustion)
            let output = dm.get_serial_output();
            let mut results_complete = false;
            if let Ok(mut buf) = serial_output.lock() {
                extend_with_limit(&mut buf, &output, MAX_SERIAL_OUTPUT_SIZE);
                // Check if the guest has written the exit code marker, indicating
                // the benchmark is complete and results are ready.
                results_complete = serial_has_exit_marker(&buf);
            }

            dm.check_timer();

            if should_shutdown || results_complete {
                Ok(VmExitAction::Shutdown)
            } else {
                Ok(VmExitAction::Continue)
            }
        }

        VcpuExit::MmioRead(addr, data) => {
            let mut dm = devices.lock().map_err(|_| {
                VmmError::Device("Failed to lock device manager".to_owned())
            })?;
            dm.handle_mmio_read(addr, data);
            dm.check_timer();
            Ok(VmExitAction::Continue)
        }

        VcpuExit::MmioWrite(addr, data) => {
            let mut dm = devices.lock().map_err(|_| {
                VmmError::Device("Failed to lock device manager".to_owned())
            })?;
            dm.handle_mmio_write(addr, data);

            // After MMIO write, poll vsock for any pending activity
            dm.poll_vsock();

            dm.check_timer();

            Ok(VmExitAction::Continue)
        }

        VcpuExit::Hlt => {
            // CPU is halted, waiting for interrupt
            let mut dm = devices.lock().map_err(|_| {
                VmmError::Device("Failed to lock device manager".to_owned())
            })?;

            // Small sleep to avoid busy-waiting when guest is idle
            std::thread::sleep(std::time::Duration::from_micros(100));

            // Check and inject timer interrupt
            dm.check_timer();

            // Collect any serial output (with size limit)
            let output = dm.get_serial_output();
            let mut results_complete = false;
            if let Ok(mut buf) = serial_output.lock() {
                extend_with_limit(&mut buf, &output, MAX_SERIAL_OUTPUT_SIZE);
                results_complete = serial_has_exit_marker(&buf);
            }

            if results_complete {
                Ok(VmExitAction::Shutdown)
            } else {
                Ok(VmExitAction::Continue)
            }
        }

        VcpuExit::Shutdown => Ok(VmExitAction::Shutdown),

        VcpuExit::SystemEvent(event_type, _flags) => {
            // System events include shutdown, reset, crash
            // Event type 1 = shutdown, 2 = reset
            if event_type == 1 || event_type == 2 {
                Ok(VmExitAction::Shutdown)
            } else {
                Ok(VmExitAction::Continue)
            }
        }

        // Handle other exit reasons
        _other => {
            // Check timer on every unknown exit
            let mut dm = devices.lock().map_err(|_| {
                VmmError::Device("Failed to lock device manager".to_owned())
            })?;
            dm.check_timer();
            // Collect any serial output (with size limit)
            let output = dm.get_serial_output();
            let mut results_complete = false;
            if let Ok(mut buf) = serial_output.lock() {
                extend_with_limit(&mut buf, &output, MAX_SERIAL_OUTPUT_SIZE);
                results_complete = serial_has_exit_marker(&buf);
            }

            if results_complete {
                Ok(VmExitAction::Shutdown)
            } else {
                Ok(VmExitAction::Continue)
            }
        }
    }
}

/// Extract captured output from vsock (preferred) or serial buffer,
/// performing HMAC verification when a nonce is available.
fn extract_output(
    devices: &Arc<Mutex<DeviceManager>>,
    serial_output: &Arc<Mutex<Vec<u8>>>,
    nonce: Option<&str>,
) -> Result<crate::VmResults, String> {
    // Prefer vsock results if available
    let dm = devices
        .lock()
        .map_err(|_| "Failed to lock device manager".to_owned())?;

    if let Some(results) = dm.get_vsock_results() {
        if !results.is_empty() {
            // Get raw vsock bytes for HMAC verification (must match exactly
            // what the guest sent, avoiding any UTF-8 lossy conversion).
            let stdout_bytes = dm.get_vsock_stdout_bytes().unwrap_or_default();
            let stderr_bytes = dm.get_vsock_stderr_bytes().unwrap_or_default();
            let exit_code_bytes = dm.get_vsock_exit_code_bytes().unwrap_or_default();
            let hmac_data = dm.get_vsock_hmac();

            let hmac_verified = match (nonce, hmac_data) {
                (Some(n), Some(received_hmac)) => {
                    Some(verify_hmac(
                        n,
                        stdout_bytes,
                        stderr_bytes,
                        exit_code_bytes,
                        &received_hmac,
                    ))
                }
                (Some(_), None) => Some(false), // Nonce provided but no HMAC received
                (None, _) => None,              // No nonce, skip verification
            };

            return Ok(crate::VmResults {
                output: results,
                hmac_verified,
                transport: crate::Transport::Vsock,
            });
        }
    }
    drop(dm);

    // Fall back to serial output
    let serial = serial_output
        .lock()
        .map_err(|_| "Failed to lock serial output".to_owned())?;
    let raw = String::from_utf8_lossy(&serial);

    // Parse structured output from serial markers (written by bencher-init).
    let output = parse_serial_output(&raw);

    // Try to extract HMAC from serial markers
    let hmac_verified = nonce.map(|n| {
        if let Some(hmac_hex) = parse_serial_hmac(&raw) {
            // For serial, reconstruct the data the guest signed
            let stdout = parse_serial_section(&raw, "STDOUT");
            let stderr = parse_serial_section(&raw, "STDERR");
            let exit_code = parse_serial_exit_code(&raw);
            let received_hmac = hex_decode(&hmac_hex);
            verify_hmac(
                n,
                stdout.as_bytes(),
                stderr.as_bytes(),
                exit_code.as_bytes(),
                &received_hmac,
            )
        } else {
            false // Nonce provided but no HMAC in serial output
        }
    });

    Ok(crate::VmResults {
        output,
        hmac_verified,
        transport: crate::Transport::Serial,
    })
}

/// Verify an HMAC-SHA256 tag over the benchmark results.
///
/// The HMAC is computed over: `stdout_bytes || stderr_bytes || exit_code_bytes`
/// using the nonce as the key.
pub fn verify_hmac(
    nonce: &str,
    stdout: &[u8],
    stderr: &[u8],
    exit_code: &[u8],
    received_hmac: &[u8],
) -> bool {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    type HmacSha256 = Hmac<Sha256>;

    let Ok(mut mac) = HmacSha256::new_from_slice(nonce.as_bytes()) else {
        return false;
    };
    mac.update(stdout);
    mac.update(stderr);
    mac.update(exit_code);

    mac.verify_slice(received_hmac).is_ok()
}

/// Parse HMAC hex from serial output markers.
///
/// Looks for `---BENCHER_HMAC:<hex>---` in the serial stream.
fn parse_serial_hmac(raw: &str) -> Option<String> {
    const HMAC_BEGIN: &str = "---BENCHER_HMAC:";
    const HMAC_END: &str = "---";

    let start = raw.find(HMAC_BEGIN)?;
    let hex_start = start + HMAC_BEGIN.len();
    let remaining = &raw[hex_start..];
    let end = remaining.find(HMAC_END)?;
    Some(remaining[..end].to_owned())
}

/// Parse a section from serial markers (STDOUT or STDERR content).
///
/// The serial format is: `---BENCHER_<NAME>_BEGIN---\n<raw_bytes>\n---BENCHER_<NAME>_END---`
/// The `\n` after BEGIN and before END are separators, not part of the content.
/// We strip exactly one trailing `\n` (the separator) to recover the original raw bytes.
fn parse_serial_section(raw: &str, name: &str) -> String {
    let begin_marker = format!("---BENCHER_{name}_BEGIN---");
    let end_marker = format!("---BENCHER_{name}_END---");

    if let Some(begin) = raw.find(&begin_marker) {
        let after_begin = begin + begin_marker.len();
        let content_start = if raw[after_begin..].starts_with('\n') {
            after_begin + 1
        } else {
            after_begin
        };
        if let Some(end) = raw[content_start..].find(&end_marker) {
            let content = &raw[content_start..content_start + end];
            // Strip exactly one trailing \n (the separator before the END marker).
            // This preserves the raw bytes exactly as the guest sent them.
            return content.strip_suffix('\n').unwrap_or(content).to_owned();
        }
    }
    String::new()
}

/// Parse exit code from serial markers.
fn parse_serial_exit_code(raw: &str) -> String {
    const MARKER: &str = "---BENCHER_EXIT_CODE:";
    if let Some(start) = raw.find(MARKER) {
        let code_start = start + MARKER.len();
        if let Some(end) = raw[code_start..].find("---") {
            return raw[code_start..code_start + end].to_owned();
        }
    }
    String::new()
}

/// Decode a hex string to bytes.
fn hex_decode(hex: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(hex.len() / 2);
    let mut chars = hex.chars();
    while let (Some(a), Some(b)) = (chars.next(), chars.next()) {
        if let (Some(hi), Some(lo)) = (a.to_digit(16), b.to_digit(16)) {
            bytes.push((hi as u8) << 4 | lo as u8);
        }
    }
    bytes
}

/// Parse structured benchmark output from raw serial stream.
///
/// Extracts the stdout content between `---BENCHER_STDOUT_BEGIN---` and
/// `---BENCHER_STDOUT_END---` markers. If no markers are found, returns
/// the raw serial output as-is (for debugging).
fn parse_serial_output(raw: &str) -> String {
    const STDOUT_BEGIN: &str = "---BENCHER_STDOUT_BEGIN---";
    const STDOUT_END: &str = "---BENCHER_STDOUT_END---";

    if let Some(begin) = raw.find(STDOUT_BEGIN) {
        let after_begin = begin + STDOUT_BEGIN.len();
        // Skip the newline after the begin marker
        let content_start = if raw[after_begin..].starts_with('\n') {
            after_begin + 1
        } else {
            after_begin
        };

        if let Some(end) = raw[content_start..].find(STDOUT_END) {
            let content = &raw[content_start..content_start + end];
            // Strip exactly one trailing \n (the separator before the END marker)
            return content.strip_suffix('\n').unwrap_or(content).to_owned();
        }
    }

    // No markers found - return raw serial output for debugging
    raw.to_owned()
}

/// Check if the serial output buffer contains the exit code marker.
fn serial_has_exit_marker(buf: &[u8]) -> bool {
    if buf.len() < SERIAL_EXIT_MARKER.len() {
        return false;
    }
    // Search only the last 1024 bytes for performance
    let search_start = buf.len().saturating_sub(1024);
    buf[search_start..]
        .windows(SERIAL_EXIT_MARKER.len())
        .any(|w| w == SERIAL_EXIT_MARKER)
}

/// Extend a buffer with new data, respecting a maximum size limit.
///
/// If adding the new data would exceed the limit, only as much data as
/// fits within the limit is added. This prevents unbounded memory growth
/// from malicious workloads that generate excessive output.
fn extend_with_limit(buffer: &mut Vec<u8>, data: &[u8], max_size: usize) {
    if buffer.len() >= max_size {
        return; // Already at limit
    }
    let remaining = max_size - buffer.len();
    let to_add = data.len().min(remaining);
    buffer.extend_from_slice(&data[..to_add]);
}

/// No-op signal handler for SIGALRM.
///
/// When the timeout expires, we send SIGALRM to interrupt blocked KVM ioctls.
/// This handler does nothing - its purpose is to prevent the default handler
/// (which terminates the process) from running.
extern "C" fn noop_signal_handler(_signum: libc::c_int) {
    // Intentionally empty. The signal delivery itself is what interrupts
    // the blocking KVM ioctl, causing it to return EINTR.
}

/// Result of handling a VM exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmExitAction {
    /// Continue running the vCPU.
    Continue,

    /// The VM should shut down.
    Shutdown,
}
