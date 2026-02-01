//! virtio-vsock device emulation.
//!
//! This provides host-guest communication via the vsock protocol.
//! The guest can connect to the host using AF_VSOCK sockets.
//!
//! This implements the virtio-mmio transport for vsock devices.

use std::collections::VecDeque;
use std::io::{Read as _, Write as _};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::Arc;

use camino::Utf8Path;
use virtio_queue::{Queue, QueueOwnedT, QueueT};
use vm_memory::{Bytes, GuestAddress, GuestMemoryMmap};

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

/// Port for benchmark results (guest connects to HOST_CID:RESULTS_PORT).
pub const RESULTS_PORT: u32 = 5000;

/// Maximum queue size.
const QUEUE_SIZE: u16 = 256;

/// RX queue index.
const RX_QUEUE: usize = 0;

/// TX queue index.
const TX_QUEUE: usize = 1;

/// Event queue index.
const EVENT_QUEUE: usize = 2;

/// Number of queues.
const NUM_QUEUES: usize = 3;

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
    pub const CONFIG: u64 = 0x100;
}

/// vsock packet operations.
pub mod vsock_op {
    pub const INVALID: u16 = 0;
    pub const REQUEST: u16 = 1;
    pub const RESPONSE: u16 = 2;
    pub const RST: u16 = 3;
    pub const SHUTDOWN: u16 = 4;
    pub const RW: u16 = 5;
    pub const CREDIT_UPDATE: u16 = 6;
    pub const CREDIT_REQUEST: u16 = 7;
}

/// vsock packet header size in bytes.
pub const VSOCK_HEADER_SIZE: usize = 44;

/// vsock packet header.
///
/// This struct uses a packed representation to match the wire format.
/// Use the `from_bytes` and `to_bytes` methods for safe serialization.
#[repr(C)]
#[derive(Debug, Default, Clone, Copy)]
pub struct VsockPacketHeader {
    pub src_cid: u64,
    pub dst_cid: u64,
    pub src_port: u32,
    pub dst_port: u32,
    pub len: u32,
    pub type_: u16,
    pub op: u16,
    pub flags: u32,
    pub buf_alloc: u32,
    pub fwd_cnt: u32,
}

impl VsockPacketHeader {
    const SIZE: usize = VSOCK_HEADER_SIZE;

    /// Read a header from a byte slice (safe, handles alignment).
    #[must_use]
    pub fn from_bytes(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < Self::SIZE {
            return None;
        }
        Some(Self {
            src_cid: u64::from_le_bytes([
                bytes[0], bytes[1], bytes[2], bytes[3],
                bytes[4], bytes[5], bytes[6], bytes[7],
            ]),
            dst_cid: u64::from_le_bytes([
                bytes[8], bytes[9], bytes[10], bytes[11],
                bytes[12], bytes[13], bytes[14], bytes[15],
            ]),
            src_port: u32::from_le_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]),
            dst_port: u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]),
            len: u32::from_le_bytes([bytes[24], bytes[25], bytes[26], bytes[27]]),
            type_: u16::from_le_bytes([bytes[28], bytes[29]]),
            op: u16::from_le_bytes([bytes[30], bytes[31]]),
            flags: u32::from_le_bytes([bytes[32], bytes[33], bytes[34], bytes[35]]),
            buf_alloc: u32::from_le_bytes([bytes[36], bytes[37], bytes[38], bytes[39]]),
            fwd_cnt: u32::from_le_bytes([bytes[40], bytes[41], bytes[42], bytes[43]]),
        })
    }

    /// Write the header to a byte slice (safe, handles alignment).
    pub fn to_bytes(&self, bytes: &mut [u8]) -> bool {
        if bytes.len() < Self::SIZE {
            return false;
        }
        bytes[0..8].copy_from_slice(&self.src_cid.to_le_bytes());
        bytes[8..16].copy_from_slice(&self.dst_cid.to_le_bytes());
        bytes[16..20].copy_from_slice(&self.src_port.to_le_bytes());
        bytes[20..24].copy_from_slice(&self.dst_port.to_le_bytes());
        bytes[24..28].copy_from_slice(&self.len.to_le_bytes());
        bytes[28..30].copy_from_slice(&self.type_.to_le_bytes());
        bytes[30..32].copy_from_slice(&self.op.to_le_bytes());
        bytes[32..36].copy_from_slice(&self.flags.to_le_bytes());
        bytes[36..40].copy_from_slice(&self.buf_alloc.to_le_bytes());
        bytes[40..44].copy_from_slice(&self.fwd_cnt.to_le_bytes());
        true
    }
}

/// A vsock connection from the host (Unix socket) to the guest.
struct VsockConnection {
    /// The Unix stream for this connection.
    stream: UnixStream,
    /// Local port.
    local_port: u32,
    /// Remote port (guest port).
    remote_port: u32,
    /// Remote CID (guest CID).
    remote_cid: u64,
    /// Pending data to send to guest.
    rx_buffer: VecDeque<u8>,
    /// Buffer allocation advertised by peer.
    peer_buf_alloc: u32,
    /// Forward count from peer.
    peer_fwd_cnt: u32,
    /// Our buffer allocation.
    buf_alloc: u32,
    /// Bytes we've forwarded.
    fwd_cnt: u32,
}

/// A guest-initiated connection (guest connects to host).
///
/// Note: Credit flow control fields are reserved for future use.
#[expect(dead_code)]
struct GuestConnection {
    /// Guest's source port.
    guest_port: u32,
    /// Host's destination port (what the guest connected to).
    host_port: u32,
    /// Buffer allocation advertised by guest (reserved for credit flow).
    peer_buf_alloc: u32,
    /// Forward count from guest (reserved for credit flow).
    peer_fwd_cnt: u32,
    /// Our buffer allocation (reserved for credit flow).
    buf_alloc: u32,
    /// Bytes we've forwarded.
    fwd_cnt: u32,
}

/// A virtio-vsock device.
pub struct VirtioVsockDevice {
    /// The guest's CID.
    guest_cid: u64,

    /// Unix socket listener for host-side connections.
    listener: UnixListener,

    /// Path to the Unix socket.
    socket_path: String,

    /// Guest memory reference.
    guest_memory: Option<Arc<GuestMemoryMmap>>,

    /// The virtqueues (RX, TX, Event).
    queues: [Queue; NUM_QUEUES],

    /// Queue configuration.
    queue_configs: [QueueConfig; NUM_QUEUES],

    /// Active host-initiated connections (host -> guest).
    connections: Vec<VsockConnection>,

    /// Active guest-initiated connections (guest -> host).
    guest_connections: Vec<GuestConnection>,

    /// Buffer for results received from guest on RESULTS_PORT.
    results_buffer: Vec<u8>,

    /// Flag indicating results are complete (guest closed connection).
    results_complete: bool,

    /// Pending response packets to send to guest.
    pending_rx: VecDeque<(VsockPacketHeader, Vec<u8>)>,

    /// Device status register.
    status: u32,

    /// Selected feature page.
    features_sel: u32,

    /// Selected queue.
    queue_sel: u32,

    /// Interrupt status.
    interrupt_status: u32,
}

/// Per-queue configuration.
#[derive(Default)]
struct QueueConfig {
    num: u16,
    ready: bool,
    desc: u64,
    avail: u64,
    used: u64,
}

impl VirtioVsockDevice {
    /// Create a new virtio-vsock device.
    pub fn new(guest_cid: u64, socket_path: &Utf8Path) -> Result<Self, VmmError> {
        // Remove existing socket if present
        let _ = std::fs::remove_file(socket_path);

        let listener = UnixListener::bind(socket_path)?;
        listener.set_nonblocking(true)?;

        Ok(Self {
            guest_cid,
            listener,
            socket_path: socket_path.to_string(),
            guest_memory: None,
            queues: [
                Queue::new(QUEUE_SIZE).map_err(|e| VmmError::Device(e.to_string()))?,
                Queue::new(QUEUE_SIZE).map_err(|e| VmmError::Device(e.to_string()))?,
                Queue::new(QUEUE_SIZE).map_err(|e| VmmError::Device(e.to_string()))?,
            ],
            queue_configs: Default::default(),
            connections: Vec::new(),
            guest_connections: Vec::new(),
            results_buffer: Vec::new(),
            results_complete: false,
            pending_rx: VecDeque::new(),
            status: 0,
            features_sel: 0,
            queue_sel: 0,
            interrupt_status: 0,
        })
    }

    /// Set the guest memory reference.
    pub fn set_guest_memory(&mut self, mem: Arc<GuestMemoryMmap>) {
        self.guest_memory = Some(mem);
    }

    /// Get the guest's CID.
    #[must_use]
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

    /// Check if an interrupt is pending.
    #[must_use]
    pub fn has_pending_interrupt(&self) -> bool {
        self.interrupt_status != 0
    }

    /// Check if results have been received from the guest.
    #[must_use]
    pub fn has_results(&self) -> bool {
        self.results_complete || !self.results_buffer.is_empty()
    }

    /// Check if results collection is complete (guest closed connection).
    #[must_use]
    pub fn results_complete(&self) -> bool {
        self.results_complete
    }

    /// Take the collected results, leaving an empty buffer.
    pub fn take_results(&mut self) -> Vec<u8> {
        std::mem::take(&mut self.results_buffer)
    }

    /// Get the collected results as a string.
    #[must_use]
    pub fn results_as_string(&self) -> String {
        String::from_utf8_lossy(&self.results_buffer).to_string()
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
            regs::QUEUE_NUM_MAX => u32::from(QUEUE_SIZE),
            regs::QUEUE_READY => {
                let idx = self.queue_sel as usize;
                if idx < NUM_QUEUES {
                    u32::from(self.queue_configs[idx].ready)
                } else {
                    0
                }
            }
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
        let idx = self.queue_sel as usize;

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
                if idx < NUM_QUEUES {
                    self.queue_configs[idx].num = value as u16;
                }
            }
            regs::QUEUE_READY => {
                if idx < NUM_QUEUES && value == 1 {
                    self.activate_queue(idx);
                    self.queue_configs[idx].ready = true;
                }
            }
            regs::QUEUE_NOTIFY => {
                self.handle_queue_notify(value as usize);
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
                if idx < NUM_QUEUES {
                    self.queue_configs[idx].desc =
                        (self.queue_configs[idx].desc & 0xFFFF_FFFF_0000_0000) | u64::from(value);
                }
            }
            regs::QUEUE_DESC_HIGH => {
                if idx < NUM_QUEUES {
                    self.queue_configs[idx].desc =
                        (self.queue_configs[idx].desc & 0x0000_0000_FFFF_FFFF) | (u64::from(value) << 32);
                }
            }
            regs::QUEUE_DRIVER_LOW => {
                if idx < NUM_QUEUES {
                    self.queue_configs[idx].avail =
                        (self.queue_configs[idx].avail & 0xFFFF_FFFF_0000_0000) | u64::from(value);
                }
            }
            regs::QUEUE_DRIVER_HIGH => {
                if idx < NUM_QUEUES {
                    self.queue_configs[idx].avail =
                        (self.queue_configs[idx].avail & 0x0000_0000_FFFF_FFFF) | (u64::from(value) << 32);
                }
            }
            regs::QUEUE_DEVICE_LOW => {
                if idx < NUM_QUEUES {
                    self.queue_configs[idx].used =
                        (self.queue_configs[idx].used & 0xFFFF_FFFF_0000_0000) | u64::from(value);
                }
            }
            regs::QUEUE_DEVICE_HIGH => {
                if idx < NUM_QUEUES {
                    self.queue_configs[idx].used =
                        (self.queue_configs[idx].used & 0x0000_0000_FFFF_FFFF) | (u64::from(value) << 32);
                }
            }
            _ => {}
        }
    }

    /// Activate a queue.
    fn activate_queue(&mut self, idx: usize) {
        if idx >= NUM_QUEUES {
            return;
        }

        let config = &self.queue_configs[idx];
        self.queues[idx].set_size(config.num);
        self.queues[idx].set_desc_table_address(Some(GuestAddress(config.desc)), None);
        self.queues[idx].set_avail_ring_address(Some(GuestAddress(config.avail)), None);
        self.queues[idx].set_used_ring_address(Some(GuestAddress(config.used)), None);
        self.queues[idx].set_ready(true);
    }

    /// Handle a queue notification.
    fn handle_queue_notify(&mut self, queue_idx: usize) {
        match queue_idx {
            RX_QUEUE => self.process_rx_queue(),
            TX_QUEUE => self.process_tx_queue(),
            EVENT_QUEUE => {
                // Event queue is for device events, not much to do here
            }
            _ => {}
        }
    }

    /// Process the RX queue (host -> guest).
    fn process_rx_queue(&mut self) {
        let Some(mem) = self.guest_memory.as_ref() else {
            return;
        };
        let mem = mem.as_ref();

        // First, check for new incoming connections
        self.accept_connections();

        // Read data from all connections into pending_rx
        self.read_from_connections();

        // Write pending packets to guest
        while let Some((header, data)) = self.pending_rx.pop_front() {
            if let Some(mut chain) = self.queues[RX_QUEUE].pop_descriptor_chain(mem) {
                let head_index = chain.head_index();
                let mut total_written = 0u32;

                // Write header to first descriptor
                if let Some(desc) = chain.next() {
                    if desc.is_write_only() {
                        let mut header_bytes = [0u8; VSOCK_HEADER_SIZE];
                        header.to_bytes(&mut header_bytes);
                        if mem.write_slice(&header_bytes, desc.addr()).is_ok() {
                            total_written += VSOCK_HEADER_SIZE as u32;
                        }
                    }
                }

                // Write data to subsequent descriptors
                let mut data_offset = 0;
                for desc in chain {
                    if !desc.is_write_only() || data_offset >= data.len() {
                        continue;
                    }
                    let to_write = (data.len() - data_offset).min(desc.len() as usize);
                    if mem.write_slice(&data[data_offset..data_offset + to_write], desc.addr()).is_ok() {
                        total_written += to_write as u32;
                        data_offset += to_write;
                    }
                }

                self.queues[RX_QUEUE]
                    .add_used(mem, head_index, total_written)
                    .ok();
                self.interrupt_status |= 1;
            } else {
                // No available descriptors, put it back
                self.pending_rx.push_front((header, data));
                break;
            }
        }
    }

    /// Process the TX queue (guest -> host).
    fn process_tx_queue(&mut self) {
        let Some(mem) = self.guest_memory.as_ref() else {
            return;
        };
        let mem = mem.as_ref();

        while let Some(mut chain) = self.queues[TX_QUEUE].pop_descriptor_chain(mem) {
            let head_index = chain.head_index();

            // Read header from first descriptor
            let header = if let Some(desc) = chain.next() {
                let mut header_bytes = [0u8; VSOCK_HEADER_SIZE];
                if mem.read_slice(&mut header_bytes, desc.addr()).is_err() {
                    self.queues[TX_QUEUE].add_used(mem, head_index, 0).ok();
                    continue;
                }
                match VsockPacketHeader::from_bytes(&header_bytes) {
                    Some(h) => h,
                    None => {
                        self.queues[TX_QUEUE].add_used(mem, head_index, 0).ok();
                        continue;
                    }
                }
            } else {
                self.queues[TX_QUEUE].add_used(mem, head_index, 0).ok();
                continue;
            };

            // Read data from subsequent descriptors
            let mut data = Vec::new();
            for desc in chain {
                if desc.is_write_only() {
                    continue;
                }
                let mut buf = vec![0u8; desc.len() as usize];
                if mem.read_slice(&mut buf, desc.addr()).is_ok() {
                    data.extend_from_slice(&buf);
                }
            }

            // Process the packet
            self.handle_tx_packet(header, data);

            self.queues[TX_QUEUE].add_used(mem, head_index, 0).ok();
        }

        self.interrupt_status |= 1;
    }

    /// Handle a packet from the guest.
    fn handle_tx_packet(&mut self, header: VsockPacketHeader, data: Vec<u8>) {
        match header.op {
            vsock_op::REQUEST => {
                // Connection request from guest
                self.handle_connect_request(header);
            }
            vsock_op::RESPONSE => {
                // Response to our connection request (we don't initiate connections)
            }
            vsock_op::RST => {
                // Connection reset
                self.handle_reset(header);
            }
            vsock_op::SHUTDOWN => {
                // Shutdown connection
                self.handle_shutdown(header);
            }
            vsock_op::RW => {
                // Data packet
                self.handle_data(header, data);
            }
            vsock_op::CREDIT_UPDATE => {
                // Credit update from guest
                self.handle_credit_update(header);
            }
            vsock_op::CREDIT_REQUEST => {
                // Guest is asking for our credit
                self.send_credit_update(header.src_port, header.dst_port);
            }
            _ => {}
        }
    }

    /// Handle a connection request from guest.
    fn handle_connect_request(&mut self, header: VsockPacketHeader) {
        // Track this guest-initiated connection
        let guest_conn = GuestConnection {
            guest_port: header.src_port,
            host_port: header.dst_port,
            peer_buf_alloc: header.buf_alloc,
            peer_fwd_cnt: header.fwd_cnt,
            buf_alloc: 64 * 1024,
            fwd_cnt: 0,
        };
        self.guest_connections.push(guest_conn);

        // Accept the connection
        let response = VsockPacketHeader {
            src_cid: HOST_CID,
            dst_cid: self.guest_cid,
            src_port: header.dst_port,
            dst_port: header.src_port,
            len: 0,
            type_: 1, // STREAM
            op: vsock_op::RESPONSE,
            flags: 0,
            buf_alloc: 64 * 1024, // 64 KiB buffer
            fwd_cnt: 0,
        };
        self.pending_rx.push_back((response, Vec::new()));
    }

    /// Handle connection reset.
    fn handle_reset(&mut self, header: VsockPacketHeader) {
        // Check if this is a guest-initiated connection being reset
        if header.dst_port == RESULTS_PORT {
            self.guest_connections.retain(|c| {
                !(c.guest_port == header.src_port && c.host_port == header.dst_port)
            });
            // Treat reset as results complete (even if unexpected)
            self.results_complete = true;
            return;
        }

        // Remove the host-initiated connection
        self.connections.retain(|c| {
            !(c.remote_cid == header.src_cid && c.remote_port == header.src_port)
        });
    }

    /// Handle connection shutdown.
    fn handle_shutdown(&mut self, header: VsockPacketHeader) {
        // Check if this is the results connection being closed
        if header.dst_port == RESULTS_PORT {
            self.guest_connections.retain(|c| {
                !(c.guest_port == header.src_port && c.host_port == header.dst_port)
            });
            // Mark results as complete
            self.results_complete = true;
            return;
        }

        // Close the host-initiated connection gracefully
        self.connections.retain(|c| {
            !(c.remote_cid == header.src_cid && c.remote_port == header.src_port)
        });
    }

    /// Handle data from guest.
    fn handle_data(&mut self, header: VsockPacketHeader, data: Vec<u8>) {
        // Check if this is data for the results port (guest-initiated connection)
        if header.dst_port == RESULTS_PORT {
            // Find the matching guest connection and update fwd_cnt
            for conn in &mut self.guest_connections {
                if conn.guest_port == header.src_port && conn.host_port == header.dst_port {
                    conn.fwd_cnt += data.len() as u32;
                    break;
                }
            }
            // Collect the results
            self.results_buffer.extend_from_slice(&data);
            return;
        }

        // Otherwise, find the host-initiated connection and write data to the Unix socket
        for conn in &mut self.connections {
            if conn.remote_cid == header.src_cid && conn.remote_port == header.src_port {
                let _ = conn.stream.write_all(&data);
                conn.fwd_cnt += data.len() as u32;
                break;
            }
        }
    }

    /// Handle credit update from guest.
    fn handle_credit_update(&mut self, header: VsockPacketHeader) {
        for conn in &mut self.connections {
            if conn.remote_cid == header.src_cid && conn.remote_port == header.src_port {
                conn.peer_buf_alloc = header.buf_alloc;
                conn.peer_fwd_cnt = header.fwd_cnt;
                break;
            }
        }
    }

    /// Send credit update to guest.
    fn send_credit_update(&mut self, src_port: u32, dst_port: u32) {
        if let Some(conn) = self.connections.iter().find(|c| c.local_port == dst_port) {
            let header = VsockPacketHeader {
                src_cid: HOST_CID,
                dst_cid: self.guest_cid,
                src_port: dst_port,
                dst_port: src_port,
                len: 0,
                type_: 1,
                op: vsock_op::CREDIT_UPDATE,
                flags: 0,
                buf_alloc: conn.buf_alloc,
                fwd_cnt: conn.fwd_cnt,
            };
            self.pending_rx.push_back((header, Vec::new()));
        }
    }

    /// Accept new connections on the Unix socket.
    fn accept_connections(&mut self) {
        while let Ok((stream, _)) = self.listener.accept() {
            stream.set_nonblocking(true).ok();
            let local_port = self.connections.len() as u32 + 1024;
            self.connections.push(VsockConnection {
                stream,
                local_port,
                remote_port: 0, // Will be set when guest connects
                remote_cid: self.guest_cid,
                rx_buffer: VecDeque::new(),
                peer_buf_alloc: 0,
                peer_fwd_cnt: 0,
                buf_alloc: 64 * 1024,
                fwd_cnt: 0,
            });
        }
    }

    /// Read data from all connections.
    fn read_from_connections(&mut self) {
        for conn in &mut self.connections {
            let mut buf = [0u8; 4096];
            while let Ok(n) = conn.stream.read(&mut buf) {
                if n == 0 {
                    break;
                }
                conn.rx_buffer.extend(&buf[..n]);
            }

            // Send buffered data to guest
            while !conn.rx_buffer.is_empty() {
                let data: Vec<u8> = conn.rx_buffer.drain(..).collect();
                let header = VsockPacketHeader {
                    src_cid: HOST_CID,
                    dst_cid: conn.remote_cid,
                    src_port: conn.local_port,
                    dst_port: conn.remote_port,
                    len: data.len() as u32,
                    type_: 1,
                    op: vsock_op::RW,
                    flags: 0,
                    buf_alloc: conn.buf_alloc,
                    fwd_cnt: conn.fwd_cnt,
                };
                self.pending_rx.push_back((header, data));
            }
        }
    }

    /// Poll for activity (accept connections, read data).
    ///
    /// This should be called periodically to check for incoming connections
    /// and data on existing connections.
    pub fn poll(&mut self) {
        // Accept new connections
        self.accept_connections();

        // Read data from connections
        self.read_from_connections();

        // If we have pending data and guest memory is set, try to process RX queue
        if !self.pending_rx.is_empty() && self.guest_memory.is_some() {
            self.process_rx_queue();
        }
    }

    /// Reset the device.
    fn reset(&mut self) {
        self.status = 0;
        self.features_sel = 0;
        self.interrupt_status = 0;
        self.queue_sel = 0;
        self.connections.clear();
        self.guest_connections.clear();
        self.results_buffer.clear();
        self.results_complete = false;
        self.pending_rx.clear();
        for i in 0..NUM_QUEUES {
            self.queue_configs[i] = QueueConfig::default();
            self.queues[i].set_ready(false);
        }
    }
}

impl Drop for VirtioVsockDevice {
    fn drop(&mut self) {
        // Clean up the socket file
        let _ = std::fs::remove_file(&self.socket_path);
    }
}
