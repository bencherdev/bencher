//! ARM64 Generic Interrupt Controller (GIC) setup.
//!
//! This module handles the creation and configuration of the virtual GIC
//! for ARM64 guests. It supports both GICv3 and GICv2, falling back to
//! GICv2 if GICv3 is not available.

#![cfg(all(target_os = "linux", target_arch = "aarch64"))]

use kvm_bindings::{
    kvm_create_device, kvm_device_attr, kvm_device_type_KVM_DEV_TYPE_ARM_VGIC_V2,
    kvm_device_type_KVM_DEV_TYPE_ARM_VGIC_V3, KVM_DEV_ARM_VGIC_CTRL_INIT,
    KVM_DEV_ARM_VGIC_GRP_ADDR, KVM_DEV_ARM_VGIC_GRP_CTRL, KVM_DEV_ARM_VGIC_GRP_NR_IRQS,
    KVM_VGIC_V2_ADDR_TYPE_CPU, KVM_VGIC_V2_ADDR_TYPE_DIST, KVM_VGIC_V3_ADDR_TYPE_DIST,
    KVM_VGIC_V3_ADDR_TYPE_REDIST,
};
use kvm_ioctls::{DeviceFd, VmFd};

use crate::error::VmmError;

/// GIC distributor base address.
/// Must be aligned to 64KB.
pub const GIC_DIST_BASE: u64 = 0x0800_0000;

/// GIC distributor size (64KB for GICv2, 64KB for GICv3).
pub const GIC_DIST_SIZE: u64 = 0x0001_0000;

/// GIC CPU interface base address (GICv2 only).
pub const GICV2_CPU_BASE: u64 = GIC_DIST_BASE + GIC_DIST_SIZE;

/// GIC CPU interface size (GICv2).
pub const GICV2_CPU_SIZE: u64 = 0x0002_0000;

/// GIC redistributor base address (GICv3 only).
/// Each vCPU has a 128KB region.
pub const GICV3_REDIST_BASE: u64 = GIC_DIST_BASE + GIC_DIST_SIZE;

/// GIC redistributor size per vCPU (128KB for GICv3).
pub const GICV3_REDIST_SIZE_PER_CPU: u64 = 0x0002_0000;

/// Number of IRQs to allocate.
/// SPIs start at 32, so this gives us 32-127 as usable SPIs.
pub const GIC_NR_IRQS: u32 = 128;

/// GIC version.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GicVersion {
    V2,
    V3,
}

/// A configured GIC device.
pub struct Gic {
    /// The device file descriptor.
    device_fd: DeviceFd,
    /// The GIC version.
    version: GicVersion,
    /// Number of vCPUs.
    vcpu_count: u64,
}

impl Gic {
    /// Create and configure the GIC.
    ///
    /// Tries GICv3 first, falls back to GICv2 if not available.
    pub fn new(vm_fd: &VmFd, vcpu_count: u64) -> Result<Self, VmmError> {
        // Try GICv3 first
        if let Ok(gic) = Self::create_gicv3(vm_fd, vcpu_count) {
            return Ok(gic);
        }

        // Fall back to GICv2
        Self::create_gicv2(vm_fd, vcpu_count)
    }

    /// Create a GICv3 device.
    fn create_gicv3(vm_fd: &VmFd, vcpu_count: u64) -> Result<Self, VmmError> {
        let mut gic_device = kvm_create_device {
            type_: kvm_device_type_KVM_DEV_TYPE_ARM_VGIC_V3,
            fd: 0,
            flags: 0,
        };

        let device_fd = vm_fd
            .create_device(&mut gic_device)
            .map_err(|e| VmmError::Gic(format!("Failed to create GICv3: {e}")))?;

        let gic = Self {
            device_fd,
            version: GicVersion::V3,
            vcpu_count,
        };

        gic.configure_gicv3()?;
        Ok(gic)
    }

    /// Create a GICv2 device.
    fn create_gicv2(vm_fd: &VmFd, vcpu_count: u64) -> Result<Self, VmmError> {
        let mut gic_device = kvm_create_device {
            type_: kvm_device_type_KVM_DEV_TYPE_ARM_VGIC_V2,
            fd: 0,
            flags: 0,
        };

        let device_fd = vm_fd
            .create_device(&mut gic_device)
            .map_err(|e| VmmError::Gic(format!("Failed to create GICv2: {e}")))?;

        let gic = Self {
            device_fd,
            version: GicVersion::V2,
            vcpu_count,
        };

        gic.configure_gicv2()?;
        Ok(gic)
    }

    /// Configure GICv3.
    fn configure_gicv3(&self) -> Result<(), VmmError> {
        // Set number of IRQs
        self.set_nr_irqs()?;

        // Set distributor address
        self.set_device_attr(
            KVM_DEV_ARM_VGIC_GRP_ADDR,
            u64::from(KVM_VGIC_V3_ADDR_TYPE_DIST),
            &GIC_DIST_BASE,
        )?;

        // Set redistributor address
        self.set_device_attr(
            KVM_DEV_ARM_VGIC_GRP_ADDR,
            u64::from(KVM_VGIC_V3_ADDR_TYPE_REDIST),
            &GICV3_REDIST_BASE,
        )?;

        // Initialize the GIC
        self.init()?;

        Ok(())
    }

    /// Configure GICv2.
    fn configure_gicv2(&self) -> Result<(), VmmError> {
        // Set number of IRQs
        self.set_nr_irqs()?;

        // Set distributor address
        self.set_device_attr(
            KVM_DEV_ARM_VGIC_GRP_ADDR,
            u64::from(KVM_VGIC_V2_ADDR_TYPE_DIST),
            &GIC_DIST_BASE,
        )?;

        // Set CPU interface address
        self.set_device_attr(
            KVM_DEV_ARM_VGIC_GRP_ADDR,
            u64::from(KVM_VGIC_V2_ADDR_TYPE_CPU),
            &GICV2_CPU_BASE,
        )?;

        // Initialize the GIC
        self.init()?;

        Ok(())
    }

    /// Set the number of IRQs.
    fn set_nr_irqs(&self) -> Result<(), VmmError> {
        let nr_irqs = u64::from(GIC_NR_IRQS);
        self.set_device_attr(KVM_DEV_ARM_VGIC_GRP_NR_IRQS, 0, &nr_irqs)
    }

    /// Initialize the GIC.
    fn init(&self) -> Result<(), VmmError> {
        let init_attr = kvm_device_attr {
            group: KVM_DEV_ARM_VGIC_GRP_CTRL,
            attr: u64::from(KVM_DEV_ARM_VGIC_CTRL_INIT),
            addr: 0,
            flags: 0,
        };

        self.device_fd
            .set_device_attr(&init_attr)
            .map_err(|e| VmmError::Gic(format!("Failed to initialize GIC: {e}")))
    }

    /// Set a device attribute.
    fn set_device_attr<T>(&self, group: u32, attr: u64, value: &T) -> Result<(), VmmError> {
        let device_attr = kvm_device_attr {
            group,
            attr,
            addr: (value as *const T) as u64,
            flags: 0,
        };

        self.device_fd
            .set_device_attr(&device_attr)
            .map_err(|e| VmmError::Gic(format!("Failed to set GIC attribute: {e}")))
    }

    /// Get the GIC version.
    #[must_use]
    pub fn version(&self) -> GicVersion {
        self.version
    }

    /// Get the GIC distributor base address.
    #[must_use]
    pub const fn dist_base(&self) -> u64 {
        GIC_DIST_BASE
    }

    /// Get the GIC distributor size.
    #[must_use]
    pub const fn dist_size(&self) -> u64 {
        GIC_DIST_SIZE
    }

    /// Get the GIC CPU/redistributor base address.
    #[must_use]
    pub fn cpu_base(&self) -> u64 {
        match self.version {
            GicVersion::V2 => GICV2_CPU_BASE,
            GicVersion::V3 => GICV3_REDIST_BASE,
        }
    }

    /// Get the GIC CPU/redistributor size.
    #[must_use]
    pub fn cpu_size(&self) -> u64 {
        match self.version {
            GicVersion::V2 => GICV2_CPU_SIZE,
            GicVersion::V3 => GICV3_REDIST_SIZE_PER_CPU * self.vcpu_count,
        }
    }
}
