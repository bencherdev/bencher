#![feature(derive_default_enum)]
#![feature(test)]
extern crate test;

mod cli;
mod error;
mod tests;

use cli::Bencher;
pub use error::BencherError;

#[tokio::main]
async fn main() -> Result<(), String> {
    run().await.map_err(|e| format!("{e}"))
}

async fn run() -> Result<(), BencherError> {
    let bencher = Bencher::new()?;
    let benchmark_output = bencher.run()?;
    let metrics = bencher.convert(benchmark_output)?;
    bencher.send(metrics).await
}
