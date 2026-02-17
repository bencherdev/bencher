//! Bencher Runner CLI.
//!
//! Usage:
//!   bencher-runner run --image <IMAGE> [OPTIONS]
//!   bencher-runner vmm --jail-root <PATH> --kernel <PATH> --rootfs <PATH> [OPTIONS]

// Suppress unused-crate warnings for deps only used behind `plus`.
#![cfg_attr(not(feature = "plus"), allow(unused_crate_dependencies))]

mod parser;
mod runner;

use runner::Runner;

fn main() -> anyhow::Result<()> {
    exec()
}

fn exec() -> anyhow::Result<()> {
    let runner = Runner::new()?;
    runner.exec()
}
