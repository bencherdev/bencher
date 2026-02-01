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
    init::run()
}

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("bencher-init is only supported on Linux");
    std::process::exit(1);
}
