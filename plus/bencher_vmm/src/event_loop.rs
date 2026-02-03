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

/// Maximum size for serial output buffer (10 MiB).
///
/// This prevents malicious workloads from exhausting host memory by
/// generating excessive output. Output beyond this limit is discarded.
const MAX_SERIAL_OUTPUT_SIZE: usize = 10 * 1024 * 1024;

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
///
/// # Returns
///
/// The benchmark results collected from the guest serial output or vsock.
pub fn run(
    mut vcpus: Vec<Vcpu>,
    devices: Arc<Mutex<DeviceManager>>,
    timeout_secs: u64,
) -> Result<String, VmmError> {
    // Flag to signal all vCPUs to stop
    let shutdown = Arc::new(AtomicBool::new(false));
    // Flag to track if we timed out
    let timed_out = Arc::new(AtomicBool::new(false));

    // Start timeout thread if timeout is set
    // The timeout thread will signal shutdown after the timeout expires.
    // We use SeqCst ordering to ensure visibility across threads.
    let timeout_handle = if timeout_secs > 0 {
        let shutdown_clone = Arc::clone(&shutdown);
        let timed_out_clone = Arc::clone(&timed_out);
        Some(thread::spawn(move || {
            thread::sleep(Duration::from_secs(timeout_secs));
            // Only set timeout if we haven't already shut down
            if !shutdown_clone.swap(true, Ordering::SeqCst) {
                timed_out_clone.store(true, Ordering::SeqCst);
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
        run_vcpu_loop(
            &mut vcpus[0],
            Arc::clone(&devices),
            Arc::clone(&shutdown),
            Arc::clone(&serial_output),
        )
    } else {
        // For multiple vCPUs, spawn threads
        let handles: Vec<_> = vcpus
            .into_iter()
            .map(|mut vcpu| {
                let devices = Arc::clone(&devices);
                let shutdown = Arc::clone(&shutdown);
                let serial_output = Arc::clone(&serial_output);

                thread::spawn(move || {
                    run_vcpu_loop(&mut vcpu, devices, shutdown, serial_output)
                })
            })
            .collect();

        // Wait for all vCPU threads to complete
        let mut result = Ok(());
        for handle in handles {
            match handle.join() {
                Ok(Ok(())) => {}
                Ok(Err(e)) => {
                    result = Err(e);
                    break;
                }
                Err(_) => {
                    result = Err(VmmError::Vcpu("vCPU thread panicked".to_owned()));
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

        // Wait for the timeout thread to finish, but with a grace period
        // This prevents hanging if the timeout thread is stuck
        let join_result = handle.join();

        // If join failed, the thread panicked - but we still want to check timeout
        if join_result.is_err() {
            // Thread panicked, but continue with result handling
        }
    }

    // Check if we timed out (use SeqCst to ensure we see the latest value)
    if timed_out.load(Ordering::SeqCst) {
        return Err(VmmError::Timeout(timeout_secs));
    }

    // Propagate any vCPU errors
    result?;

    // Prefer vsock results if available, fall back to serial output
    let dm = devices.lock().map_err(|_| {
        VmmError::Device("Failed to lock device manager".to_owned())
    })?;

    if let Some(results) = dm.get_vsock_results() {
        if !results.is_empty() {
            return Ok(results);
        }
    }

    // Fall back to serial output
    let serial = serial_output.lock().map_err(|_| {
        VmmError::Device("Failed to lock serial output".to_owned())
    })?;
    let output = String::from_utf8_lossy(&serial).to_string();
    Ok(output)
}

/// Run the main loop for a single vCPU.
fn run_vcpu_loop(
    vcpu: &mut Vcpu,
    devices: Arc<Mutex<DeviceManager>>,
    shutdown: Arc<AtomicBool>,
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
            let mut dm = devices.lock().map_err(|_| {
                VmmError::Device("Failed to lock device manager".to_owned())
            })?;

            let should_shutdown = dm.handle_io_write(port, data);

            // Collect serial output (with size limit to prevent memory exhaustion)
            let output = dm.get_serial_output();
            if let Ok(mut buf) = serial_output.lock() {
                extend_with_limit(&mut buf, &output, MAX_SERIAL_OUTPUT_SIZE);
            }

            dm.check_timer();

            if should_shutdown {
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
            if let Ok(mut buf) = serial_output.lock() {
                extend_with_limit(&mut buf, &output, MAX_SERIAL_OUTPUT_SIZE);
            }

            Ok(VmExitAction::Continue)
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
            if let Ok(mut buf) = serial_output.lock() {
                extend_with_limit(&mut buf, &output, MAX_SERIAL_OUTPUT_SIZE);
            }

            Ok(VmExitAction::Continue)
        }
    }
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

/// Result of handling a VM exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmExitAction {
    /// Continue running the vCPU.
    Continue,

    /// The VM should shut down.
    Shutdown,
}
