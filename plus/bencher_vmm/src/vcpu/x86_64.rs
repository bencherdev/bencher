//! x86_64 vCPU configuration.
//!
//! This module handles the setup of vCPU registers, CPUID, and MSRs
//! for x86_64 guests booting in 64-bit long mode.

use kvm_bindings::{
    kvm_regs, kvm_sregs, CpuId, KVM_MAX_CPUID_ENTRIES,
};
use kvm_ioctls::{Kvm, VcpuFd};
use vm_memory::{Bytes, GuestAddress, GuestMemory, GuestMemoryMmap};

use crate::error::VmmError;

/// The address where boot parameters are stored.
pub const BOOT_PARAMS_ADDR: u64 = 0x7000;

/// The address where the kernel is loaded.
pub const KERNEL_LOAD_ADDR: u64 = 0x0100_0000; // 16 MiB

/// GDT address in guest memory.
const GDT_ADDR: u64 = 0x1000;

/// Page tables address (PML4).
const PML4_ADDR: u64 = 0x9000;

/// PDPT address.
const PDPT_ADDR: u64 = 0xa000;

/// PD address for first 1GB.
const PD_ADDR: u64 = 0xb000;

/// PD address for 3-4GB range (for virtio MMIO devices at 0xd0000000).
const PD_ADDR_HIGH: u64 = 0xc000;

/// Configure an x86_64 vCPU.
pub fn configure_vcpu(
    kvm: &Kvm,
    vcpu_fd: &VcpuFd,
    guest_memory: &GuestMemoryMmap,
    _index: u8,
    entry_point: u64,
) -> Result<(), VmmError> {
    // Setup GDT in guest memory
    setup_gdt(guest_memory)?;

    // Setup page tables for identity mapping
    setup_page_tables(guest_memory)?;

    // Setup CPUID
    setup_cpuid(kvm, vcpu_fd)?;

    // Setup special registers (segment registers, control registers)
    setup_sregs(vcpu_fd)?;

    // Setup general purpose registers with the actual entry point
    setup_regs(vcpu_fd, entry_point)?;

    Ok(())
}

/// Setup GDT in guest memory.
///
/// Creates a minimal GDT with null, code, and data segments.
fn setup_gdt(guest_memory: &GuestMemoryMmap) -> Result<(), VmmError> {
    // GDT entries (each 8 bytes):
    // 0x00: Null descriptor
    // 0x08: Unused (for alignment)
    // 0x10: 64-bit code segment (selector 0x10)
    // 0x18: 64-bit data segment (selector 0x18)
    let gdt: [u64; 4] = [
        0x0000_0000_0000_0000, // Null descriptor
        0x0000_0000_0000_0000, // Unused
        0x00af_9a00_0000_ffff, // Code: base=0, limit=0xfffff, type=0xa (exec/read), L=1 (64-bit), P=1
        0x00cf_9200_0000_ffff, // Data: base=0, limit=0xfffff, type=0x2 (read/write), P=1
    ];

    // Write each GDT entry as little-endian u64
    for (i, entry) in gdt.iter().enumerate() {
        guest_memory
            .write_obj(*entry, GuestAddress(GDT_ADDR + i as u64 * 8))
            .map_err(|e| VmmError::Boot(format!("Failed to write GDT entry {i}: {e}")))?;
    }

    Ok(())
}

/// Setup page tables for identity mapping.
///
/// Creates identity mappings for:
/// - First 1GB of memory (0x0000_0000 - 0x3FFF_FFFF) using 2MB pages
/// - 3-4GB range (0xC000_0000 - 0xFFFF_FFFF) for virtio MMIO devices at 0xD000_0000
fn setup_page_tables(guest_memory: &GuestMemoryMmap) -> Result<(), VmmError> {
    // PML4 entry pointing to PDPT
    // Present (bit 0), Writable (bit 1), User (bit 2 - optional)
    let pml4_entry: u64 = PDPT_ADDR | 0b11; // P + W
    guest_memory
        .write_obj(pml4_entry, GuestAddress(PML4_ADDR))
        .map_err(|e| VmmError::Boot(format!("Failed to write PML4: {e}")))?;

    // PDPT entry 0 pointing to PD for first 1GB (0x00000000 - 0x3FFFFFFF)
    let pdpt_entry_0: u64 = PD_ADDR | 0b11; // P + W
    guest_memory
        .write_obj(pdpt_entry_0, GuestAddress(PDPT_ADDR))
        .map_err(|e| VmmError::Boot(format!("Failed to write PDPT[0]: {e}")))?;

    // PDPT entry 3 pointing to PD for 3-4GB range (0xC0000000 - 0xFFFFFFFF)
    // This is needed for virtio MMIO devices at 0xD0000000
    let pdpt_entry_3: u64 = PD_ADDR_HIGH | 0b11; // P + W
    guest_memory
        .write_obj(pdpt_entry_3, GuestAddress(PDPT_ADDR + 3 * 8))
        .map_err(|e| VmmError::Boot(format!("Failed to write PDPT[3]: {e}")))?;

    // PD entries for first 1GB: 512 x 2MB pages = 1GB identity mapped
    // Each entry maps 2MB with PS (Page Size) bit set for huge pages
    for i in 0u64..512 {
        let pd_entry: u64 = (i * 0x20_0000) | 0b1000_0011; // P + W + PS (2MB page)
        guest_memory
            .write_obj(pd_entry, GuestAddress(PD_ADDR + i * 8))
            .map_err(|e| VmmError::Boot(format!("Failed to write PD[0] entry {i}: {e}")))?;
    }

    // PD entries for 3-4GB range: 512 x 2MB pages = 1GB identity mapped
    // Maps 0xC0000000 - 0xFFFFFFFF
    // virtio devices are at 0xD0000000 which is in this range
    for i in 0u64..512 {
        // Base address for 3-4GB range is 0xC0000000 = 3GB
        let phys_addr = 0xC000_0000 + (i * 0x20_0000);
        // Mark as uncacheable for MMIO (PCD=1, PWT=1) in addition to P + W + PS
        let pd_entry: u64 = phys_addr | 0b1001_1011; // P + W + PWT + PCD + PS
        guest_memory
            .write_obj(pd_entry, GuestAddress(PD_ADDR_HIGH + i * 8))
            .map_err(|e| VmmError::Boot(format!("Failed to write PD[3] entry {i}: {e}")))?;
    }

    Ok(())
}

/// Setup CPUID for the vCPU.
fn setup_cpuid(kvm: &Kvm, vcpu_fd: &VcpuFd) -> Result<(), VmmError> {
    // Get the supported CPUID entries from KVM
    let mut cpuid = kvm
        .get_supported_cpuid(KVM_MAX_CPUID_ENTRIES)
        .map_err(VmmError::Kvm)?;

    // Filter and modify CPUID entries as needed
    // For a minimal VMM, we mostly use the host's CPUID
    filter_cpuid(&mut cpuid);

    vcpu_fd.set_cpuid2(&cpuid).map_err(VmmError::Kvm)?;

    Ok(())
}

/// Filter CPUID entries to hide certain features or set specific values.
fn filter_cpuid(cpuid: &mut CpuId) {
    for entry in cpuid.as_mut_slice() {
        match entry.function {
            // Processor Brand String - could customize this
            0x8000_0002..=0x8000_0004 => {}

            // Hide hypervisor presence if desired (not recommended for paravirt)
            // 0x1 => entry.ecx &= !(1 << 31),

            _ => {}
        }
    }
}

/// Setup special registers for 64-bit long mode.
fn setup_sregs(vcpu_fd: &VcpuFd) -> Result<(), VmmError> {
    let mut sregs = vcpu_fd.get_sregs().map_err(VmmError::Kvm)?;

    // Setup GDT register to point to our GDT
    sregs.gdt.base = GDT_ADDR;
    sregs.gdt.limit = 31; // 4 entries * 8 bytes - 1

    // Setup code segment for 64-bit mode
    sregs.cs.base = 0;
    sregs.cs.limit = 0xffff_ffff;
    sregs.cs.selector = 0x10; // Third GDT entry (kernel code)
    sregs.cs.type_ = 0xb;     // Execute/Read, accessed
    sregs.cs.present = 1;
    sregs.cs.dpl = 0;         // Ring 0
    sregs.cs.db = 0;          // Not 32-bit
    sregs.cs.s = 1;           // Code/data segment
    sregs.cs.l = 1;           // 64-bit mode
    sregs.cs.g = 1;           // 4K granularity

    // Setup data segment
    sregs.ds.base = 0;
    sregs.ds.limit = 0xffff_ffff;
    sregs.ds.selector = 0x18; // Fourth GDT entry (kernel data)
    sregs.ds.type_ = 0x3;     // Read/Write, accessed
    sregs.ds.present = 1;
    sregs.ds.dpl = 0;
    sregs.ds.db = 1;          // 32-bit
    sregs.ds.s = 1;
    sregs.ds.l = 0;
    sregs.ds.g = 1;

    // Copy DS settings to other data segments
    sregs.es = sregs.ds;
    sregs.fs = sregs.ds;
    sregs.gs = sregs.ds;
    sregs.ss = sregs.ds;

    // Enable protected mode and paging
    sregs.cr0 = 0x8003_0031; // PE, MP, ET, NE, WP, PG
    sregs.cr3 = PML4_ADDR;   // Page table address
    sregs.cr4 = 0x20;        // PAE

    // Enable long mode
    sregs.efer = 0x500; // LME | LMA

    vcpu_fd.set_sregs(&sregs).map_err(VmmError::Kvm)?;

    Ok(())
}

/// Setup general purpose registers.
fn setup_regs(vcpu_fd: &VcpuFd, entry_point: u64) -> Result<(), VmmError> {
    let regs = kvm_regs {
        // Instruction pointer: kernel entry point
        rip: entry_point,

        // RSI: pointer to boot_params
        rsi: BOOT_PARAMS_ADDR,

        // RFLAGS: interrupts disabled, reserved bit set
        rflags: 0x2,

        // Stack pointer (will be set by kernel)
        rsp: 0,

        // Clear other registers
        rax: 0,
        rbx: 0,
        rcx: 0,
        rdx: 0,
        rdi: 0,
        rbp: 0,
        r8: 0,
        r9: 0,
        r10: 0,
        r11: 0,
        r12: 0,
        r13: 0,
        r14: 0,
        r15: 0,
    };

    vcpu_fd.set_regs(&regs).map_err(VmmError::Kvm)?;

    eprintln!(
        "[VCPU] x86_64 configured: rip={:#x}, rsi={:#x} (boot_params), rflags={:#x}",
        regs.rip, regs.rsi, regs.rflags
    );

    Ok(())
}
