//! Kernel loading and boot protocol implementation.
//!
//! This module handles loading the Linux kernel and setting up boot parameters
//! according to the Linux boot protocol for each architecture.

#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "aarch64")]
mod aarch64;

#[cfg(target_arch = "aarch64")]
pub use aarch64::load_kernel as load_kernel_aarch64;

#[cfg(target_arch = "x86_64")]
pub use x86_64::load_kernel;

use camino::Utf8Path;
use vm_memory::GuestMemoryMmap;

use crate::error::VmmError;

/// The kernel entry point address after loading.
pub struct KernelEntry {
    /// The address where execution should begin.
    pub entry_addr: u64,
}

/// Load a Linux kernel into guest memory (x86_64).
#[cfg(target_arch = "x86_64")]
pub fn load_kernel_x86_64(
    guest_memory: &GuestMemoryMmap,
    kernel_path: &Utf8Path,
    cmdline: &str,
) -> Result<KernelEntry, VmmError> {
    x86_64::load_kernel(guest_memory, kernel_path, cmdline)
}
