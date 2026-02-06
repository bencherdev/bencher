//! Bencher Runner CLI.
//!
//! Usage:
//!   bencher-runner run --image <IMAGE> [OPTIONS]
//!   bencher-runner vmm --jail-root <PATH> --kernel <PATH> --rootfs <PATH> [OPTIONS]

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
