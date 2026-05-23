//! Bencher Runner CLI.
//!
//! Usage:
//!   bencher-runner run --image <IMAGE> [OPTIONS]
//!   bencher-runner vmm --jail-root <PATH> --kernel <PATH> --rootfs <PATH> [OPTIONS]

#![expect(clippy::print_stderr, reason = "runner CLI error output")]

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
    use rustls::crypto::aws_lc_rs;

    let crypto_provider = aws_lc_rs::default_provider();
    if let Err(err) = crypto_provider.install_default() {
        return Err(error::RunnerCliError::CryptoProvider(format!("{err:?}")));
    }

    let runner = runner::Runner::new()?;
    runner.exec()
}

#[cfg(not(feature = "plus"))]
fn exec() -> Result<(), error::RunnerCliError> {
    Err(error::RunnerCliError::NoPlusFeature)
}
