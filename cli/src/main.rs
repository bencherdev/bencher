#![feature(test)]

extern crate test;

mod adapter;
mod args;
mod benchmark;
mod error;
mod save;
mod tests;

use crate::benchmark::Benchmark;
use crate::error::CliError;

fn main() -> Result<(), CliError> {
    let bench = Benchmark::new()?;
    let output = bench.run()?;
    let report = bench.convert(output)?;

    println!("{report:?}");

    bench.save(report)?;

    Ok(())
}
