#![feature(test)]
extern crate test;

mod cli;
mod error;
mod tests;

use cli::Bencher;
pub use error::BencherError;

fn main() -> Result<(), BencherError> {
    let bencher = Bencher::new()?;
    let benchmark_output = bencher.run()?;
    let report = bencher.convert(benchmark_output)?;
    bencher.output(report)
}
