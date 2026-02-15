#![cfg(feature = "plus")]

//! Bencher Runner - Orchestrates benchmark execution in Firecracker microVMs.
//!
//! This crate provides the main runner logic for executing benchmarks
//! in isolated Firecracker microVMs. It coordinates:
//!
//! - OCI image pulling and unpacking
//! - ext4 rootfs creation
//! - Firecracker microVM lifecycle management
//! - Result collection via vsock

// Suppress unused crate warnings on non-Linux
#![cfg_attr(not(target_os = "linux"), allow(unused_crate_dependencies))]

// tempfile is only used on Linux in the execute function
#[cfg(target_os = "linux")]
use tempfile as _;

mod config;
pub mod cpu;
pub mod daemon;
mod error;
#[cfg(target_os = "linux")]
pub mod firecracker;
#[cfg(target_os = "linux")]
pub mod firecracker_bin;
#[cfg(target_os = "linux")]
pub mod init;
pub mod jail;
#[cfg(target_os = "linux")]
pub mod kernel;
pub mod metrics;
mod run;
pub mod tuning;
pub mod units;

pub use bencher_json::{Cpu, Disk, Memory};
pub use config::Config;
pub use error::RunnerError;
pub use jail::ResourceLimits;
pub use run::{RunArgs, RunOutput, execute, resolve_oci_image, run_with_args};
pub use tuning::TuningConfig;
