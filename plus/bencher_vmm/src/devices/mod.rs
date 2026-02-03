//! Virtual device emulation.
//!
//! This module provides the virtual devices needed for running benchmarks:
//! - Serial console (UART 16550A) for kernel output
//! - i8042 keyboard controller for shutdown signaling
//! - virtio-blk for the rootfs
//! - virtio-vsock for host-guest communication

mod i8042;
mod pit;
mod serial;
mod virtio_blk;
mod virtio_vsock;

pub use i8042::I8042Device;
pub use pit::PitDevice;
pub use serial::SerialDevice;
pub use virtio_blk::VirtioBlkDevice;
pub use virtio_vsock::VirtioVsockDevice;

use std::sync::Arc;

use camino::Utf8Path;
use kvm_ioctls::VmFd;
use vm_memory::GuestMemoryMmap;

use crate::error::VmmError;

/// IRQ number for virtio-blk (as specified in kernel cmdline).
pub const VIRTIO_BLK_IRQ: u32 = 5;

/// IRQ number for virtio-vsock.
pub const VIRTIO_VSOCK_IRQ: u32 = 6;

/// I/O port for the first serial port (COM1).
pub const SERIAL_PORT_BASE: u16 = 0x3f8;
pub const SERIAL_PORT_END: u16 = 0x3ff;

/// I/O port for the i8042 keyboard controller.
pub const I8042_DATA_PORT: u16 = 0x60;
pub const I8042_COMMAND_PORT: u16 = 0x64;

/// Timer IRQ (IRQ0 from PIT).
pub const TIMER_IRQ: u32 = 0;

/// MMIO base address for virtio devices.
pub const VIRTIO_MMIO_BASE: u64 = 0xd000_0000;
pub const VIRTIO_MMIO_SIZE: u64 = 0x1000; // 4 KiB per device

/// Guest CID for vsock connections.
pub const GUEST_CID: u64 = 3;

/// Setup all required virtual devices.
pub fn setup_devices(
    _vm_fd: &Arc<VmFd>,
    rootfs_path: &Utf8Path,
    vsock_path: Option<&Utf8Path>,
) -> Result<DeviceManager, VmmError> {
    let mut manager = DeviceManager::new()?;

    // Setup virtio-blk with the rootfs
    manager.setup_virtio_blk(rootfs_path)?;

    // Setup virtio-vsock if requested
    if let Some(path) = vsock_path {
        manager.setup_virtio_vsock(GUEST_CID, path)?;
    }

    Ok(manager)
}

/// Manages all virtual devices for a VM.
pub struct DeviceManager {
    /// Serial console device.
    pub serial: SerialDevice,

    /// i8042 keyboard controller (for shutdown).
    pub i8042: I8042Device,

    /// Programmable Interval Timer (for timer interrupts).
    pub pit: PitDevice,

    /// virtio-blk device (for rootfs).
    pub virtio_blk: Option<VirtioBlkDevice>,

    /// virtio-vsock device (for host-guest communication).
    pub virtio_vsock: Option<VirtioVsockDevice>,

    /// VM file descriptor for interrupt injection.
    vm_fd: Option<Arc<VmFd>>,
}

impl DeviceManager {
    /// Create a new device manager with default devices.
    pub fn new() -> Result<Self, VmmError> {
        Ok(Self {
            serial: SerialDevice::new()?,
            i8042: I8042Device::new(),
            pit: PitDevice::new(),
            virtio_blk: None,
            virtio_vsock: None,
            vm_fd: None,
        })
    }

    /// Set the VM file descriptor for interrupt injection.
    pub fn set_vm_fd(&mut self, vm_fd: Arc<VmFd>) {
        self.vm_fd = Some(vm_fd);
    }

    /// Inject an IRQ to the guest.
    fn inject_irq(&self, irq: u32) {
        if let Some(ref vm_fd) = self.vm_fd {
            if let Err(e) = vm_fd.set_irq_line(irq, true) {
                eprintln!("[DEVICES] Failed to assert IRQ {irq}: {e}");
            }
            // De-assert after a brief moment (edge-triggered)
            if let Err(e) = vm_fd.set_irq_line(irq, false) {
                eprintln!("[DEVICES] Failed to deassert IRQ {irq}: {e}");
            }
        }
    }

    /// Setup virtio-blk device with a rootfs image.
    pub fn setup_virtio_blk(&mut self, rootfs_path: &Utf8Path) -> Result<(), VmmError> {
        let device = VirtioBlkDevice::new(rootfs_path, false)?;
        self.virtio_blk = Some(device);
        Ok(())
    }

    /// Setup virtio-vsock device for host-guest communication.
    pub fn setup_virtio_vsock(&mut self, guest_cid: u64, socket_path: &Utf8Path) -> Result<(), VmmError> {
        let device = VirtioVsockDevice::new(guest_cid, socket_path)?;
        self.virtio_vsock = Some(device);
        Ok(())
    }

    /// Set guest memory for all virtio devices.
    ///
    /// This must be called before virtio queue processing can work.
    pub fn set_guest_memory(&mut self, mem: Arc<GuestMemoryMmap>) {
        if let Some(ref mut blk) = self.virtio_blk {
            blk.set_guest_memory(Arc::clone(&mem));
        }
        if let Some(ref mut vsock) = self.virtio_vsock {
            vsock.set_guest_memory(mem);
        }
    }

    /// Poll for vsock activity (accept connections, read data).
    ///
    /// This should be called periodically in the event loop.
    pub fn poll_vsock(&mut self) {
        if let Some(ref mut vsock) = self.virtio_vsock {
            vsock.poll();
        }
    }

    /// Check and inject timer interrupt if needed.
    ///
    /// This should be called periodically in the event loop.
    pub fn check_timer(&mut self) {
        if self.pit.check_interrupt() {
            self.inject_irq(TIMER_IRQ);
        }
    }

    /// Check if any virtio device has a pending interrupt.
    #[must_use]
    pub fn has_pending_virtio_interrupt(&self) -> bool {
        let blk_pending = self.virtio_blk.as_ref().is_some_and(|d| d.has_pending_interrupt());
        let vsock_pending = self.virtio_vsock.as_ref().is_some_and(|d| d.has_pending_interrupt());
        blk_pending || vsock_pending
    }

    /// Handle an I/O port read.
    pub fn handle_io_read(&mut self, port: u16, data: &mut [u8]) {
        match port {
            SERIAL_PORT_BASE..=SERIAL_PORT_END => {
                self.serial.read(port - SERIAL_PORT_BASE, data);
            }
            I8042_DATA_PORT | I8042_COMMAND_PORT => {
                self.i8042.read(port, data);
            }
            pit::PIT_CHANNEL_0..=pit::PIT_MODE_COMMAND => {
                self.pit.read(port, data);
            }
            _ => {
                // Unknown port, return 0xff
                data.fill(0xff);
            }
        }
    }

    /// Handle an I/O port write.
    ///
    /// Returns `true` if the VM should shut down.
    pub fn handle_io_write(&mut self, port: u16, data: &[u8]) -> bool {
        match port {
            SERIAL_PORT_BASE..=SERIAL_PORT_END => {
                self.serial.write(port - SERIAL_PORT_BASE, data);
                false
            }
            I8042_DATA_PORT | I8042_COMMAND_PORT => self.i8042.write(port, data),
            pit::PIT_CHANNEL_0..=pit::PIT_MODE_COMMAND => {
                self.pit.write(port, data);
                false
            }
            _ => {
                // Unknown port, ignore
                false
            }
        }
    }

    /// Handle an MMIO read.
    pub fn handle_mmio_read(&mut self, addr: u64, data: &mut [u8]) {
        // Check if this is a virtio-blk access
        if addr >= VIRTIO_MMIO_BASE && addr < VIRTIO_MMIO_BASE + VIRTIO_MMIO_SIZE {
            if let Some(ref mut blk) = self.virtio_blk {
                let offset = addr - VIRTIO_MMIO_BASE;
                blk.read(offset, data);
                return;
            }
        }

        // Check if this is a virtio-vsock access
        let vsock_base = VIRTIO_MMIO_BASE + VIRTIO_MMIO_SIZE;
        if addr >= vsock_base && addr < vsock_base + VIRTIO_MMIO_SIZE {
            if let Some(ref mut vsock) = self.virtio_vsock {
                let offset = addr - vsock_base;
                vsock.read(offset, data);
                return;
            }
        }

        // Unknown MMIO address, return zeros
        data.fill(0);
    }

    /// Handle an MMIO write.
    pub fn handle_mmio_write(&mut self, addr: u64, data: &[u8]) {
        // Check if this is a virtio-blk access
        if addr >= VIRTIO_MMIO_BASE && addr < VIRTIO_MMIO_BASE + VIRTIO_MMIO_SIZE {
            if let Some(ref mut blk) = self.virtio_blk {
                let offset = addr - VIRTIO_MMIO_BASE;
                let needs_irq = blk.write(offset, data);
                if needs_irq {
                    self.inject_irq(VIRTIO_BLK_IRQ);
                }
                return;
            }
        }

        // Check if this is a virtio-vsock access
        let vsock_base = VIRTIO_MMIO_BASE + VIRTIO_MMIO_SIZE;
        if addr >= vsock_base && addr < vsock_base + VIRTIO_MMIO_SIZE {
            if let Some(ref mut vsock) = self.virtio_vsock {
                let offset = addr - vsock_base;
                vsock.write(offset, data);
            }
        }
    }

    /// Get any output from the serial console.
    pub fn get_serial_output(&mut self) -> Vec<u8> {
        self.serial.take_output()
    }

    /// Check if vsock results are available.
    #[must_use]
    pub fn has_vsock_results(&self) -> bool {
        self.virtio_vsock.as_ref().is_some_and(|v| v.has_results())
    }

    /// Check if vsock results collection is complete (stdout port closed).
    #[must_use]
    pub fn vsock_results_complete(&self) -> bool {
        self.virtio_vsock.as_ref().is_some_and(|v| v.results_complete())
    }

    /// Check if all required vsock ports are complete (stdout, stderr, exit_code).
    #[must_use]
    pub fn vsock_all_required_complete(&self) -> bool {
        self.virtio_vsock.as_ref().is_some_and(|v| v.all_required_complete())
    }

    /// Get the vsock results as a string (stdout port).
    #[must_use]
    pub fn get_vsock_results(&self) -> Option<String> {
        self.virtio_vsock.as_ref().map(|v| v.results_as_string())
    }

    /// Take the vsock results, leaving an empty buffer (stdout port).
    pub fn take_vsock_results(&mut self) -> Option<Vec<u8>> {
        self.virtio_vsock.as_mut().map(|v| v.take_results())
    }

    /// Get stdout from the guest.
    #[must_use]
    pub fn get_vsock_stdout(&self) -> Option<String> {
        self.virtio_vsock.as_ref().map(|v| v.stdout())
    }

    /// Get raw stdout bytes from the guest.
    #[must_use]
    pub fn get_vsock_stdout_bytes(&self) -> Option<&[u8]> {
        self.virtio_vsock
            .as_ref()
            .and_then(|v| v.port_data_bytes(virtio_vsock::PORT_STDOUT))
    }

    /// Get raw stderr bytes from the guest.
    #[must_use]
    pub fn get_vsock_stderr_bytes(&self) -> Option<&[u8]> {
        self.virtio_vsock
            .as_ref()
            .and_then(|v| v.port_data_bytes(virtio_vsock::PORT_STDERR))
    }

    /// Get raw exit code bytes from the guest.
    #[must_use]
    pub fn get_vsock_exit_code_bytes(&self) -> Option<&[u8]> {
        self.virtio_vsock
            .as_ref()
            .and_then(|v| v.port_data_bytes(virtio_vsock::PORT_EXIT_CODE))
    }

    /// Get stderr from the guest.
    #[must_use]
    pub fn get_vsock_stderr(&self) -> Option<String> {
        self.virtio_vsock.as_ref().map(|v| v.stderr())
    }

    /// Get the exit code from the guest.
    #[must_use]
    pub fn get_vsock_exit_code(&self) -> Option<i32> {
        self.virtio_vsock.as_ref().and_then(|v| v.exit_code())
    }

    /// Get the HMAC data from the guest (if provided).
    #[must_use]
    pub fn get_vsock_hmac(&self) -> Option<Vec<u8>> {
        self.virtio_vsock.as_ref().and_then(|v| v.hmac_data())
    }

    /// Get the output file data from the guest (if provided).
    #[must_use]
    pub fn get_vsock_output_file(&self) -> Option<Vec<u8>> {
        self.virtio_vsock.as_ref().and_then(|v| v.output_file())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use vm_memory::{GuestAddress, GuestMemoryMmap};

    /// Create test guest memory (1 MiB).
    fn create_test_memory() -> Arc<GuestMemoryMmap> {
        Arc::new(
            GuestMemoryMmap::from_ranges(&[(GuestAddress(0), 1024 * 1024)])
                .expect("Failed to create test memory"),
        )
    }

    /// Create a temporary file for testing virtio-blk.
    fn create_test_disk() -> (tempfile::NamedTempFile, camino::Utf8PathBuf) {
        let mut file = tempfile::NamedTempFile::new().expect("Failed to create temp file");
        // Write some test data (at least one sector)
        let data = vec![0u8; 512];
        file.write_all(&data).expect("Failed to write test data");
        file.flush().expect("Failed to flush");
        let path = camino::Utf8PathBuf::from(file.path().to_string_lossy().to_string());
        (file, path)
    }

    #[test]
    fn test_device_manager_new() {
        let dm = DeviceManager::new().unwrap();
        assert!(dm.virtio_blk.is_none());
        assert!(dm.virtio_vsock.is_none());
        assert!(!dm.has_pending_virtio_interrupt());
    }

    #[test]
    fn test_device_manager_setup_virtio_blk() {
        let (_file, path) = create_test_disk();
        let mut dm = DeviceManager::new().unwrap();

        dm.setup_virtio_blk(&path).unwrap();
        assert!(dm.virtio_blk.is_some());
    }

    #[test]
    fn test_device_manager_set_guest_memory() {
        let (_file, path) = create_test_disk();
        let mut dm = DeviceManager::new().unwrap();
        dm.setup_virtio_blk(&path).unwrap();

        let mem = create_test_memory();
        dm.set_guest_memory(mem);

        // After setting memory, the device should be ready for queue processing
        // (though we can't fully test that without a running guest)
    }

    #[test]
    fn test_device_manager_virtio_mmio_magic() {
        let (_file, path) = create_test_disk();
        let mut dm = DeviceManager::new().unwrap();
        dm.setup_virtio_blk(&path).unwrap();

        // Read the magic value from virtio-blk MMIO
        let mut data = [0u8; 4];
        dm.handle_mmio_read(VIRTIO_MMIO_BASE, &mut data);

        let magic = u32::from_le_bytes(data);
        assert_eq!(magic, 0x7472_6976, "Expected virtio magic value 'virt'");
    }

    #[test]
    fn test_device_manager_virtio_device_id() {
        let (_file, path) = create_test_disk();
        let mut dm = DeviceManager::new().unwrap();
        dm.setup_virtio_blk(&path).unwrap();

        // Read the device ID (offset 0x08)
        let mut data = [0u8; 4];
        dm.handle_mmio_read(VIRTIO_MMIO_BASE + 0x08, &mut data);

        let device_id = u32::from_le_bytes(data);
        assert_eq!(device_id, 2, "Expected virtio-blk device ID (2)");
    }

    #[test]
    fn test_device_manager_serial_output() {
        let mut dm = DeviceManager::new().unwrap();

        // Write to serial port (transmit register is at offset 0)
        dm.handle_io_write(SERIAL_PORT_BASE, &[b'H']);
        dm.handle_io_write(SERIAL_PORT_BASE, &[b'i']);

        let output = dm.get_serial_output();
        assert_eq!(&output, b"Hi");
    }

    #[test]
    fn test_device_manager_unknown_mmio() {
        let mut dm = DeviceManager::new().unwrap();

        // Read from an unknown MMIO address
        let mut data = [0xffu8; 4];
        dm.handle_mmio_read(0x1234_5678, &mut data);

        // Should return zeros for unknown addresses
        assert_eq!(data, [0, 0, 0, 0]);
    }

    #[test]
    fn test_device_manager_poll_vsock_no_device() {
        let mut dm = DeviceManager::new().unwrap();

        // poll_vsock should not panic when no vsock device is configured
        dm.poll_vsock();
    }
}
