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
//! - Bundled Linux kernel (embedded in release builds)

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
mod sandbox;
#[cfg(target_os = "linux")]
mod vcpu;
#[cfg(target_os = "linux")]
mod vm;
#[cfg(target_os = "linux")]
mod vsock_client;

mod error;
mod kernel;

pub use error::VmmError;
pub use kernel::{kernel_bytes, write_kernel_to_file};

// Linux exports
#[cfg(target_os = "linux")]
pub use vm::{Vm, VmConfig, run_vm};
#[cfg(target_os = "linux")]
pub use vsock_client::{VsockClient, VsockClientBuilder, BenchmarkClient};

/// Vsock port constants for guest-host communication.
pub mod ports {
    /// Port for stdout (default results).
    pub const STDOUT: u32 = 5000;
    /// Port for stderr.
    pub const STDERR: u32 = 5001;
    /// Port for exit code.
    pub const EXIT_CODE: u32 = 5002;
    /// Port for output file (optional).
    pub const OUTPUT_FILE: u32 = 5005;
}

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
        pub timeout_secs: u64,
    }

    /// Default timeout in seconds (5 minutes).
    const DEFAULT_TIMEOUT_SECS: u64 = 300;

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
                timeout_secs: DEFAULT_TIMEOUT_SECS,
            }
        }

        /// Enable vsock communication with the given socket path.
        #[must_use]
        pub fn with_vsock(mut self, socket_path: Utf8PathBuf) -> Self {
            self.vsock_path = Some(socket_path);
            self
        }

        /// Set the execution timeout in seconds. Set to 0 to disable.
        #[must_use]
        pub fn with_timeout(mut self, timeout_secs: u64) -> Self {
            self.timeout_secs = timeout_secs;
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
