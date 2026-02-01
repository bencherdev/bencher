//! virtio-blk device emulation.
//!
//! This provides a block device for mounting the rootfs in the guest.
//! The device is backed by a file on the host (the squashfs image).
//!
//! This implements the virtio-mmio transport as defined in the virtio spec.
//! See: https://docs.oasis-open.org/virtio/virtio/v1.1/virtio-v1.1.html

use std::fs::File;
use std::io::{Read, Seek, SeekFrom};

use camino::Utf8Path;

use crate::error::VmmError;

/// virtio device type for block devices.
pub const VIRTIO_BLK_DEVICE_TYPE: u32 = 2;

/// virtio MMIO magic value ("virt").
const VIRTIO_MMIO_MAGIC: u32 = 0x7472_6976;

/// virtio MMIO version (legacy = 1, modern = 2).
const VIRTIO_MMIO_VERSION: u32 = 2;

/// virtio vendor ID (we use a placeholder).
const VIRTIO_VENDOR_ID: u32 = 0x554d_4551; // "QEMU" backwards

/// MMIO register offsets.
mod regs {
    pub const MAGIC_VALUE: u64 = 0x00;
    pub const VERSION: u64 = 0x04;
    pub const DEVICE_ID: u64 = 0x08;
    pub const VENDOR_ID: u64 = 0x0c;
    pub const DEVICE_FEATURES: u64 = 0x10;
    pub const DEVICE_FEATURES_SEL: u64 = 0x14;
    pub const DRIVER_FEATURES: u64 = 0x20;
    pub const DRIVER_FEATURES_SEL: u64 = 0x24;
    pub const QUEUE_SEL: u64 = 0x30;
    pub const QUEUE_NUM_MAX: u64 = 0x34;
    pub const QUEUE_NUM: u64 = 0x38;
    pub const QUEUE_READY: u64 = 0x44;
    pub const QUEUE_NOTIFY: u64 = 0x50;
    pub const INTERRUPT_STATUS: u64 = 0x60;
    pub const INTERRUPT_ACK: u64 = 0x64;
    pub const STATUS: u64 = 0x70;
    pub const QUEUE_DESC_LOW: u64 = 0x80;
    pub const QUEUE_DESC_HIGH: u64 = 0x84;
    pub const QUEUE_DRIVER_LOW: u64 = 0x90;
    pub const QUEUE_DRIVER_HIGH: u64 = 0x94;
    pub const QUEUE_DEVICE_LOW: u64 = 0xa0;
    pub const QUEUE_DEVICE_HIGH: u64 = 0xa4;
    pub const CONFIG_GENERATION: u64 = 0xfc;
    pub const CONFIG: u64 = 0x100;
}

/// virtio-blk feature bits.
mod features {
    pub const VIRTIO_BLK_F_SIZE_MAX: u64 = 1 << 1;
    pub const VIRTIO_BLK_F_SEG_MAX: u64 = 1 << 2;
    pub const VIRTIO_BLK_F_RO: u64 = 1 << 5;
    pub const VIRTIO_BLK_F_BLK_SIZE: u64 = 1 << 6;
    pub const VIRTIO_F_VERSION_1: u64 = 1 << 32;
}

/// A virtio block device.
pub struct VirtioBlkDevice {
    /// The backing file for the block device.
    file: File,

    /// The size of the backing file in bytes.
    size: u64,

    /// Whether the device is read-only.
    read_only: bool,

    /// Device status register.
    status: u32,

    /// Selected feature page (0 or 1).
    features_sel: u32,

    /// Driver-acknowledged features (low 32 bits).
    driver_features_low: u32,

    /// Driver-acknowledged features (high 32 bits).
    driver_features_high: u32,

    /// Selected queue.
    queue_sel: u32,

    /// Interrupt status.
    interrupt_status: u32,

    /// Queue configuration.
    queue_num: u32,
    queue_ready: u32,
    queue_desc_low: u32,
    queue_desc_high: u32,
    queue_driver_low: u32,
    queue_driver_high: u32,
    queue_device_low: u32,
    queue_device_high: u32,
}

impl VirtioBlkDevice {
    /// Create a new virtio-blk device backed by a file.
    pub fn new(path: &Utf8Path, read_only: bool) -> Result<Self, VmmError> {
        let file = if read_only {
            File::open(path)?
        } else {
            File::options().read(true).write(true).open(path)?
        };

        let metadata = file.metadata()?;
        let size = metadata.len();

        Ok(Self {
            file,
            size,
            read_only,
            status: 0,
            features_sel: 0,
            driver_features_low: 0,
            driver_features_high: 0,
            queue_sel: 0,
            interrupt_status: 0,
            queue_num: 0,
            queue_ready: 0,
            queue_desc_low: 0,
            queue_desc_high: 0,
            queue_driver_low: 0,
            queue_driver_high: 0,
            queue_device_low: 0,
            queue_device_high: 0,
        })
    }

    /// Get the size of the block device in 512-byte sectors.
    pub fn capacity_sectors(&self) -> u64 {
        self.size / 512
    }

    /// Check if the device is read-only.
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Get the device features.
    fn device_features(&self) -> u64 {
        let mut features = features::VIRTIO_F_VERSION_1
            | features::VIRTIO_BLK_F_SIZE_MAX
            | features::VIRTIO_BLK_F_SEG_MAX
            | features::VIRTIO_BLK_F_BLK_SIZE;

        if self.read_only {
            features |= features::VIRTIO_BLK_F_RO;
        }

        features
    }

    /// Read from MMIO registers.
    pub fn read(&mut self, offset: u64, data: &mut [u8]) {
        let value: u32 = match offset {
            regs::MAGIC_VALUE => VIRTIO_MMIO_MAGIC,
            regs::VERSION => VIRTIO_MMIO_VERSION,
            regs::DEVICE_ID => VIRTIO_BLK_DEVICE_TYPE,
            regs::VENDOR_ID => VIRTIO_VENDOR_ID,
            regs::DEVICE_FEATURES => {
                let features = self.device_features();
                if self.features_sel == 0 {
                    features as u32
                } else {
                    (features >> 32) as u32
                }
            }
            regs::QUEUE_NUM_MAX => 256, // Maximum queue size
            regs::QUEUE_READY => self.queue_ready,
            regs::INTERRUPT_STATUS => self.interrupt_status,
            regs::STATUS => self.status,
            regs::CONFIG_GENERATION => 0,
            // Config space: capacity (8 bytes at offset 0x100)
            regs::CONFIG => self.capacity_sectors() as u32,
            0x104 => (self.capacity_sectors() >> 32) as u32,
            // size_max
            0x108 => 0x0010_0000, // 1 MiB
            // seg_max
            0x10c => 128,
            // blk_size
            0x114 => 512,
            _ => 0,
        };

        // Write the value to the data buffer (little-endian)
        let len = data.len().min(4);
        data[..len].copy_from_slice(&value.to_le_bytes()[..len]);
    }

    /// Write to MMIO registers.
    pub fn write(&mut self, offset: u64, data: &[u8]) {
        if data.len() < 4 {
            return;
        }

        let value = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);

        match offset {
            regs::DEVICE_FEATURES_SEL => {
                self.features_sel = value;
            }
            regs::DRIVER_FEATURES => {
                if self.features_sel == 0 {
                    self.driver_features_low = value;
                } else {
                    self.driver_features_high = value;
                }
            }
            regs::DRIVER_FEATURES_SEL => {
                self.features_sel = value;
            }
            regs::QUEUE_SEL => {
                self.queue_sel = value;
            }
            regs::QUEUE_NUM => {
                self.queue_num = value;
            }
            regs::QUEUE_READY => {
                self.queue_ready = value;
            }
            regs::QUEUE_NOTIFY => {
                // Guest is notifying us that there's work to do
                self.handle_queue_notify();
            }
            regs::INTERRUPT_ACK => {
                self.interrupt_status &= !value;
            }
            regs::STATUS => {
                self.status = value;
                if value == 0 {
                    // Device reset
                    self.reset();
                }
            }
            regs::QUEUE_DESC_LOW => {
                self.queue_desc_low = value;
            }
            regs::QUEUE_DESC_HIGH => {
                self.queue_desc_high = value;
            }
            regs::QUEUE_DRIVER_LOW => {
                self.queue_driver_low = value;
            }
            regs::QUEUE_DRIVER_HIGH => {
                self.queue_driver_high = value;
            }
            regs::QUEUE_DEVICE_LOW => {
                self.queue_device_low = value;
            }
            regs::QUEUE_DEVICE_HIGH => {
                self.queue_device_high = value;
            }
            _ => {}
        }
    }

    /// Handle a queue notification from the guest.
    fn handle_queue_notify(&mut self) {
        // In a full implementation, this would:
        // 1. Read descriptors from the virtqueue
        // 2. Process block I/O requests
        // 3. Write completions back
        // 4. Raise an interrupt
        //
        // For now, we set the interrupt status bit to signal completion
        self.interrupt_status |= 1;
    }

    /// Reset the device.
    fn reset(&mut self) {
        self.status = 0;
        self.features_sel = 0;
        self.driver_features_low = 0;
        self.driver_features_high = 0;
        self.queue_sel = 0;
        self.interrupt_status = 0;
        self.queue_num = 0;
        self.queue_ready = 0;
    }

    /// Read a block from the device.
    pub fn read_block(&mut self, sector: u64, buf: &mut [u8]) -> Result<usize, VmmError> {
        self.file.seek(SeekFrom::Start(sector * 512))?;
        Ok(self.file.read(buf)?)
    }
}

/// virtio-blk configuration space.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct VirtioBlkConfig {
    /// Capacity in 512-byte sectors.
    pub capacity: u64,

    /// Size max.
    pub size_max: u32,

    /// Seg max.
    pub seg_max: u32,

    /// Geometry.
    pub geometry: VirtioBlkGeometry,

    /// Block size.
    pub blk_size: u32,
}

/// Block device geometry.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct VirtioBlkGeometry {
    /// Cylinders.
    pub cylinders: u16,

    /// Heads.
    pub heads: u8,

    /// Sectors.
    pub sectors: u8,
}
