//! virtio-vsock device emulation.
//!
//! This provides host-guest communication via the vsock protocol.
//! The guest can connect to the host using AF_VSOCK sockets.
//!
//! This implements the virtio-mmio transport for vsock devices.

use std::os::unix::net::UnixListener;

use camino::Utf8Path;

use crate::error::VmmError;

/// virtio device type for vsock.
pub const VIRTIO_VSOCK_DEVICE_TYPE: u32 = 19;

/// virtio MMIO magic value ("virt").
const VIRTIO_MMIO_MAGIC: u32 = 0x7472_6976;

/// virtio MMIO version.
const VIRTIO_MMIO_VERSION: u32 = 2;

/// virtio vendor ID.
const VIRTIO_VENDOR_ID: u32 = 0x554d_4551;

/// The host's CID (Context ID).
pub const HOST_CID: u64 = 2;

/// MMIO register offsets (same as virtio-blk).
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
    pub const CONFIG: u64 = 0x100;
}

/// A virtio-vsock device.
pub struct VirtioVsockDevice {
    /// The guest's CID.
    guest_cid: u64,

    /// Unix socket listener for host-side connections.
    listener: UnixListener,

    /// Path to the Unix socket.
    socket_path: String,

    /// Device status register.
    status: u32,

    /// Selected feature page.
    features_sel: u32,

    /// Interrupt status.
    interrupt_status: u32,

    /// Queue configuration.
    queue_sel: u32,
    queue_num: u32,
    queue_ready: u32,
}

impl VirtioVsockDevice {
    /// Create a new virtio-vsock device.
    ///
    /// # Arguments
    ///
    /// * `guest_cid` - The guest's Context ID
    /// * `socket_path` - Path to the Unix socket for host-side connections
    pub fn new(guest_cid: u64, socket_path: &Utf8Path) -> Result<Self, VmmError> {
        // Remove existing socket if present
        let _ = std::fs::remove_file(socket_path);

        let listener = UnixListener::bind(socket_path)?;
        listener.set_nonblocking(true)?;

        Ok(Self {
            guest_cid,
            listener,
            socket_path: socket_path.to_string(),
            status: 0,
            features_sel: 0,
            interrupt_status: 0,
            queue_sel: 0,
            queue_num: 0,
            queue_ready: 0,
        })
    }

    /// Get the guest's CID.
    pub fn guest_cid(&self) -> u64 {
        self.guest_cid
    }

    /// Get the Unix socket listener.
    #[must_use]
    pub fn listener(&self) -> &UnixListener {
        &self.listener
    }

    /// Get the socket path.
    #[must_use]
    pub fn socket_path(&self) -> &str {
        &self.socket_path
    }

    /// Get device features.
    fn device_features(&self) -> u64 {
        // VIRTIO_F_VERSION_1
        1u64 << 32
    }

    /// Read from MMIO registers.
    pub fn read(&mut self, offset: u64, data: &mut [u8]) {
        let value: u32 = match offset {
            regs::MAGIC_VALUE => VIRTIO_MMIO_MAGIC,
            regs::VERSION => VIRTIO_MMIO_VERSION,
            regs::DEVICE_ID => VIRTIO_VSOCK_DEVICE_TYPE,
            regs::VENDOR_ID => VIRTIO_VENDOR_ID,
            regs::DEVICE_FEATURES => {
                let features = self.device_features();
                if self.features_sel == 0 {
                    features as u32
                } else {
                    (features >> 32) as u32
                }
            }
            regs::QUEUE_NUM_MAX => 256,
            regs::QUEUE_READY => self.queue_ready,
            regs::INTERRUPT_STATUS => self.interrupt_status,
            regs::STATUS => self.status,
            // Config space: guest_cid (8 bytes at offset 0x100)
            regs::CONFIG => self.guest_cid as u32,
            0x104 => (self.guest_cid >> 32) as u32,
            _ => 0,
        };

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
            regs::DRIVER_FEATURES | regs::DRIVER_FEATURES_SEL => {
                // Accept driver features
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
                self.handle_queue_notify();
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
            _ => {}
        }
    }

    /// Handle a queue notification.
    fn handle_queue_notify(&mut self) {
        // In a full implementation, this would process vsock packets
        self.interrupt_status |= 1;
    }

    /// Reset the device.
    fn reset(&mut self) {
        self.status = 0;
        self.features_sel = 0;
        self.interrupt_status = 0;
        self.queue_sel = 0;
        self.queue_num = 0;
        self.queue_ready = 0;
    }
}

impl Drop for VirtioVsockDevice {
    fn drop(&mut self) {
        // Clean up the socket file
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

/// virtio-vsock configuration space.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct VirtioVsockConfig {
    /// Guest CID.
    pub guest_cid: u64,
}

/// vsock packet header.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct VsockPacketHeader {
    /// Source CID.
    pub src_cid: u64,

    /// Destination CID.
    pub dst_cid: u64,

    /// Source port.
    pub src_port: u32,

    /// Destination port.
    pub dst_port: u32,

    /// Packet length.
    pub len: u32,

    /// Packet type.
    pub type_: u16,

    /// Operation.
    pub op: u16,

    /// Flags.
    pub flags: u32,

    /// Buffer allocation.
    pub buf_alloc: u32,

    /// Forward count.
    pub fwd_cnt: u32,
}

/// vsock packet operations.
pub mod vsock_op {
    /// Invalid operation.
    pub const INVALID: u16 = 0;

    /// Request connection.
    pub const REQUEST: u16 = 1;

    /// Response to connection request.
    pub const RESPONSE: u16 = 2;

    /// Reset connection.
    pub const RST: u16 = 3;

    /// Shutdown connection.
    pub const SHUTDOWN: u16 = 4;

    /// Data packet.
    pub const RW: u16 = 5;

    /// Credit update.
    pub const CREDIT_UPDATE: u16 = 6;

    /// Credit request.
    pub const CREDIT_REQUEST: u16 = 7;
}
