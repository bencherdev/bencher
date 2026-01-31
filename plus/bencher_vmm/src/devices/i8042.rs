//! i8042 keyboard controller emulation.
//!
//! This provides minimal i8042 emulation for handling VM shutdown.
//! When the guest writes the reset command, we signal VM termination.

/// i8042 commands.
const CMD_RESET_CPU: u8 = 0xfe;

/// An i8042 keyboard controller device.
///
/// This is a minimal implementation that only handles CPU reset commands.
pub struct I8042Device {
    /// Data output buffer.
    data: u8,

    /// Status register.
    status: u8,

    /// Command register.
    command: u8,
}

impl I8042Device {
    /// Create a new i8042 device.
    pub fn new() -> Self {
        Self {
            data: 0,
            status: 0,
            command: 0,
        }
    }

    /// Read from the device.
    pub fn read(&self, port: u16, data: &mut [u8]) {
        if data.is_empty() {
            return;
        }

        data[0] = match port {
            0x60 => self.data,
            0x64 => self.status,
            _ => 0,
        };
    }

    /// Write to the device.
    ///
    /// Returns `true` if the guest requested a CPU reset (shutdown).
    pub fn write(&mut self, port: u16, data: &[u8]) -> bool {
        if data.is_empty() {
            return false;
        }

        let value = data[0];

        match port {
            0x60 => {
                self.data = value;
                false
            }
            0x64 => {
                self.command = value;

                // Check for CPU reset command
                if value == CMD_RESET_CPU {
                    return true;
                }

                false
            }
            _ => false,
        }
    }
}

impl Default for I8042Device {
    fn default() -> Self {
        Self::new()
    }
}
