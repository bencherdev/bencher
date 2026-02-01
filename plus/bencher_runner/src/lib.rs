#![cfg(feature = "plus")]

//! Bencher Runner - Orchestrates benchmark execution in VMs.
//!
//! This crate provides the main runner logic for executing benchmarks
//! in isolated VMs. It coordinates:
//!
//! - OCI image unpacking
//! - Squashfs rootfs creation
//! - VM lifecycle management
//! - Result collection

// tempfile is only used on Linux in the execute function, but we import it
// unconditionally to satisfy the unused-crate-dependencies lint
use tempfile as _;

mod config;
mod error;
mod run;

pub use config::Config;
pub use error::RunnerError;
pub use run::{execute, resolve_oci_image, run};
