//! Virtual device emulation.
//!
//! This module provides the virtual devices needed for running benchmarks:
//! - Serial console (UART 16550A) for kernel output
//! - i8042 keyboard controller for shutdown signaling
//! - virtio-blk for the rootfs
//! - virtio-vsock for host-guest communication

mod i8042;
mod serial;
mod virtio_blk;
mod virtio_vsock;

pub use i8042::I8042Device;
pub use serial::SerialDevice;
pub use virtio_blk::VirtioBlkDevice;
pub use virtio_vsock::VirtioVsockDevice;

use camino::Utf8Path;
use kvm_ioctls::VmFd;

use crate::error::VmmError;

/// I/O port for the first serial port (COM1).
pub const SERIAL_PORT_BASE: u16 = 0x3f8;
pub const SERIAL_PORT_END: u16 = 0x3ff;

/// I/O port for the i8042 keyboard controller.
pub const I8042_DATA_PORT: u16 = 0x60;
pub const I8042_COMMAND_PORT: u16 = 0x64;

/// MMIO base address for virtio devices.
pub const VIRTIO_MMIO_BASE: u64 = 0xd000_0000;
pub const VIRTIO_MMIO_SIZE: u64 = 0x1000; // 4 KiB per device

/// Setup all required virtual devices.
pub fn setup_devices(_vm_fd: &VmFd, rootfs_path: &Utf8Path) -> Result<DeviceManager, VmmError> {
    let mut manager = DeviceManager::new()?;

    // Setup virtio-blk with the rootfs
    manager.setup_virtio_blk(rootfs_path)?;

    Ok(manager)
}

/// Manages all virtual devices for a VM.
pub struct DeviceManager {
    /// Serial console device.
    pub serial: SerialDevice,

    /// i8042 keyboard controller (for shutdown).
    pub i8042: I8042Device,

    /// virtio-blk device (for rootfs).
    pub virtio_blk: Option<VirtioBlkDevice>,

    /// virtio-vsock device (for host-guest communication).
    pub virtio_vsock: Option<VirtioVsockDevice>,
}

impl DeviceManager {
    /// Create a new device manager with default devices.
    pub fn new() -> Result<Self, VmmError> {
        Ok(Self {
            serial: SerialDevice::new()?,
            i8042: I8042Device::new(),
            virtio_blk: None,
            virtio_vsock: None,
        })
    }

    /// Setup virtio-blk device with a rootfs image.
    pub fn setup_virtio_blk(&mut self, rootfs_path: &Utf8Path) -> Result<(), VmmError> {
        let device = VirtioBlkDevice::new(rootfs_path, true)?;
        self.virtio_blk = Some(device);
        Ok(())
    }

    /// Setup virtio-vsock device for host-guest communication.
    #[expect(dead_code)]
    pub fn setup_virtio_vsock(&mut self, guest_cid: u64, socket_path: &Utf8Path) -> Result<(), VmmError> {
        let device = VirtioVsockDevice::new(guest_cid, socket_path)?;
        self.virtio_vsock = Some(device);
        Ok(())
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
                blk.write(offset, data);
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
}
