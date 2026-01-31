//! Guest memory management.
//!
//! This module handles the allocation and configuration of guest memory
//! using the vm-memory crate's GuestMemoryMmap abstraction.

use vm_memory::{GuestAddress, GuestMemoryMmap, GuestMemoryRegion};

use crate::error::VmmError;

/// The start address for guest memory (1 MiB).
/// This is after the real-mode IVT, BDA, and EBDA regions.
pub const GUEST_MEM_START: u64 = 0x0010_0000;

/// Create guest memory regions for the VM.
///
/// # Arguments
///
/// * `memory_mib` - The total memory size in MiB
///
/// # Returns
///
/// A `GuestMemoryMmap` instance with the configured memory regions.
pub fn create_guest_memory(memory_mib: u32) -> Result<GuestMemoryMmap, VmmError> {
    let memory_bytes = u64::from(memory_mib) * 1024 * 1024;

    // For simplicity, create a single contiguous memory region starting at 0.
    // In a production VMM, you might need multiple regions to work around
    // reserved areas (like the PCI hole on x86).
    let regions = vec![(GuestAddress(0), memory_bytes as usize)];

    GuestMemoryMmap::from_ranges(&regions).map_err(|e| VmmError::Memory(e.to_string()))
}

/// Get the total size of guest memory in bytes.
pub fn guest_memory_size(guest_memory: &GuestMemoryMmap) -> u64 {
    guest_memory
        .iter()
        .map(|region| region.len())
        .sum::<u64>()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_guest_memory() {
        let memory = create_guest_memory(128).expect("Failed to create guest memory");
        assert_eq!(guest_memory_size(&memory), 128 * 1024 * 1024);
    }
}
