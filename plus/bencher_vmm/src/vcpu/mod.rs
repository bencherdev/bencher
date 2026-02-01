//! vCPU creation and management.
//!
//! This module handles the architecture-specific setup of virtual CPUs.

#[cfg(target_arch = "x86_64")]
pub(crate) mod x86_64;

#[cfg(target_arch = "aarch64")]
pub(crate) mod aarch64;

use kvm_ioctls::{Kvm, VcpuFd, VmFd};
use vm_memory::GuestMemoryMmap;

use crate::error::VmmError;

/// A virtual CPU.
pub struct Vcpu {
    /// The vCPU file descriptor.
    pub fd: VcpuFd,

    /// The vCPU index.
    pub index: u8,
}

/// Create and configure vCPUs for the VM.
///
/// # Arguments
///
/// * `kvm` - The KVM instance
/// * `vm_fd` - The VM file descriptor
/// * `guest_memory` - The guest memory
/// * `vcpu_count` - The number of vCPUs to create
///
/// # Returns
///
/// A vector of configured vCPUs.
pub fn create_vcpus(
    kvm: &Kvm,
    vm_fd: &VmFd,
    guest_memory: &GuestMemoryMmap,
    vcpu_count: u8,
) -> Result<Vec<Vcpu>, VmmError> {
    let mut vcpus = Vec::with_capacity(vcpu_count as usize);

    for index in 0..vcpu_count {
        let vcpu_fd = vm_fd
            .create_vcpu(index as u64)
            .map_err(VmmError::Kvm)?;

        // Configure the vCPU (architecture-specific)
        configure_vcpu(kvm, vm_fd, &vcpu_fd, guest_memory, index)?;

        vcpus.push(Vcpu {
            fd: vcpu_fd,
            index,
        });
    }

    Ok(vcpus)
}

/// Configure a vCPU with architecture-specific settings.
#[cfg(target_arch = "x86_64")]
fn configure_vcpu(
    kvm: &Kvm,
    _vm_fd: &VmFd,
    vcpu_fd: &VcpuFd,
    guest_memory: &GuestMemoryMmap,
    index: u8,
) -> Result<(), VmmError> {
    x86_64::configure_vcpu(kvm, vcpu_fd, guest_memory, index)
}

#[cfg(target_arch = "aarch64")]
fn configure_vcpu(
    _kvm: &Kvm,
    vm_fd: &VmFd,
    vcpu_fd: &VcpuFd,
    guest_memory: &GuestMemoryMmap,
    index: u8,
) -> Result<(), VmmError> {
    aarch64::configure_vcpu(vm_fd, vcpu_fd, guest_memory, index)
}

#[cfg(not(any(target_arch = "x86_64", target_arch = "aarch64")))]
fn configure_vcpu(
    _kvm: &Kvm,
    _vm_fd: &VmFd,
    _vcpu_fd: &VcpuFd,
    _guest_memory: &GuestMemoryMmap,
    _index: u8,
) -> Result<(), VmmError> {
    Err(VmmError::UnsupportedArch)
}
