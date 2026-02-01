#![cfg(feature = "plus")]

//! Bencher VMM - A minimal Virtual Machine Monitor for benchmark runners.
//!
//! This crate provides a lightweight VMM built on the rust-vmm ecosystem,
//! designed specifically for running benchmarks in isolated VMs.
//!
//! # Platform Support
//!
//! This crate only works on Linux with KVM support. On other platforms,
//! stub types are provided that return errors when used.
//!
//! # Architecture Support (Linux only)
//!
//! - **`x86_64`**: Full support for Linux guests with `bzImage` kernels
//! - **`aarch64`**: Full support with GICv3/GICv2 and device tree generation
//!
//! # Features
//!
//! - Serial console (UART 16550A) for kernel output capture
//! - i8042 keyboard controller for clean shutdown
//! - virtio-blk for mounting squashfs rootfs
//! - virtio-vsock for host-guest communication

// Linux implementation
#[cfg(target_os = "linux")]
mod boot;
#[cfg(target_os = "linux")]
mod devices;
#[cfg(target_os = "linux")]
mod event_loop;
#[cfg(all(target_os = "linux", target_arch = "aarch64"))]
mod gic;
#[cfg(target_os = "linux")]
mod memory;
#[cfg(target_os = "linux")]
mod vcpu;
#[cfg(target_os = "linux")]
mod vm;
#[cfg(target_os = "linux")]
mod vsock_client;

mod error;

pub use error::VmmError;

// Linux exports
#[cfg(target_os = "linux")]
pub use vm::{Vm, VmConfig, run_vm};
#[cfg(target_os = "linux")]
pub use vsock_client::{VsockClient, VsockClientBuilder, BenchmarkClient};

// Non-Linux stubs
#[cfg(not(target_os = "linux"))]
mod stubs {
    use camino::Utf8PathBuf;
    use super::VmmError;

    /// Configuration for a virtual machine (stub for non-Linux).
    #[derive(Debug, Clone)]
    pub struct VmConfig {
        pub kernel_path: Utf8PathBuf,
        pub rootfs_path: Utf8PathBuf,
        pub vcpus: u8,
        pub memory_mib: u32,
        pub kernel_cmdline: String,
        pub vsock_path: Option<Utf8PathBuf>,
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
                vsock_path: None,
            }
        }

        /// Enable vsock communication with the given socket path.
        #[must_use]
        pub fn with_vsock(mut self, socket_path: Utf8PathBuf) -> Self {
            self.vsock_path = Some(socket_path);
            self
        }
    }

    /// A running virtual machine (stub for non-Linux).
    pub struct Vm;

    impl Vm {
        pub fn new(_config: &VmConfig) -> Result<Self, VmmError> {
            Err(VmmError::UnsupportedPlatform)
        }

        pub fn run(&mut self) -> Result<String, VmmError> {
            Err(VmmError::UnsupportedPlatform)
        }
    }

    /// Run a VM (stub for non-Linux).
    pub fn run_vm(_config: &VmConfig) -> Result<String, VmmError> {
        Err(VmmError::UnsupportedPlatform)
    }
}

#[cfg(not(target_os = "linux"))]
pub use stubs::{Vm, VmConfig, run_vm};
