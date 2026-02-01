//! VM lifecycle management.
//!
//! This module handles the creation, execution, and teardown of virtual machines.

use std::sync::{Arc, Mutex};

use camino::Utf8PathBuf;
use kvm_ioctls::{Kvm, VmFd};
use vm_memory::GuestMemoryMmap;

use crate::devices::DeviceManager;
use crate::error::VmmError;
use crate::memory::create_guest_memory;
use crate::vcpu::Vcpu;

/// Configuration for a virtual machine.
#[derive(Debug, Clone)]
pub struct VmConfig {
    /// Path to the Linux kernel image.
    pub kernel_path: Utf8PathBuf,

    /// Path to the squashfs rootfs image.
    pub rootfs_path: Utf8PathBuf,

    /// Number of vCPUs.
    pub vcpus: u8,

    /// Memory size in MiB.
    pub memory_mib: u32,

    /// Kernel command line arguments.
    pub kernel_cmdline: String,
}

impl VmConfig {
    /// Create a new VM configuration.
    pub fn new(kernel_path: Utf8PathBuf, rootfs_path: Utf8PathBuf) -> Self {
        Self {
            kernel_path,
            rootfs_path,
            vcpus: 1,
            memory_mib: 512,
            kernel_cmdline: "console=ttyS0 reboot=k panic=1 pci=off root=/dev/vda ro".to_owned(),
        }
    }
}

/// A running virtual machine.
pub struct Vm {
    /// The KVM file descriptor.
    /// Kept alive for the VM's lifetime (dropping closes KVM handle).
    _kvm: Kvm,

    /// The VM file descriptor.
    /// Kept alive for the VM's lifetime (dropping closes VM handle).
    _vm_fd: VmFd,

    /// Guest memory.
    /// Kept alive for the VM's lifetime (dropping unmaps memory).
    _guest_memory: GuestMemoryMmap,

    /// Virtual CPUs.
    vcpus: Vec<Vcpu>,

    /// Device manager.
    devices: Arc<Mutex<DeviceManager>>,
}

impl Vm {
    /// Create a new VM from configuration.
    pub fn new(config: &VmConfig) -> Result<Self, VmmError> {
        // Step 1: Open KVM
        let kvm = Kvm::new().map_err(VmmError::Kvm)?;

        // Check for required extensions
        check_kvm_extensions(&kvm)?;

        // Step 2: Create VM
        let vm_fd = kvm.create_vm().map_err(VmmError::Kvm)?;

        // Step 3: Create guest memory
        let memory_size = u64::from(config.memory_mib) * 1024 * 1024;
        let guest_memory = create_guest_memory(config.memory_mib)?;

        // Step 4: Register memory regions with KVM
        register_memory_regions(&vm_fd, &guest_memory)?;

        // Step 5: Create interrupt controller and load kernel (architecture-specific)
        #[cfg(target_arch = "x86_64")]
        {
            create_irq_chip_x86_64(&vm_fd)?;
            let _kernel_entry = crate::boot::load_kernel(
                &guest_memory,
                &config.kernel_path,
                &config.kernel_cmdline,
            )?;
        }

        #[cfg(target_arch = "aarch64")]
        {
            // Create GIC (tries GICv3, falls back to GICv2)
            let gic = crate::gic::Gic::new(&vm_fd, u64::from(config.vcpus))?;

            // Load kernel with device tree
            let _kernel_entry = crate::boot::load_kernel_aarch64(
                &guest_memory,
                &config.kernel_path,
                &config.kernel_cmdline,
                u32::from(config.vcpus),
                memory_size,
                &gic,
            )?;
        }

        // Step 6: Create vCPUs
        let vcpus = crate::vcpu::create_vcpus(&kvm, &vm_fd, &guest_memory, config.vcpus)?;

        // Step 7: Setup devices
        let devices = crate::devices::setup_devices(&vm_fd, &config.rootfs_path)?;

        Ok(Self {
            _kvm: kvm,
            _vm_fd: vm_fd,
            _guest_memory: guest_memory,
            vcpus,
            devices: Arc::new(Mutex::new(devices)),
        })
    }

    /// Run the VM until it shuts down.
    ///
    /// Returns the benchmark results collected via serial output.
    pub fn run(&mut self) -> Result<String, VmmError> {
        crate::event_loop::run(&mut self.vcpus, Arc::clone(&self.devices))
    }
}

/// Run a VM with the given configuration and return the benchmark results.
///
/// This is the main entry point for executing a benchmark in a VM.
///
/// # Arguments
///
/// * `config` - The VM configuration
///
/// # Returns
///
/// The benchmark results collected from the guest via serial output, as a JSON string.
pub fn run_vm(config: &VmConfig) -> Result<String, VmmError> {
    let mut vm = Vm::new(config)?;
    vm.run()
}

/// Check that required KVM extensions are available.
fn check_kvm_extensions(kvm: &Kvm) -> Result<(), VmmError> {
    // Check KVM API version
    let api_version = kvm.get_api_version();
    if api_version != 12 {
        return Err(VmmError::Kvm(kvm_ioctls::Error::new(libc::EINVAL)));
    }

    // Check for user memory extension
    if !kvm.check_extension(kvm_ioctls::Cap::UserMemory) {
        return Err(VmmError::Memory(
            "KVM_CAP_USER_MEMORY not supported".to_owned(),
        ));
    }

    Ok(())
}

/// Register guest memory regions with KVM.
fn register_memory_regions(
    vm_fd: &VmFd,
    guest_memory: &GuestMemoryMmap,
) -> Result<(), VmmError> {
    use vm_memory::GuestMemoryRegion;

    for (index, region) in guest_memory.iter().enumerate() {
        let mem_region = kvm_bindings::kvm_userspace_memory_region {
            slot: index as u32,
            guest_phys_addr: region.start_addr().0,
            memory_size: region.len(),
            userspace_addr: region.as_ptr() as u64,
            flags: 0,
        };

        // SAFETY: The memory region is properly configured and the vm_fd is valid.
        // The guest memory will remain valid for the lifetime of the VM.
        unsafe {
            vm_fd
                .set_user_memory_region(mem_region)
                .map_err(VmmError::Kvm)?;
        }
    }

    Ok(())
}

#[cfg(target_arch = "x86_64")]
fn create_irq_chip_x86_64(vm_fd: &VmFd) -> Result<(), VmmError> {
    // Create the in-kernel IRQ chip (PIC + IOAPIC + PIT)
    vm_fd.create_irq_chip().map_err(VmmError::Kvm)?;

    // Create the PIT (Programmable Interval Timer)
    let pit_config = kvm_bindings::kvm_pit_config::default();
    vm_fd.create_pit2(pit_config).map_err(VmmError::Kvm)?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(target_os = "linux")]
    fn test_kvm_open() {
        // This test requires /dev/kvm to be available
        if std::path::Path::new("/dev/kvm").exists() {
            let kvm = Kvm::new().expect("Failed to open KVM");
            assert!(check_kvm_extensions(&kvm).is_ok());
        }
    }
}
