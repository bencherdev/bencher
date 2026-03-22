//! Bencher Runner CLI.
//!
//! Usage:
//!   bencher-runner run --image <IMAGE> [OPTIONS]
//!   bencher-runner vmm --jail-root <PATH> --kernel <PATH> --rootfs <PATH> [OPTIONS]

#![expect(clippy::print_stderr)]

mod error;
mod parser;
mod runner;

fn main() -> std::process::ExitCode {
    if let Err(e) = exec() {
        eprintln!("Error: {e}");
        std::process::ExitCode::FAILURE
    } else {
        std::process::ExitCode::SUCCESS
    }
}

#[cfg(feature = "plus")]
fn exec() -> Result<(), error::RunnerCliError> {
    use rustls::crypto::ring;

    let crypto_provider = ring::default_provider();
    #[expect(clippy::use_debug)]
    if let Err(err) = crypto_provider.install_default() {
        eprintln!("Failed to install default crypto provider: {err:?}");
    }

    let runner = runner::Runner::new()?;
    runner.exec()
}

#[cfg(not(feature = "plus"))]
fn exec() -> Result<(), error::RunnerCliError> {
    Err(error::RunnerCliError::NoPlusFeature)
}
