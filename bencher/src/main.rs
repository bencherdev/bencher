#![feature(test)]
extern crate test;

mod cli;
pub mod error;
mod tests;

use cli::benchmark::Bencher;
use error::CliError;

fn main() -> Result<(), CliError> {
    let bencher = Bencher::new()?;
    let output = bencher.run()?;
    let report = bencher.convert(output)?;
    bencher.output(report)
}
