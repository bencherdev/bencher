#![cfg(feature = "plus")]

//! Bencher Runner - Orchestrates benchmark execution in isolated VMs.
//!
//! This crate provides the main runner logic for executing benchmarks
//! in isolated VMs. It coordinates:
//!
//! - OCI image pulling and unpacking
//! - Squashfs rootfs creation
//! - Jail setup (namespaces, cgroups, `pivot_root`)
//! - VM lifecycle management via KVM
//! - Result collection via vsock
//!
//! # Architecture
//!
//! The runner operates in two modes via subcommands:
//!
//! ## `run` subcommand (orchestration)
//! - Pulls OCI image from registry
//! - Creates squashfs rootfs
//! - Prepares jail root with bind mounts
//! - Execs to `vmm` subcommand
//!
//! ## `vmm` subcommand (execution)
//! - Enters new namespaces (user, mount, network, etc.)
//! - Performs `pivot_root` to isolated filesystem
//! - Applies seccomp and drops capabilities
//! - Runs the VMM to boot the guest
//! - Collects results via vsock

// Suppress unused crate warnings on non-Linux
#![cfg_attr(not(target_os = "linux"), allow(unused_crate_dependencies))]

// tempfile is only used on Linux in the execute function
#[cfg(target_os = "linux")]
use tempfile as _;

mod config;
mod error;
#[cfg(target_os = "linux")]
pub mod init;
pub mod jail;
pub mod metrics;
mod run;
#[cfg(target_os = "linux")]
pub mod vmm;

pub use config::Config;
pub use error::RunnerError;
pub use jail::ResourceLimits;
pub use run::{execute, resolve_oci_image, run, run_with_args, RunArgs};

#[cfg(target_os = "linux")]
pub use vmm::run_vmm;
