//! aarch64 kernel loading and boot protocol.
//!
//! This module implements the ARM64 boot protocol for loading Image kernels.
//! ARM64 uses a device tree blob (DTB) instead of boot_params.

use std::fs::File;
use std::io::Read;

use camino::Utf8Path;
use linux_loader::loader::pe::PE as PeLoader;
use linux_loader::loader::KernelLoader;
use vm_memory::{GuestAddress, GuestMemoryMmap, GuestMemory};

use crate::error::VmmError;
use crate::vcpu::aarch64::{DTB_ADDR, KERNEL_LOAD_ADDR};

use super::KernelEntry;

/// Load an aarch64 Linux kernel.
pub fn load_kernel(
    guest_memory: &GuestMemoryMmap,
    kernel_path: &Utf8Path,
    cmdline: &str,
) -> Result<KernelEntry, VmmError> {
    // Open the kernel file
    let mut kernel_file =
        File::open(kernel_path).map_err(|e| VmmError::KernelLoad(e.to_string()))?;

    // Load kernel image
    let entry_addr = load_kernel_image(guest_memory, &mut kernel_file)?;

    // Setup device tree
    setup_device_tree(guest_memory, cmdline)?;

    Ok(KernelEntry { entry_addr })
}

/// Load the kernel image into guest memory.
fn load_kernel_image(
    guest_memory: &GuestMemoryMmap,
    kernel_file: &mut File,
) -> Result<u64, VmmError> {
    // Try to load as PE (ARM64 Image format)
    let result = PeLoader::load(
        guest_memory,
        None,
        kernel_file,
        Some(GuestAddress(KERNEL_LOAD_ADDR)),
    );

    match result {
        Ok(kernel_entry) => Ok(kernel_entry.kernel_load.0),
        Err(_) => {
            // Fall back to raw image loading
            use std::io::Seek;
            kernel_file
                .seek(std::io::SeekFrom::Start(0))
                .map_err(|e| VmmError::KernelLoad(e.to_string()))?;

            let mut kernel_data = Vec::new();
            kernel_file
                .read_to_end(&mut kernel_data)
                .map_err(|e| VmmError::KernelLoad(e.to_string()))?;

            guest_memory
                .write_slice(&kernel_data, GuestAddress(KERNEL_LOAD_ADDR))
                .map_err(|e| VmmError::KernelLoad(e.to_string()))?;

            Ok(KERNEL_LOAD_ADDR)
        }
    }
}

/// Setup the device tree blob for ARM64.
///
/// This creates a minimal device tree with:
/// - CPU configuration
/// - Memory configuration
/// - Chosen node with command line
fn setup_device_tree(guest_memory: &GuestMemoryMmap, cmdline: &str) -> Result<(), VmmError> {
    // TODO: Generate a proper device tree blob
    // For now, this is a placeholder. In a real implementation, we would:
    // 1. Create a device tree with cpu, memory, and interrupt controller nodes
    // 2. Add the chosen node with bootargs (command line)
    // 3. Flatten and write to guest memory at DTB_ADDR

    // A minimal DTB would be generated using a library like `vm-fdt`
    // For now, we just reserve the space

    let _ = (guest_memory, cmdline);

    // Placeholder: write a minimal valid FDT header
    // In production, use vm-fdt crate to generate the DTB
    let fdt_header = [
        0xd0, 0x0d, 0xfe, 0xed, // magic
        0x00, 0x00, 0x00, 0x48, // totalsize
        0x00, 0x00, 0x00, 0x38, // off_dt_struct
        0x00, 0x00, 0x00, 0x48, // off_dt_strings
        0x00, 0x00, 0x00, 0x28, // off_mem_rsvmap
        0x00, 0x00, 0x00, 0x11, // version
        0x00, 0x00, 0x00, 0x10, // last_comp_version
        0x00, 0x00, 0x00, 0x00, // boot_cpuid_phys
        0x00, 0x00, 0x00, 0x00, // size_dt_strings
        0x00, 0x00, 0x00, 0x10, // size_dt_struct
    ];

    guest_memory
        .write_slice(&fdt_header, GuestAddress(DTB_ADDR))
        .map_err(|e| VmmError::Boot(e.to_string()))?;

    Ok(())
}
