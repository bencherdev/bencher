//! x86_64 vCPU configuration.
//!
//! This module handles the setup of vCPU registers, CPUID, and MSRs
//! for x86_64 guests booting in 64-bit long mode.

use kvm_bindings::{
    kvm_regs, kvm_sregs, CpuId, KVM_MAX_CPUID_ENTRIES,
};
use kvm_ioctls::{Kvm, VcpuFd};
use vm_memory::GuestMemoryMmap;

use crate::error::VmmError;

/// The address where boot parameters are stored.
pub const BOOT_PARAMS_ADDR: u64 = 0x7000;

/// The address where the kernel is loaded.
pub const KERNEL_LOAD_ADDR: u64 = 0x0100_0000; // 16 MiB

/// Configure an x86_64 vCPU.
pub fn configure_vcpu(
    kvm: &Kvm,
    vcpu_fd: &VcpuFd,
    _guest_memory: &GuestMemoryMmap,
    _index: u8,
) -> Result<(), VmmError> {
    // Setup CPUID
    setup_cpuid(kvm, vcpu_fd)?;

    // Setup special registers (segment registers, control registers)
    setup_sregs(vcpu_fd)?;

    // Setup general purpose registers
    setup_regs(vcpu_fd)?;

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

    // Setup code segment for 64-bit mode
    sregs.cs.base = 0;
    sregs.cs.limit = 0xffff_ffff;
    sregs.cs.selector = 0x10; // Second GDT entry (kernel code)
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
    sregs.ds.selector = 0x18; // Third GDT entry (kernel data)
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
    sregs.cr3 = 0;           // Page table address (will be set by kernel)
    sregs.cr4 = 0x20;        // PAE

    // Enable long mode
    sregs.efer = 0x500; // LME | LMA

    vcpu_fd.set_sregs(&sregs).map_err(VmmError::Kvm)?;

    Ok(())
}

/// Setup general purpose registers.
fn setup_regs(vcpu_fd: &VcpuFd) -> Result<(), VmmError> {
    let regs = kvm_regs {
        // Instruction pointer: 64-bit entry point
        rip: KERNEL_LOAD_ADDR + 0x200,

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

    Ok(())
}
