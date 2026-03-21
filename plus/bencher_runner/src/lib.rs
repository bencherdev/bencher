//! Bencher Runner - Orchestrates benchmark execution in Firecracker microVMs.
//!
//! This crate provides the main runner logic for executing benchmarks
//! in isolated Firecracker microVMs. It coordinates:
//!
//! - OCI image pulling and unpacking
//! - ext4 rootfs creation
//! - Firecracker microVM lifecycle management
//! - Result collection via vsock

// Suppress unused crate warnings on non-Linux or without plus
#![cfg_attr(
    any(not(target_os = "linux"), not(feature = "plus")),
    allow(unused_crate_dependencies)
)]

// tempfile is used in the execute function (Linux VM and non-sandboxed)
#[cfg(feature = "plus")]
use tempfile as _;

#[cfg(feature = "plus")]
mod config;
#[cfg(feature = "plus")]
pub mod cpu;
#[cfg(feature = "plus")]
mod error;
#[cfg(all(feature = "plus", target_os = "linux"))]
pub mod firecracker;
#[cfg(all(feature = "plus", target_os = "linux"))]
pub mod firecracker_bin;
#[cfg(all(feature = "plus", target_os = "linux"))]
pub mod init;
#[cfg(feature = "plus")]
pub mod jail;
#[cfg(all(feature = "plus", target_os = "linux"))]
pub mod kernel;
#[cfg(feature = "plus")]
mod local;
#[cfg(feature = "plus")]
mod log_level;
#[cfg(feature = "plus")]
pub mod metrics;
#[cfg(feature = "plus")]
mod run;
#[cfg(feature = "plus")]
pub mod tuning;
#[cfg(feature = "plus")]
pub mod units;
#[cfg(feature = "plus")]
pub mod up;
#[cfg(all(feature = "plus", target_os = "linux"))]
mod vm;

#[cfg(feature = "plus")]
pub use bencher_json::{Cpu, Disk, GracePeriod, Memory};
#[cfg(feature = "plus")]
pub use config::Config;
#[cfg(feature = "plus")]
pub use error::{ConfigError, JailError, RunnerError};
#[cfg(feature = "plus")]
pub use jail::ResourceLimits;
#[cfg(feature = "plus")]
pub use log_level::FirecrackerLogLevel;
#[cfg(feature = "plus")]
pub use run::{RunArgs, RunOutput, execute, resolve_oci_image, run_with_args};
#[cfg(feature = "plus")]
pub use tuning::{PerfEventParanoid, Swappiness, TuningConfig};
