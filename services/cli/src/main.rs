#![allow(unused_crate_dependencies)]

use std::{io::ErrorKind, process::ExitCode};

use bencher_cli::{CliError, RunError};
use tokio_rustls::rustls::crypto::ring;

#[allow(clippy::print_stderr)]
#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    let crypto_provider = ring::default_provider();
    #[allow(clippy::use_debug)]
    if let Err(err) = crypto_provider.install_default() {
        eprintln!("Failed to install default AWS credentials provider: {err:?}");
        return ExitCode::FAILURE;
    }

    match bencher_cli::exec().await {
        Ok(()) => ExitCode::SUCCESS,
        // https://github.com/rust-lang/rust/issues/46016#issuecomment-1242039016
        Err(CliError::Run(RunError::RunCommand { err, .. }))
            if err.kind() == ErrorKind::BrokenPipe =>
        {
            ExitCode::SUCCESS
        },
        Err(err) => {
            eprintln!("\n{err}");
            ExitCode::FAILURE
        },
    }
}
