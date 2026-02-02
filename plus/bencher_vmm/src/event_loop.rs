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
    let timeout_handle = if timeout_secs > 0 {
        let shutdown_clone = Arc::clone(&shutdown);
        let timed_out_clone = Arc::clone(&timed_out);
        Some(thread::spawn(move || {
            thread::sleep(Duration::from_secs(timeout_secs));
            if !shutdown_clone.load(Ordering::Relaxed) {
                timed_out_clone.store(true, Ordering::Relaxed);
                shutdown_clone.store(true, Ordering::Relaxed);
            }
        }))
    } else {
        None
    };

    // For single vCPU (common case), run in the current thread
    let result = if vcpus.len() == 1 {
        run_vcpu_loop(&mut vcpus[0], Arc::clone(&devices), Arc::clone(&shutdown))
    } else {
        // For multiple vCPUs, spawn threads
        // Note: This is a simplified implementation. A production VMM would use
        // proper thread synchronization and handle vCPU affinity.
        let handles: Vec<_> = vcpus
            .into_iter()
            .map(|mut vcpu| {
                let devices = Arc::clone(&devices);
                let shutdown = Arc::clone(&shutdown);

                thread::spawn(move || run_vcpu_loop(&mut vcpu, devices, shutdown))
            })
            .collect();

        // Wait for all vCPU threads to complete
        let mut result = Ok(String::new());
        for handle in handles {
            match handle.join() {
                Ok(Ok(output)) => {
                    if !output.is_empty() {
                        result = Ok(output);
                    }
                }
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

    // Clean up timeout thread
    if let Some(handle) = timeout_handle {
        // Signal shutdown to stop the timeout thread if it's still waiting
        shutdown.store(true, Ordering::Relaxed);
        drop(handle); // Don't wait for it, just drop
    }

    // Check if we timed out
    if timed_out.load(Ordering::Relaxed) {
        return Err(VmmError::Timeout(timeout_secs));
    }

    result
}

/// Run the main loop for a single vCPU.
fn run_vcpu_loop(
    vcpu: &mut Vcpu,
    devices: Arc<Mutex<DeviceManager>>,
    shutdown: Arc<AtomicBool>,
) -> Result<String, VmmError> {
    let mut serial_output = Vec::new();

    loop {
        // Check if we should stop
        if shutdown.load(Ordering::Relaxed) {
            break;
        }

        // Run the vCPU until it exits
        match vcpu.fd.run() {
            Ok(exit_reason) => {
                let action = handle_vcpu_exit(exit_reason, &devices, &mut serial_output)?;
                match action {
                    VmExitAction::Continue => continue,
                    VmExitAction::Shutdown => {
                        shutdown.store(true, Ordering::Relaxed);
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
    let output = String::from_utf8_lossy(&serial_output).to_string();
    Ok(output)
}

/// Handle a vCPU exit.
fn handle_vcpu_exit(
    exit: VcpuExit,
    devices: &Arc<Mutex<DeviceManager>>,
    serial_output: &mut Vec<u8>,
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

            // Collect serial output
            let output = dm.get_serial_output();
            serial_output.extend(output);

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

            // Collect any serial output
            let output = dm.get_serial_output();
            serial_output.extend(output);

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
            // Collect any serial output
            let output = dm.get_serial_output();
            serial_output.extend(output);

            Ok(VmExitAction::Continue)
        }
    }
}

/// Result of handling a VM exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmExitAction {
    /// Continue running the vCPU.
    Continue,

    /// The VM should shut down.
    Shutdown,
}
