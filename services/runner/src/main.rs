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

fn main() -> std::process::ExitCode {
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
