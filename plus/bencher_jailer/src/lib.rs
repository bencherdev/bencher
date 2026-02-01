#![cfg(feature = "plus")]
// Suppress unused crate warnings on non-Linux platforms where these are conditionally compiled out
#![cfg_attr(not(target_os = "linux"), allow(unused_crate_dependencies))]

//! Bencher Jailer - Security isolation for VMM processes.
//!
//! This crate provides a jail environment for running VMM processes with
//! multiple layers of isolation:
//!
//! - **Namespaces**: User, PID, mount, network, UTS, IPC, cgroup
//! - **Cgroups v2**: CPU, memory, PIDs limits
//! - **Chroot/`pivot_root`**: Isolated filesystem root
//! - **Capabilities**: All dropped
//! - **Seccomp**: Applied by the VMM itself
//! - **rlimits**: File descriptors, processes, file sizes
//!
//! # Platform Support
//!
//! This crate only works on Linux. On other platforms, stub types are provided
//! that return errors when used.
//!
//! # Example
//!
//! ```ignore
//! use bencher_jailer::{JailConfig, Jail};
//! use camino::Utf8PathBuf;
//!
//! let config = JailConfig::new(
//!     "benchmark-123",
//!     Utf8PathBuf::from("/usr/bin/vmm"),
//!     Utf8PathBuf::from("/var/lib/bencher/jails/benchmark-123"),
//! )
//! .with_args(vec!["--kernel".into(), "/kernel".into()])
//! .with_memory_limit(512 * 1024 * 1024)  // 512 MiB
//! .with_cpu_limit(1.0);  // 1 CPU
//!
//! let mut jail = Jail::new(config)?;
//! let exit_code = jail.run()?;
//! ```
//!
//! # Security Model
//!
//! The jail setup follows this sequence:
//!
//! ```text
//! 1. Parent process (root or with CAP_SYS_ADMIN)
//!    │
//!    ├── Create cgroup for jail
//!    ├── Set up jail root filesystem
//!    │
//!    └── fork()
//!        │
//!        ├── Parent: Add child to cgroup, wait for exit
//!        │
//!        └── Child:
//!            ├── unshare() - Create new namespaces
//!            ├── Set up UID/GID mapping
//!            ├── Mount /proc, /dev
//!            ├── pivot_root() - Change root filesystem
//!            ├── Apply rlimits
//!            ├── PR_SET_NO_NEW_PRIVS
//!            ├── Drop all capabilities
//!            └── execve() - Run target process
//! ```

mod config;
mod error;

pub use config::{BindMount, JailConfig, NamespaceConfig, ResourceLimits};
pub use error::JailerError;

// Linux implementation
#[cfg(target_os = "linux")]
mod cgroup;
#[cfg(target_os = "linux")]
mod chroot;
#[cfg(target_os = "linux")]
mod jail;
#[cfg(target_os = "linux")]
mod namespace;
#[cfg(target_os = "linux")]
mod rlimit;

#[cfg(target_os = "linux")]
pub use cgroup::{available_controllers, is_cgroup_v2_available, CgroupManager};
#[cfg(target_os = "linux")]
pub use jail::Jail;

// Non-Linux stubs
#[cfg(not(target_os = "linux"))]
mod stubs {
    use super::{JailConfig, JailerError};

    /// A jail (stub for non-Linux).
    pub struct Jail;

    impl Jail {
        /// Create a new jail (stub).
        pub fn new(_config: JailConfig) -> Result<Self, JailerError> {
            Err(JailerError::UnsupportedPlatform)
        }

        /// Run the jail (stub).
        pub fn run(&mut self) -> Result<i32, JailerError> {
            Err(JailerError::UnsupportedPlatform)
        }
    }

    /// Check if cgroup v2 is available (stub).
    pub fn is_cgroup_v2_available() -> bool {
        false
    }
}

#[cfg(not(target_os = "linux"))]
pub use stubs::{is_cgroup_v2_available, Jail};
