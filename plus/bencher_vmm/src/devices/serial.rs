//! Serial console (UART 16550A) emulation.
//!
//! This provides a simple serial port for kernel console output.
//! The guest writes to the serial port, and we capture the output.

use std::collections::VecDeque;

use crate::error::VmmError;

/// Line Status Register bits.
const LSR_DATA_READY: u8 = 0x01;
const LSR_THR_EMPTY: u8 = 0x20;
const LSR_BOTH_EMPTY: u8 = 0x40;

/// UART register offsets.
const THR: u16 = 0; // Transmit Holding Register (write)
const RBR: u16 = 0; // Receive Buffer Register (read)
const IER: u16 = 1; // Interrupt Enable Register
const IIR: u16 = 2; // Interrupt Identification Register (read)
const FCR: u16 = 2; // FIFO Control Register (write)
const LCR: u16 = 3; // Line Control Register
const MCR: u16 = 4; // Modem Control Register
const LSR: u16 = 5; // Line Status Register
const MSR: u16 = 6; // Modem Status Register
const SCR: u16 = 7; // Scratch Register

/// A serial console device.
pub struct SerialDevice {
    /// Output buffer for data written by the guest.
    output: VecDeque<u8>,

    /// Input buffer for data to send to the guest.
    input: VecDeque<u8>,

    /// Interrupt Enable Register.
    ier: u8,

    /// Line Control Register.
    lcr: u8,

    /// Modem Control Register.
    mcr: u8,

    /// Scratch Register.
    scr: u8,

    /// FIFO Control Register.
    fcr: u8,

    /// Divisor Latch (when DLAB is set).
    divisor: u16,
}

impl SerialDevice {
    /// Create a new serial device.
    pub fn new() -> Result<Self, VmmError> {
        Ok(Self {
            output: VecDeque::with_capacity(4096),
            input: VecDeque::new(),
            ier: 0,
            lcr: 0,
            mcr: 0,
            scr: 0,
            fcr: 0,
            divisor: 1,
        })
    }

    /// Check if DLAB (Divisor Latch Access Bit) is set.
    fn dlab(&self) -> bool {
        self.lcr & 0x80 != 0
    }

    /// Read from a register.
    pub fn read(&mut self, offset: u16, data: &mut [u8]) {
        if data.is_empty() {
            return;
        }

        data[0] = match offset {
            RBR if !self.dlab() => {
                // Read from receive buffer
                self.input.pop_front().unwrap_or(0)
            }
            RBR if self.dlab() => {
                // Divisor latch low
                (self.divisor & 0xff) as u8
            }
            IER if !self.dlab() => self.ier,
            IER if self.dlab() => {
                // Divisor latch high
                ((self.divisor >> 8) & 0xff) as u8
            }
            IIR => {
                // No interrupt pending
                0x01
            }
            LCR => self.lcr,
            MCR => self.mcr,
            LSR => {
                let mut status = LSR_THR_EMPTY | LSR_BOTH_EMPTY;
                if !self.input.is_empty() {
                    status |= LSR_DATA_READY;
                }
                status
            }
            MSR => {
                // CTS and DSR asserted
                0x30
            }
            SCR => self.scr,
            _ => 0,
        };
    }

    /// Write to a register.
    pub fn write(&mut self, offset: u16, data: &[u8]) {
        if data.is_empty() {
            return;
        }

        let value = data[0];

        match offset {
            THR if !self.dlab() => {
                // Write to transmit buffer (output)
                self.output.push_back(value);
            }
            THR if self.dlab() => {
                // Divisor latch low
                self.divisor = (self.divisor & 0xff00) | u16::from(value);
            }
            IER if !self.dlab() => {
                self.ier = value & 0x0f;
            }
            IER if self.dlab() => {
                // Divisor latch high
                self.divisor = (self.divisor & 0x00ff) | (u16::from(value) << 8);
            }
            FCR => {
                self.fcr = value;
                // Handle FIFO reset bits
                if value & 0x02 != 0 {
                    self.input.clear();
                }
                if value & 0x04 != 0 {
                    self.output.clear();
                }
            }
            LCR => {
                self.lcr = value;
            }
            MCR => {
                self.mcr = value & 0x1f;
            }
            SCR => {
                self.scr = value;
            }
            _ => {}
        }
    }

    /// Take all output data from the buffer.
    pub fn take_output(&mut self) -> Vec<u8> {
        self.output.drain(..).collect()
    }

    /// Queue input data to send to the guest.
    pub fn queue_input(&mut self, data: &[u8]) {
        self.input.extend(data);
    }
}
