//! Bencher Init - Minimal init system for benchmark VMs.
//!
//! This is a purpose-built PID 1 for running benchmarks in isolated VMs.
//! It handles:
//! - Mounting essential filesystems (/proc, /dev, /sys, /tmp)
//! - Signal handling (SIGTERM for graceful shutdown)
//! - Running the benchmark command
//! - Zombie reaping
//! - Sending results via vsock
//! - Clean shutdown
//!
//! This binary is Linux-only and designed to run as the init process
//! inside a minimal VM guest.

#![expect(clippy::print_stderr)]
#![cfg_attr(not(target_os = "linux"), allow(unused_crate_dependencies))]

#[cfg(target_os = "linux")]
mod init;

#[cfg(target_os = "linux")]
fn main() -> std::process::ExitCode {
    // Try multiple methods to output immediately

    // Method 1: Direct write to stdout (fd 1)
    let msg = b"[bencher-init] main() entered\n";
    unsafe {
        libc::write(libc::STDOUT_FILENO, msg.as_ptr().cast(), msg.len());
    }

    // Method 2: Write directly to COM1 serial port (0x3F8) for early boot diagnostics.
    // This works before /dev is mounted because it uses raw x86 I/O port access.
    //
    // Protocol:
    //   0x3FD = Line Status Register (LSR) — read to check transmitter status
    //   0x3F8 = Data Register — write byte to transmit
    //   0x20  = LSR bit 5 (THRE) — Transmit Holding Register Empty flag
    //
    // Loop: poll LSR until THRE is set, then write one byte to the data register.
    // This requires iopl(3) or ioperm to be called first on Linux.
    #[cfg(target_arch = "x86_64")]
    unsafe {
        // Try to get I/O port access (may fail without root, but we're init)
        let _ = libc::iopl(3);

        // Write each character directly to COM1 data port
        let serial_msg = b"[bencher-init] SERIAL PORT OUTPUT\r\n";
        for &byte in serial_msg {
            // Wait for transmit holding register to be empty
            loop {
                let status: u8;
                std::arch::asm!(
                    "in al, dx",
                    in("dx") 0x3FD_u16,  // Line status register
                    out("al") status,
                    options(nostack, nomem, preserves_flags)
                );
                if status & 0x20 != 0 {
                    break; // THR empty
                }
            }
            // Write byte to data register
            std::arch::asm!(
                "out dx, al",
                in("dx") 0x3F8_u16,  // Data register
                in("al") byte,
                options(nostack, nomem, preserves_flags)
            );
        }
    }

    init::run()
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("bencher-init is only supported on Linux");
    std::process::exit(1);
}
