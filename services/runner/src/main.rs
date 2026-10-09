//! Bencher Runner CLI.
//!
//! Usage:
//!   bencher-runner run --image <IMAGE> [OPTIONS]
//!   bencher-runner vmm --jail-root <PATH> --kernel <PATH> --rootfs <PATH> [OPTIONS]

mod error;
mod parser;
mod runner;

use slog::Logger;

fn main() -> std::process::ExitCode {
    let log = bencher_logger::runner_logger();
    if let Err(e) = exec(&log) {
        slog::error!(log, "Runner failed"; "error" => bencher_logger::capped(e));
        std::process::ExitCode::FAILURE
    } else {
        std::process::ExitCode::SUCCESS
    }
}

#[cfg(feature = "plus")]
fn exec(log: &Logger) -> Result<(), error::RunnerCliError> {
    use rustls::crypto::aws_lc_rs;

    let crypto_provider = aws_lc_rs::default_provider();
    if let Err(err) = crypto_provider.install_default() {
        return Err(error::RunnerCliError::CryptoProvider(format!("{err:?}")));
    }

    let runner = runner::Runner::new()?;
    runner.exec(log)
}

#[cfg(not(feature = "plus"))]
fn exec(_log: &Logger) -> Result<(), error::RunnerCliError> {
    Err(error::RunnerCliError::NoPlusFeature)
}
