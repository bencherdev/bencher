#![feature(test)]

use std::process::Command;

extern crate test;

use clap::{ArgEnum, Parser};

mod error;
mod output;
mod report;
mod tool;

use crate::error::CliError;
use crate::output::Output;

const UNIX_SHELL: &str = "/bin/sh";
const WINDOWS_SHELL: &str = "cmd";

const UNIX_FLAG: &str = "-c";
const WINDOWS_FLAG: &str = "/C";

/// Time Series Benchmarking
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
struct Args {
    /// Shell command path
    #[clap(short, long)]
    shell: Option<String>,

    /// Shell command flag
    #[clap(short, long)]
    flag: Option<String>,

    /// Benchmark command to execute
    #[clap(short = 'x', long = "exec")]
    pub cmd: String,

    /// Benchmark tool ID
    #[clap(short, long, arg_enum, default_value = "rust")]
    tool: Tool,
}

/// Supported Languages
#[derive(ArgEnum, Clone, Debug)]
enum Tool {
    /// Rust 🦀
    #[clap(name = "rust")]
    Rust,
    // Custom(String),
}

fn main() -> Result<(), CliError> {
    let args = Args::parse();

    println!("CMD: {}", args.cmd);

    let shell = if let Some(shell) = args.shell {
        shell
    } else if cfg!(target_family = "unix") {
        UNIX_SHELL.into()
    } else if cfg!(target_family = "windows") {
        WINDOWS_SHELL.into()
    } else {
        return Err(CliError::Shell);
    };

    let flag = if let Some(flag) = args.flag {
        flag
    } else if cfg!(target_family = "unix") {
        UNIX_FLAG.into()
    } else if cfg!(target_family = "windows") {
        WINDOWS_FLAG.into()
    } else {
        return Err(CliError::Flag);
    };

    let output = Command::new(shell).arg(flag).arg(&args.cmd).output();

    let output = if let Ok(output) = output {
        Output::try_from(output)?
    } else {
        return Err(CliError::Benchmark(args.cmd));
    };

    let report = match args.tool {
        Tool::Rust => tool::rust_bench::parse(output),
        // Tool::Custom(_) => todo!(),
    }?;

    // TODO this should be the JSON value
    println!("{report:?}");

    Ok(())
}

#[cfg(test)]
mod tests {
    use test::{black_box, Bencher};

    #[test]
    fn ignored() {
        assert!(true);
    }

    #[bench]
    fn benchmark(b: &mut Bencher) {
        let x: f64 = 211.0 * 11.0;
        let y: f64 = 301.0 * 103.0;

        b.iter(|| {
            // Inner closure, the actual test
            for _ in 1..10000 {
                black_box(x.powf(y).powf(x));
            }
        });
    }
}
