#![feature(derive_default_enum)]
#![feature(test)]
extern crate test;

mod cli;
mod error;
mod tests;

use cli::Bencher;
pub use error::BencherError;

#[tokio::main]
async fn main() -> Result<(), BencherError> {
    let bencher = Bencher::new()?;
    let benchmark_output = bencher.run()?;
    let report = bencher.convert(benchmark_output)?;
    bencher.send(report).await
}
