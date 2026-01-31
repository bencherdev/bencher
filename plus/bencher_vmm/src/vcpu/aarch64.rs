//! aarch64 vCPU configuration.
//!
//! This module handles the setup of vCPU registers for ARM64 guests.

use kvm_ioctls::{Kvm, VcpuFd};
use vm_memory::GuestMemoryMmap;

use crate::error::VmmError;

/// The address where the kernel is loaded on ARM64.
pub const KERNEL_LOAD_ADDR: u64 = 0x8000_0000; // 2 GiB

/// The address where the device tree blob is placed.
pub const DTB_ADDR: u64 = 0x8200_0000; // 2 GiB + 32 MiB

/// Configure an aarch64 vCPU.
pub fn configure_vcpu(
    _kvm: &Kvm,
    vcpu_fd: &VcpuFd,
    _guest_memory: &GuestMemoryMmap,
    _index: u8,
) -> Result<(), VmmError> {
    // Initialize the vCPU
    vcpu_fd.vcpu_init(&Default::default()).map_err(VmmError::Kvm)?;

    // Setup registers
    setup_regs(vcpu_fd)?;

    Ok(())
}

/// Setup registers for ARM64.
fn setup_regs(vcpu_fd: &VcpuFd) -> Result<(), VmmError> {
    // ARM64 uses a different register interface
    // We need to set:
    // - PC (program counter) to kernel entry
    // - X0 to DTB address
    // - PSTATE for EL1

    // Set PC to kernel load address
    set_one_reg(vcpu_fd, arm64_reg_id(32, 2), KERNEL_LOAD_ADDR)?;

    // Set X0 to DTB address (first argument to kernel)
    set_one_reg(vcpu_fd, arm64_reg_id(0, 0), DTB_ADDR)?;

    // Clear X1, X2, X3 (reserved, must be zero)
    set_one_reg(vcpu_fd, arm64_reg_id(1, 0), 0)?;
    set_one_reg(vcpu_fd, arm64_reg_id(2, 0), 0)?;
    set_one_reg(vcpu_fd, arm64_reg_id(3, 0), 0)?;

    Ok(())
}

/// Create an ARM64 register ID.
///
/// This follows the KVM register encoding for ARM64.
const fn arm64_reg_id(index: u64, size: u64) -> u64 {
    const KVM_REG_ARM64: u64 = 0x6000_0000_0000_0000;
    const KVM_REG_ARM_CORE: u64 = 0x0010_0000_0000_0000;
    const KVM_REG_SIZE_U64: u64 = 0x0030_0000_0000_0000;
    const KVM_REG_SIZE_U32: u64 = 0x0020_0000_0000_0000;

    let size_bits = if size == 0 {
        KVM_REG_SIZE_U64
    } else {
        KVM_REG_SIZE_U32
    };

    KVM_REG_ARM64 | size_bits | KVM_REG_ARM_CORE | (index * 4)
}

/// Set a single register value.
fn set_one_reg(vcpu_fd: &VcpuFd, reg_id: u64, value: u64) -> Result<(), VmmError> {
    // The actual implementation would use vcpu_fd.set_one_reg()
    // This is a placeholder - the kvm-ioctls crate has architecture-specific methods
    let _ = (vcpu_fd, reg_id, value);

    // TODO: Implement actual register setting for ARM64
    // vcpu_fd.set_one_reg(reg_id, &value.to_le_bytes())
    //     .map_err(|e| VmmError::Vcpu(e.to_string()))?;

    Ok(())
}
