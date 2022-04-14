#![feature(test)]

use std::process::Command;

extern crate test;

use clap::Parser;

mod adapter;
mod args;
mod error;
mod output;
mod report;

use crate::adapter::Adapter;
use crate::args::Args;
use crate::error::CliError;
use crate::output::Output;

const UNIX_SHELL: &str = "/bin/sh";
const WINDOWS_SHELL: &str = "cmd";

const UNIX_FLAG: &str = "-c";
const WINDOWS_FLAG: &str = "/C";

fn main() -> Result<(), CliError> {
    let args = Args::parse();

    let shell = args.shell()?;
    let flag = args.flag()?;

    let output = Command::new(&shell).arg(&flag).arg(args.cmd()).output();

    let output = if let Ok(output) = output {
        Output::try_from(output)?
    } else {
        return Err(CliError::Benchmark(args.cmd().into()));
    };

    let report = match args.adapter() {
        Adapter::Rust => adapter::rust::parse(output),
        Adapter::Custom(adapter) => adapter::custom::parse(adapter, output),
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
