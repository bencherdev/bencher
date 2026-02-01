//! aarch64 vCPU configuration.
//!
//! This module handles the setup of vCPU registers for ARM64 guests.

use kvm_bindings::{
    kvm_one_reg, kvm_vcpu_init, KVM_ARM_VCPU_PSCI_0_2, KVM_REG_ARM64, KVM_REG_ARM_CORE,
    KVM_REG_SIZE_U64,
};
use kvm_ioctls::{Kvm, VcpuFd};
use vm_memory::GuestMemoryMmap;

use crate::error::VmmError;

/// The address where the kernel is loaded on ARM64.
pub const KERNEL_LOAD_ADDR: u64 = 0x8000_0000; // 2 GiB

/// The address where the device tree blob is placed.
pub const DTB_ADDR: u64 = 0x8200_0000; // 2 GiB + 32 MiB

/// ARM64 core register indices (from struct user_pt_regs).
mod core_regs {
    /// X0-X30 general purpose registers.
    pub const X0: u64 = 0;
    pub const X1: u64 = 1;
    pub const X2: u64 = 2;
    pub const X3: u64 = 3;

    /// Program counter (PC).
    pub const PC: u64 = 32;

    /// Program state (PSTATE/SPSR_EL1).
    pub const PSTATE: u64 = 33;
}

/// PSTATE bits for EL1h mode.
const PSTATE_FAULT_BITS_64: u64 = 0x3c5; // EL1h, all interrupts masked

/// Configure an aarch64 vCPU.
pub fn configure_vcpu(
    kvm: &Kvm,
    vcpu_fd: &VcpuFd,
    _guest_memory: &GuestMemoryMmap,
    _index: u8,
) -> Result<(), VmmError> {
    // Get the preferred target CPU type
    let mut kvi = kvm_vcpu_init::default();
    kvm.get_preferred_target(&mut kvi)
        .map_err(|e| VmmError::Vcpu(format!("Failed to get preferred target: {e}")))?;

    // Enable PSCI v0.2 for CPU power management
    kvi.features[0] |= 1 << KVM_ARM_VCPU_PSCI_0_2;

    // Initialize the vCPU with the target
    vcpu_fd
        .vcpu_init(&kvi)
        .map_err(|e| VmmError::Vcpu(format!("Failed to init vCPU: {e}")))?;

    // Setup registers
    setup_regs(vcpu_fd)?;

    Ok(())
}

/// Setup registers for ARM64.
fn setup_regs(vcpu_fd: &VcpuFd) -> Result<(), VmmError> {
    // Set PC to kernel load address
    set_core_reg(vcpu_fd, core_regs::PC, KERNEL_LOAD_ADDR)?;

    // Set X0 to DTB address (first argument to kernel)
    set_core_reg(vcpu_fd, core_regs::X0, DTB_ADDR)?;

    // Clear X1, X2, X3 (reserved, must be zero per boot protocol)
    set_core_reg(vcpu_fd, core_regs::X1, 0)?;
    set_core_reg(vcpu_fd, core_regs::X2, 0)?;
    set_core_reg(vcpu_fd, core_regs::X3, 0)?;

    // Set PSTATE for EL1h mode with interrupts masked
    set_core_reg(vcpu_fd, core_regs::PSTATE, PSTATE_FAULT_BITS_64)?;

    Ok(())
}

/// Create an ARM64 core register ID.
///
/// Core registers use the struct user_pt_regs layout.
const fn arm64_core_reg_id(index: u64) -> u64 {
    // Each register is 8 bytes, so multiply index by 2 (in 4-byte units)
    KVM_REG_ARM64 | KVM_REG_SIZE_U64 | KVM_REG_ARM_CORE | (index * 2)
}

/// Set a core register value.
fn set_core_reg(vcpu_fd: &VcpuFd, index: u64, value: u64) -> Result<(), VmmError> {
    let reg_id = arm64_core_reg_id(index);
    let reg = kvm_one_reg {
        id: reg_id,
        addr: (&value as *const u64) as u64,
    };

    vcpu_fd
        .set_one_reg(reg_id, &value.to_le_bytes())
        .map_err(|e| VmmError::Vcpu(format!("Failed to set register {index}: {e}")))?;

    Ok(())
}
