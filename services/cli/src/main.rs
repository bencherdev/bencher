use std::{io, process::ExitCode};

mod bencher;
mod cli;
mod error;

use bencher::{sub::SubCmd, Bencher};
pub use error::CliError;

#[allow(clippy::print_stderr)]
#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    match exec().await {
        Ok(_) => ExitCode::SUCCESS,
        // https://github.com/rust-lang/rust/issues/46016#issuecomment-1242039016
        Err(CliError::Io(err)) if err.kind() == io::ErrorKind::BrokenPipe => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("{err}");
            ExitCode::FAILURE
        },
    }
}

async fn exec() -> Result<(), CliError> {
    let bencher = Bencher::new()?;
    bencher.exec().await
}

// https://github.com/rust-lang/rust/issues/46016#issuecomment-1242039016
// These macros are needed because the normal ones panic when there's a broken pipe.
// This is especially problematic for CLI tools that are frequently piped into `head` or `grep -q`
macro_rules! cli_println {
    () => (print!("\n"));
    ($fmt:expr) => ({
        use std::io::Write;
        let _w = writeln!(std::io::stdout(), $fmt);
    });
    ($fmt:expr, $($arg:tt)*) => ({
        use std::io::Write;
        let _w = writeln!(std::io::stdout(), $fmt, $($arg)*);
    })
}

pub(crate) use cli_println;
