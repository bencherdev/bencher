use std::{io, process::ExitCode};

mod bencher;
mod error;
mod parser;

use bencher::{
    sub::{RunError, SubCmd},
    Bencher,
};
pub use error::CliError;

#[allow(clippy::print_stderr)]
#[tokio::main(flavor = "current_thread")]
async fn main() -> ExitCode {
    match exec().await {
        Ok(()) => ExitCode::SUCCESS,
        // https://github.com/rust-lang/rust/issues/46016#issuecomment-1242039016
        Err(CliError::Run(RunError::RunCommand(err)))
            if err.kind() == io::ErrorKind::BrokenPipe =>
        {
            ExitCode::SUCCESS
        },
        Err(err) => {
            eprintln!("\n{err}");
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
    // () => (print!("\n"));
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

macro_rules! cli_eprintln {
    // () => (eprint!("\n"));
    ($fmt:expr) => ({
        use std::io::Write;
        let _w = writeln!(std::io::stderr(), $fmt);
    });
    ($fmt:expr, $($arg:tt)*) => ({
        use std::io::Write;
        let _w = writeln!(std::io::stderr(), $fmt, $($arg)*);
    })
}

pub(crate) use cli_eprintln;
