//! VM event loop.
//!
//! This module handles the main execution loop for the VM, processing
//! vCPU exits and device I/O.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;

use kvm_ioctls::VcpuExit;

use crate::devices::DeviceManager;
use crate::error::VmmError;
use crate::vcpu::Vcpu;

/// Run the VM event loop.
///
/// This runs the vCPUs and handles exits until the VM shuts down.
///
/// # Arguments
///
/// * `vcpus` - The virtual CPUs to run
/// * `devices` - The device manager for handling I/O
///
/// # Returns
///
/// The benchmark results collected from the guest serial output.
pub fn run(
    vcpus: &mut [Vcpu],
    devices: Arc<Mutex<DeviceManager>>,
) -> Result<String, VmmError> {
    // Flag to signal all vCPUs to stop
    let shutdown = Arc::new(AtomicBool::new(false));

    // For single vCPU (common case), run in the current thread
    if vcpus.len() == 1 {
        return run_vcpu_loop(&mut vcpus[0], Arc::clone(&devices), Arc::clone(&shutdown));
    }

    // For multiple vCPUs, spawn threads
    // Note: This is a simplified implementation. A production VMM would use
    // proper thread synchronization and handle vCPU affinity.
    let handles: Vec<_> = vcpus
        .iter_mut()
        .map(|vcpu| {
            let devices = Arc::clone(&devices);
            let shutdown = Arc::clone(&shutdown);
            let vcpu_fd = vcpu.fd.try_clone().map_err(VmmError::Kvm)?;
            let vcpu_index = vcpu.index;

            Ok(thread::spawn(move || {
                let mut vcpu = Vcpu {
                    fd: vcpu_fd,
                    index: vcpu_index,
                };
                run_vcpu_loop(&mut vcpu, devices, shutdown)
            }))
        })
        .collect::<Result<Vec<_>, VmmError>>()?;

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
            Ok(VmExitAction::Continue)
        }

        VcpuExit::MmioWrite(addr, data) => {
            let mut dm = devices.lock().map_err(|_| {
                VmmError::Device("Failed to lock device manager".to_owned())
            })?;
            dm.handle_mmio_write(addr, data);

            // After MMIO write, poll vsock for any pending activity
            dm.poll_vsock();

            Ok(VmExitAction::Continue)
        }

        VcpuExit::Hlt => {
            // CPU is halted, waiting for interrupt
            // In a simple VMM, we can treat this as shutdown
            // A production VMM would wait for interrupts
            Ok(VmExitAction::Shutdown)
        }

        VcpuExit::Shutdown => {
            Ok(VmExitAction::Shutdown)
        }

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
        _ => {
            // Unknown exit, continue for now
            // A production VMM would log this
            Ok(VmExitAction::Continue)
        }
    }
}

/// VM exit reasons we handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmExitReason {
    /// I/O port access.
    Io,

    /// Memory-mapped I/O access.
    Mmio,

    /// CPU halted.
    Hlt,

    /// VM shutdown requested.
    Shutdown,

    /// Unknown exit reason.
    Unknown(u32),
}

/// Result of handling a VM exit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VmExitAction {
    /// Continue running the vCPU.
    Continue,

    /// The VM should shut down.
    Shutdown,
}
