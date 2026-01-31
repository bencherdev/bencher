//! Kernel loading and boot protocol implementation.
//!
//! This module handles loading the Linux kernel and setting up boot parameters
//! according to the Linux boot protocol for each architecture.

#[cfg(target_arch = "x86_64")]
mod x86_64;

#[cfg(target_arch = "aarch64")]
mod aarch64;

use camino::Utf8Path;
use vm_memory::GuestMemoryMmap;

use crate::error::VmmError;

/// The kernel entry point address after loading.
pub struct KernelEntry {
    /// The address where execution should begin.
    pub entry_addr: u64,
}

/// Load a Linux kernel into guest memory.
///
/// # Arguments
///
/// * `guest_memory` - The guest memory to load the kernel into
/// * `kernel_path` - Path to the kernel image
/// * `cmdline` - Kernel command line arguments
///
/// # Returns
///
/// The kernel entry point information.
pub fn load_kernel(
    guest_memory: &GuestMemoryMmap,
    kernel_path: &Utf8Path,
    cmdline: &str,
) -> Result<KernelEntry, VmmError> {
    #[cfg(target_arch = "x86_64")]
    {
        x86_64::load_kernel(guest_memory, kernel_path, cmdline)
    }

    #[cfg(target_arch = "aarch64")]
    {
        aarch64::load_kernel(guest_memory, kernel_path, cmdline)
    }

    #[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
    {
        let _ = (guest_memory, kernel_path, cmdline);
        Err(VmmError::UnsupportedArch)
    }
}
