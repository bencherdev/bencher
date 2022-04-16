#![feature(test)]
extern crate test;

mod cli;
pub mod error;
mod tests;

use cli::Bencher;
use error::CliError;

fn main() -> Result<(), CliError> {
    let bencher = Bencher::new()?;
    let benchmark_output = bencher.run()?;
    let report = bencher.convert(benchmark_output)?;
    bencher.output(report)
}
