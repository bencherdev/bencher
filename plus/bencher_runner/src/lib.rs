//! Bencher Runner - Orchestrates benchmark execution.
//!
//! This crate provides the main runner logic for executing benchmarks
//! either in isolated Firecracker microVMs (sandboxed) or directly
//! on the host system (non-sandboxed). It coordinates:
//!
//! - OCI image pulling and unpacking
//! - Firecracker microVM lifecycle (sandboxed mode)
//! - Direct host execution (non-sandboxed mode)
//! - Result collection via vsock (sandboxed) or stdout/stderr (non-sandboxed)

// Suppress unused crate warnings on non-Linux or without plus
#![cfg_attr(
    any(not(target_os = "linux"), not(feature = "plus")),
    allow(unused_crate_dependencies)
)]

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
