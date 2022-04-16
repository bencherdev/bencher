#![feature(test)]
extern crate test;

mod cli;
mod tests;

use cli::benchmark::Benchmark;
use cli::error::CliError;

fn main() -> Result<(), CliError> {
    let bench = Benchmark::new()?;
    let output = bench.run()?;
    let report = bench.convert(output)?;
    bench.output(report)
}
