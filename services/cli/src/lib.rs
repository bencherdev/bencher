mod bencher;
mod error;
mod parser;

use bencher::{sub::SubCmd, Bencher};
pub use bencher::{
    sub::{MockError, RunError, ThresholdError},
    BackendError,
};
pub use error::CliError;
pub use parser::CliBencher;

pub const CLI_VERSION: &str = env!("CARGO_PKG_VERSION");

pub async fn exec() -> Result<(), CliError> {
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
    });
}

pub(crate) use cli_println;

macro_rules! cli_println_quietable {
    ($log:expr, $fmt:expr) => ({
        if $log {
            crate::cli_println!($fmt);
        }
    });
    ($log:expr, $fmt:expr, $($arg:tt)*) => ({
        if $log {
            crate::cli_println!($fmt, $($arg)*);
        }
    });
}

pub(crate) use cli_println_quietable;

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

macro_rules! cli_eprintln_quietable {
    ($log:expr, $fmt:expr) => ({
        if $log {
            crate::cli_eprintln!($fmt);
        }
    });
    ($log:expr, $fmt:expr, $($arg:tt)*) => ({
        if $log {
            crate::cli_eprintln!($fmt, $($arg)*);
        }
    });
}

pub(crate) use cli_eprintln_quietable;
