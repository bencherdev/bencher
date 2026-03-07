//! Bencher Runner CLI.
//!
//! Usage:
//!   bencher-runner run --image <IMAGE> [OPTIONS]
//!   bencher-runner vmm --jail-root <PATH> --kernel <PATH> --rootfs <PATH> [OPTIONS]

// Suppress unused-crate warnings for deps only used behind `plus`.
#![cfg_attr(not(feature = "plus"), allow(unused_crate_dependencies))]
#![expect(clippy::print_stderr)]

mod error;
mod parser;
mod runner;

use runner::Runner;
use rustls::crypto::ring;

fn main() -> std::process::ExitCode {
    let crypto_provider = ring::default_provider();
    #[expect(clippy::use_debug)]
    if let Err(err) = crypto_provider.install_default() {
        eprintln!("Failed to install default crypto provider: {err:?}");
        return std::process::ExitCode::FAILURE;
    }
    if let Err(e) = exec() {
        eprintln!("Error: {e}");
        std::process::ExitCode::FAILURE
    } else {
        std::process::ExitCode::SUCCESS
    }
}

fn exec() -> Result<(), error::RunnerCliError> {
    let runner = Runner::new()?;
    runner.exec()
}
