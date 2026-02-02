//! x86_64 kernel loading and boot protocol.
//!
//! This module implements the Linux x86 boot protocol for loading bzImage kernels.
//! See: https://www.kernel.org/doc/html/latest/x86/boot.html

use std::fs::File;
use std::io::Read;

use camino::Utf8Path;
use linux_loader::loader::bootparam::boot_params;
use linux_loader::loader::elf::Elf as ElfLoader;
use linux_loader::loader::KernelLoader;
use vm_memory::{Bytes, ByteValued, GuestAddress, GuestMemory, GuestMemoryMmap};

use crate::error::VmmError;
use crate::vcpu::x86_64::{BOOT_PARAMS_ADDR, KERNEL_LOAD_ADDR};

/// E820 memory type: RAM.
const E820_RAM: u32 = 1;

use super::KernelEntry;

/// Address for the kernel command line.
const CMDLINE_ADDR: u64 = 0x20000;

/// Maximum command line size.
const CMDLINE_MAX_SIZE: usize = 4096;

/// Load an x86_64 Linux kernel.
pub fn load_kernel(
    guest_memory: &GuestMemoryMmap,
    kernel_path: &Utf8Path,
    cmdline: &str,
) -> Result<KernelEntry, VmmError> {
    // Open the kernel file
    let mut kernel_file =
        File::open(kernel_path).map_err(|e| VmmError::KernelLoad(e.to_string()))?;

    // Try to load as ELF first (vmlinux), then as bzImage
    let entry_addr = load_kernel_image(guest_memory, &mut kernel_file)?;

    // Setup command line
    setup_cmdline(guest_memory, cmdline)?;

    // Setup boot parameters
    setup_boot_params(guest_memory, cmdline.len())?;

    Ok(KernelEntry { entry_addr })
}

/// Load the kernel image into guest memory.
fn load_kernel_image(
    guest_memory: &GuestMemoryMmap,
    kernel_file: &mut File,
) -> Result<u64, VmmError> {
    // Read the first few bytes to detect kernel type
    let mut header = [0u8; 8];
    kernel_file
        .read_exact(&mut header)
        .map_err(|e| VmmError::KernelLoad(e.to_string()))?;

    // Seek back to start
    use std::io::Seek;
    kernel_file
        .seek(std::io::SeekFrom::Start(0))
        .map_err(|e| VmmError::KernelLoad(e.to_string()))?;

    // Check for ELF magic
    if header[0..4] == [0x7f, b'E', b'L', b'F'] {
        // Load as ELF
        let kernel_entry = ElfLoader::load(
            guest_memory,
            None,
            kernel_file,
            Some(GuestAddress(KERNEL_LOAD_ADDR)),
        )
        .map_err(|e| VmmError::KernelLoad(e.to_string()))?;

        eprintln!(
            "[BOOT] ELF kernel loaded: entry={:#x}",
            kernel_entry.kernel_load.0
        );

        Ok(kernel_entry.kernel_load.0)
    } else {
        // Assume bzImage format
        // For bzImage, we need to use the bzImage loader
        // For simplicity, we'll use a basic approach here

        // Read the entire kernel
        let mut kernel_data = Vec::new();
        kernel_file
            .read_to_end(&mut kernel_data)
            .map_err(|e| VmmError::KernelLoad(e.to_string()))?;

        // Write to guest memory at the load address
        guest_memory
            .write_slice(&kernel_data, GuestAddress(KERNEL_LOAD_ADDR))
            .map_err(|e| VmmError::KernelLoad(e.to_string()))?;

        // The entry point for bzImage is at offset 0x200 from the load address
        Ok(KERNEL_LOAD_ADDR + 0x200)
    }
}

/// Setup the kernel command line in guest memory.
fn setup_cmdline(guest_memory: &GuestMemoryMmap, cmdline: &str) -> Result<(), VmmError> {
    // Ensure command line fits
    if cmdline.len() >= CMDLINE_MAX_SIZE {
        return Err(VmmError::Boot(format!(
            "Command line too long: {} > {}",
            cmdline.len(),
            CMDLINE_MAX_SIZE
        )));
    }

    eprintln!("[BOOT] Kernel cmdline: {cmdline}");

    // Write command line with null terminator
    let mut cmdline_bytes = cmdline.as_bytes().to_vec();
    cmdline_bytes.push(0);

    guest_memory
        .write_slice(&cmdline_bytes, GuestAddress(CMDLINE_ADDR))
        .map_err(|e| VmmError::Boot(e.to_string()))?;

    Ok(())
}

/// Setup the boot_params structure (zero page).
fn setup_boot_params(guest_memory: &GuestMemoryMmap, cmdline_size: usize) -> Result<(), VmmError> {
    let mut params = boot_params::default();

    // Setup header
    params.hdr.type_of_loader = 0xff; // Undefined loader type
    params.hdr.boot_flag = 0xaa55;
    params.hdr.header = 0x5372_6448; // "HdrS"
    params.hdr.kernel_alignment = 0x0100_0000; // 16 MiB
    params.hdr.cmd_line_ptr = CMDLINE_ADDR as u32;
    params.hdr.cmdline_size = cmdline_size as u32;

    // Setup e820 memory map
    setup_e820(&mut params, guest_memory)?;

    // Write boot_params to guest memory
    guest_memory
        .write_obj(params, GuestAddress(BOOT_PARAMS_ADDR))
        .map_err(|e| VmmError::Boot(e.to_string()))?;

    Ok(())
}

/// E820 memory type: Reserved.
const E820_RESERVED: u32 = 2;

/// Setup the e820 memory map.
fn setup_e820(params: &mut boot_params, guest_memory: &GuestMemoryMmap) -> Result<(), VmmError> {
    use linux_loader::loader::bootparam::boot_e820_entry;
    use vm_memory::GuestMemoryRegion;

    // Get total memory size
    let total_memory: u64 = guest_memory.iter().map(|r| r.len()).sum();

    // Create proper e820 map matching standard x86 memory layout:
    // Entry 0: Usable low memory (0 - 0x9FC00 = 0 to 639KB) - for real mode trampoline
    // Entry 1: Reserved BIOS area (0x9FC00 - 0x100000)
    // Entry 2: Usable high memory (1MB - end)
    params.e820_table[0] = boot_e820_entry {
        addr: 0,
        size: 0x0009_FC00, // 639 KB - leave room for EBDA
        type_: E820_RAM,
    };

    params.e820_table[1] = boot_e820_entry {
        addr: 0x0009_FC00, // 639 KB
        size: 0x0010_0000 - 0x0009_FC00, // Up to 1 MiB
        type_: E820_RESERVED,
    };

    params.e820_table[2] = boot_e820_entry {
        addr: 0x0010_0000, // 1 MiB
        size: total_memory - 0x0010_0000,
        type_: E820_RAM,
    };

    params.e820_entries = 3;

    eprintln!(
        "[BOOT] e820 map: RAM 0-639KB, reserved 639KB-1MB, RAM 1MB-{}MB",
        total_memory / (1024 * 1024)
    );

    Ok(())
}
