//! virtio-blk device emulation.
//!
//! This provides a block device for mounting the rootfs in the guest.
//! The device is backed by a file on the host (the squashfs image).
//!
//! This implements the virtio-mmio transport as defined in the virtio spec.
//! See: <https://docs.oasis-open.org/virtio/virtio/v1.1/virtio-v1.1.html>

use std::fs::File;
use std::io::{Read as _, Seek, SeekFrom, Write as _};
use std::sync::Arc;

use camino::Utf8Path;
use virtio_queue::{Queue, QueueOwnedT, QueueT};
use vm_memory::{Bytes, GuestAddress, GuestMemoryMmap};

use crate::error::VmmError;

/// virtio device type for block devices.
pub const VIRTIO_BLK_DEVICE_TYPE: u32 = 2;

/// virtio MMIO magic value ("virt").
const VIRTIO_MMIO_MAGIC: u32 = 0x7472_6976;

/// virtio MMIO version (legacy = 1, modern = 2).
const VIRTIO_MMIO_VERSION: u32 = 2;

/// virtio vendor ID (we use a placeholder).
const VIRTIO_VENDOR_ID: u32 = 0x554d_4551; // "QEMU" backwards

/// Maximum queue size.
const QUEUE_SIZE: u16 = 256;

/// Sector size in bytes.
const SECTOR_SIZE: u64 = 512;

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

/// virtio-blk request types.
mod request_type {
    pub const VIRTIO_BLK_T_IN: u32 = 0;      // Read
    pub const VIRTIO_BLK_T_OUT: u32 = 1;     // Write
    pub const VIRTIO_BLK_T_FLUSH: u32 = 4;   // Flush
    pub const VIRTIO_BLK_T_GET_ID: u32 = 8;  // Get device ID
}

/// virtio-blk status codes.
mod status {
    pub const VIRTIO_BLK_S_OK: u8 = 0;
    pub const VIRTIO_BLK_S_IOERR: u8 = 1;
    pub const VIRTIO_BLK_S_UNSUPP: u8 = 2;
}

/// virtio-blk request header (first descriptor in chain).
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
struct VirtioBlkReqHeader {
    /// Request type (IN, OUT, FLUSH, etc.)
    request_type: u32,
    /// Reserved field.
    reserved: u32,
    /// Sector number for read/write operations.
    sector: u64,
}

/// A virtio block device.
pub struct VirtioBlkDevice {
    /// The backing file for the block device.
    file: File,

    /// The size of the backing file in bytes.
    size: u64,

    /// Whether the device is read-only.
    read_only: bool,

    /// Guest memory reference for queue processing.
    guest_memory: Option<Arc<GuestMemoryMmap>>,

    /// The virtqueue.
    queue: Queue,

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

    /// Queue size set by driver.
    queue_num: u16,

    /// Queue ready flag.
    queue_ready: bool,

    /// Queue descriptor table address.
    queue_desc: u64,

    /// Queue available ring address.
    queue_avail: u64,

    /// Queue used ring address.
    queue_used: u64,
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
            guest_memory: None,
            queue: Queue::new(QUEUE_SIZE).map_err(|e| VmmError::Device(e.to_string()))?,
            status: 0,
            features_sel: 0,
            driver_features_low: 0,
            driver_features_high: 0,
            queue_sel: 0,
            interrupt_status: 0,
            queue_num: 0,
            queue_ready: false,
            queue_desc: 0,
            queue_avail: 0,
            queue_used: 0,
        })
    }

    /// Set the guest memory reference for queue processing.
    pub fn set_guest_memory(&mut self, mem: Arc<GuestMemoryMmap>) {
        self.guest_memory = Some(mem);
    }

    /// Get the size of the block device in 512-byte sectors.
    pub fn capacity_sectors(&self) -> u64 {
        self.size / SECTOR_SIZE
    }

    /// Check if the device is read-only.
    #[must_use]
    pub fn is_read_only(&self) -> bool {
        self.read_only
    }

    /// Check if an interrupt is pending.
    #[must_use]
    pub fn has_pending_interrupt(&self) -> bool {
        self.interrupt_status != 0
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
            regs::QUEUE_NUM_MAX => u32::from(QUEUE_SIZE),
            regs::QUEUE_READY => u32::from(self.queue_ready),
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
            0x114 => SECTOR_SIZE as u32,
            _ => 0,
        };

        // Write the value to the data buffer (little-endian)
        let len = data.len().min(4);
        data[..len].copy_from_slice(&value.to_le_bytes()[..len]);
    }

    /// Write to MMIO registers.
    /// Returns true if an interrupt should be injected to the guest.
    pub fn write(&mut self, offset: u64, data: &[u8]) -> bool {
        if data.len() < 4 {
            return false;
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
                self.queue_num = value as u16;
            }
            regs::QUEUE_READY => {
                if value == 1 {
                    self.activate_queue();
                    eprintln!("[VIRTIO-BLK] Queue activated: desc={:#x} avail={:#x} used={:#x} size={}",
                        self.queue_desc, self.queue_avail, self.queue_used, self.queue_num);
                }
                self.queue_ready = value == 1;
            }
            regs::QUEUE_NOTIFY => {
                // Guest is notifying us that there's work to do
                let processed = self.process_queue();
                if processed {
                    return true; // Signal that interrupt should be injected
                }
            }
            regs::INTERRUPT_ACK => {
                self.interrupt_status &= !value;
            }
            regs::STATUS => {
                self.status = value;
                if value == 0 {
                    self.reset();
                }
            }
            regs::QUEUE_DESC_LOW => {
                self.queue_desc = (self.queue_desc & 0xFFFF_FFFF_0000_0000) | u64::from(value);
            }
            regs::QUEUE_DESC_HIGH => {
                self.queue_desc = (self.queue_desc & 0x0000_0000_FFFF_FFFF) | (u64::from(value) << 32);
            }
            regs::QUEUE_DRIVER_LOW => {
                self.queue_avail = (self.queue_avail & 0xFFFF_FFFF_0000_0000) | u64::from(value);
            }
            regs::QUEUE_DRIVER_HIGH => {
                self.queue_avail = (self.queue_avail & 0x0000_0000_FFFF_FFFF) | (u64::from(value) << 32);
            }
            regs::QUEUE_DEVICE_LOW => {
                self.queue_used = (self.queue_used & 0xFFFF_FFFF_0000_0000) | u64::from(value);
            }
            regs::QUEUE_DEVICE_HIGH => {
                self.queue_used = (self.queue_used & 0x0000_0000_FFFF_FFFF) | (u64::from(value) << 32);
            }
            _ => {}
        }

        false
    }

    /// Activate the queue with the configured addresses.
    fn activate_queue(&mut self) {
        self.queue.set_size(self.queue_num);
        self.queue.set_desc_table_address(
            Some(self.queue_desc as u32),
            Some((self.queue_desc >> 32) as u32),
        );
        self.queue.set_avail_ring_address(
            Some(self.queue_avail as u32),
            Some((self.queue_avail >> 32) as u32),
        );
        self.queue.set_used_ring_address(
            Some(self.queue_used as u32),
            Some((self.queue_used >> 32) as u32),
        );
        self.queue.set_ready(true);
    }

    /// Process all available requests in the queue.
    /// Returns true if any requests were processed (interrupt should be injected).
    pub fn process_queue(&mut self) -> bool {
        // Clone the Arc to avoid borrowing self while processing
        let Some(mem_arc) = self.guest_memory.clone() else {
            return false;
        };

        let mem = mem_arc.as_ref();
        let mut processed_any = false;

        // Process all available descriptor chains
        while let Some(chain) = self.queue.pop_descriptor_chain(mem) {
            processed_any = true;
            let head_index = chain.head_index();
            let mut bytes_written = 0u32;
            let mut status = status::VIRTIO_BLK_S_OK;

            // Collect all descriptors to find header, data, and status
            let descriptors: Vec<_> = chain.collect();
            if descriptors.is_empty() {
                continue;
            }

            // First descriptor is always the header
            let header_desc = &descriptors[0];

            // Read the request header
            let header = match self.read_header_from_desc(mem, header_desc.addr()) {
                Ok(h) => h,
                Err(_) => {
                    status = status::VIRTIO_BLK_S_IOERR;
                    // Try to find status descriptor and complete
                    if let Some(status_desc) = descriptors.last() {
                        self.complete_request(mem, head_index, 0, status, status_desc.addr());
                    }
                    continue;
                }
            };

            // Last descriptor is the status descriptor
            let status_desc = descriptors.last().expect("already checked non-empty");
            let status_addr = status_desc.addr();

            // Data descriptors are everything between header and status
            let data_descriptors = if descriptors.len() > 2 {
                &descriptors[1..descriptors.len() - 1]
            } else {
                &[]
            };

            // Process based on request type
            match header.request_type {
                request_type::VIRTIO_BLK_T_IN => {
                    // Read operation: read from disk to guest memory
                    match self.handle_read_descs(mem, header.sector, data_descriptors) {
                        Ok(len) => bytes_written = len,
                        Err(e) => {
                            eprintln!("[VIRTIO-BLK] Read error: {e}");
                            status = status::VIRTIO_BLK_S_IOERR;
                        }
                    }
                }
                request_type::VIRTIO_BLK_T_OUT => {
                    // Write operation: write from guest memory to disk
                    if self.read_only {
                        status = status::VIRTIO_BLK_S_IOERR;
                    } else if let Err(e) = self.handle_write_descs(mem, header.sector, data_descriptors) {
                        eprintln!("[VIRTIO-BLK] Write error: {e}");
                        status = status::VIRTIO_BLK_S_IOERR;
                    }
                }
                request_type::VIRTIO_BLK_T_FLUSH => {
                    // Flush operation
                    if let Err(e) = self.file.sync_all() {
                        eprintln!("[VIRTIO-BLK] Flush error: {e}");
                        status = status::VIRTIO_BLK_S_IOERR;
                    }
                }
                request_type::VIRTIO_BLK_T_GET_ID => {
                    // Get device ID - just return zeros
                    bytes_written = 0;
                }
                _ => {
                    eprintln!("[VIRTIO-BLK] Unsupported request type: {}", header.request_type);
                    status = status::VIRTIO_BLK_S_UNSUPP;
                }
            }

            // Write status to the last descriptor and complete the request
            self.complete_request(mem, head_index, bytes_written, status, status_addr);
        }

        if processed_any {
            // Signal interrupt
            self.interrupt_status |= 1;
        }

        processed_any
    }

    /// Read header from a specific guest address.
    fn read_header_from_desc(
        &self,
        mem: &GuestMemoryMmap,
        addr: GuestAddress,
    ) -> Result<VirtioBlkReqHeader, VmmError> {
        let mut header = VirtioBlkReqHeader::default();
        let header_bytes = unsafe {
            std::slice::from_raw_parts_mut(
                (&mut header as *mut VirtioBlkReqHeader).cast::<u8>(),
                std::mem::size_of::<VirtioBlkReqHeader>(),
            )
        };

        mem.read_slice(header_bytes, addr)
            .map_err(|e| VmmError::Device(format!("Failed to read header: {e}")))?;

        Ok(header)
    }

    /// Handle a read request using descriptor slice.
    fn handle_read_descs(
        &mut self,
        mem: &GuestMemoryMmap,
        sector: u64,
        descriptors: &[virtio_queue::Descriptor],
    ) -> Result<u32, VmmError> {
        let mut total_read = 0u32;
        let mut current_offset = sector * SECTOR_SIZE;

        for desc in descriptors {
            // For reads, we write to write-only descriptors
            if !desc.is_write_only() {
                continue;
            }

            let len = desc.len() as usize;
            let mut buffer = vec![0u8; len];

            // Read from the backing file
            self.file
                .seek(SeekFrom::Start(current_offset))
                .map_err(|e| VmmError::Device(format!("Seek failed: {e}")))?;

            let bytes_read = self.file
                .read(&mut buffer)
                .map_err(|e| VmmError::Device(format!("Read failed: {e}")))?;

            // Write to guest memory
            mem.write_slice(&buffer[..bytes_read], desc.addr())
                .map_err(|e| VmmError::Device(format!("Write to guest failed: {e}")))?;

            total_read += bytes_read as u32;
            current_offset += bytes_read as u64;
        }

        Ok(total_read)
    }

    /// Handle a write request using descriptor slice.
    fn handle_write_descs(
        &mut self,
        mem: &GuestMemoryMmap,
        sector: u64,
        descriptors: &[virtio_queue::Descriptor],
    ) -> Result<(), VmmError> {
        let mut current_offset = sector * SECTOR_SIZE;

        for desc in descriptors {
            // For writes, we read from read-only descriptors
            if desc.is_write_only() {
                continue;
            }

            let len = desc.len() as usize;
            let mut buffer = vec![0u8; len];

            // Read from guest memory
            mem.read_slice(&mut buffer, desc.addr())
                .map_err(|e| VmmError::Device(format!("Read from guest failed: {e}")))?;

            // Write to the backing file
            self.file
                .seek(SeekFrom::Start(current_offset))
                .map_err(|e| VmmError::Device(format!("Seek failed: {e}")))?;

            self.file
                .write_all(&buffer)
                .map_err(|e| VmmError::Device(format!("Write failed: {e}")))?;

            current_offset += len as u64;
        }

        Ok(())
    }

    /// Complete a request by writing status and adding to used ring.
    fn complete_request(
        &mut self,
        mem: &GuestMemoryMmap,
        head_index: u16,
        bytes_written: u32,
        status: u8,
        status_addr: GuestAddress,
    ) {
        // Write the status byte to the status descriptor in guest memory
        if let Err(e) = mem.write_obj(status, status_addr) {
            eprintln!("[VIRTIO-BLK] Failed to write status byte: {e}");
        }

        // Add to used ring - the total length includes data + status byte
        let len = bytes_written + 1; // +1 for status byte
        self.queue
            .add_used(mem, head_index, len)
            .unwrap_or_else(|e| {
                eprintln!("[VIRTIO-BLK] Failed to add used entry: {e}");
            });
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
        self.queue_ready = false;
        self.queue_desc = 0;
        self.queue_avail = 0;
        self.queue_used = 0;
        self.queue.set_ready(false);
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
