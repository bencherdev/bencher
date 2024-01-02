use std::process::ExitCode;

use bencher_cli::{CliError, RunError};

#[allow(clippy::print_stderr)]
#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    match bencher_cli::exec().await {
        Ok(()) => ExitCode::SUCCESS,
        // https://github.com/rust-lang/rust/issues/46016#issuecomment-1242039016
        Err(CliError::Run(RunError::RunCommand(err)))
            if err.kind() == std::io::ErrorKind::BrokenPipe =>
        {
            ExitCode::SUCCESS
        },
        Err(err) => {
            eprintln!("\n{err}");
            ExitCode::FAILURE
        },
    }
}
