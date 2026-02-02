//! Intel 8254 Programmable Interval Timer (PIT) emulation.
//!
//! This provides a minimal PIT implementation to generate timer interrupts
//! for the Linux kernel scheduler.

use std::time::{Duration, Instant};

/// PIT I/O port addresses.
pub const PIT_CHANNEL_0: u16 = 0x40;
pub const PIT_CHANNEL_1: u16 = 0x41;
pub const PIT_CHANNEL_2: u16 = 0x42;
pub const PIT_MODE_COMMAND: u16 = 0x43;

/// PIT base frequency (1.193182 MHz).
const PIT_FREQUENCY: f64 = 1_193_182.0;

/// Timer IRQ (IRQ0).
pub const TIMER_IRQ: u32 = 0;

/// Programmable Interval Timer device.
pub struct PitDevice {
    /// Channel 0 reload value (counter).
    channel0_reload: u16,
    /// Channel 0 current mode.
    channel0_mode: u8,
    /// Last time we fired an interrupt.
    last_tick: Instant,
    /// Whether interrupts are enabled.
    interrupt_enabled: bool,
}

impl PitDevice {
    /// Create a new PIT device.
    #[must_use]
    pub fn new() -> Self {
        Self {
            channel0_reload: 0,
            channel0_mode: 0,
            last_tick: Instant::now(),
            interrupt_enabled: false,
        }
    }

    /// Read from a PIT register.
    pub fn read(&mut self, port: u16, data: &mut [u8]) {
        match port {
            PIT_CHANNEL_0 | PIT_CHANNEL_1 | PIT_CHANNEL_2 => {
                // Reading from channels is complex (latch counting)
                // For simplicity, return 0
                data.fill(0);
            }
            PIT_MODE_COMMAND => {
                // Read-back command not supported, return 0
                data.fill(0);
            }
            _ => {
                data.fill(0);
            }
        }
    }

    /// Write to a PIT register.
    pub fn write(&mut self, port: u16, data: &[u8]) {
        if data.is_empty() {
            return;
        }
        let value = data[0];

        match port {
            PIT_CHANNEL_0 => {
                // Writing to channel 0 counter
                // For simplicity, treat as low byte of reload value
                self.channel0_reload = u16::from(value);
                self.interrupt_enabled = true;
                self.last_tick = Instant::now();
            }
            PIT_MODE_COMMAND => {
                // Mode/command register
                // Bits 7-6: Select channel (00 = channel 0)
                // Bits 5-4: Access mode (11 = low then high byte)
                // Bits 3-1: Operating mode (000-101)
                // Bit 0: BCD mode (0 = binary)
                let channel = (value >> 6) & 0x03;
                let mode = (value >> 1) & 0x07;

                if channel == 0 {
                    self.channel0_mode = mode;
                    // Mode 2 (rate generator) or 3 (square wave) are most common
                    if mode == 2 || mode == 3 {
                        self.interrupt_enabled = true;
                    }
                }
            }
            _ => {}
        }
    }

    /// Check if a timer interrupt should fire.
    ///
    /// Returns `true` if an interrupt should be injected.
    pub fn check_interrupt(&mut self) -> bool {
        if !self.interrupt_enabled {
            return false;
        }

        // Calculate the period based on reload value
        let period = if self.channel0_reload == 0 {
            // Reload value 0 means 65536
            Duration::from_secs_f64(65536.0 / PIT_FREQUENCY)
        } else {
            Duration::from_secs_f64(f64::from(self.channel0_reload) / PIT_FREQUENCY)
        };

        // Use a minimum period of 1ms to avoid overwhelming the system
        let min_period = Duration::from_millis(1);
        let effective_period = period.max(min_period);

        let now = Instant::now();
        if now.duration_since(self.last_tick) >= effective_period {
            self.last_tick = now;
            return true;
        }

        false
    }
}

impl Default for PitDevice {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pit_new() {
        let pit = PitDevice::new();
        assert!(!pit.interrupt_enabled);
        assert_eq!(pit.channel0_reload, 0);
    }

    #[test]
    fn test_pit_mode_write() {
        let mut pit = PitDevice::new();
        // Write mode command: channel 0, low/high byte, mode 2 (rate generator)
        pit.write(PIT_MODE_COMMAND, &[0x34]);
        assert!(pit.interrupt_enabled);
        assert_eq!(pit.channel0_mode, 2);
    }

    #[test]
    fn test_pit_counter_write() {
        let mut pit = PitDevice::new();
        // Set mode first
        pit.write(PIT_MODE_COMMAND, &[0x34]);
        // Write counter value (low byte only for simplicity)
        pit.write(PIT_CHANNEL_0, &[0x00]); // ~1.19MHz / 256 = ~4.66kHz
        assert!(pit.interrupt_enabled);
    }
}
