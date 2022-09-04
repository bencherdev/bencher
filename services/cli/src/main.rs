// #![feature(test)]
// extern crate test;

mod bencher;
mod cli;
mod error;
// mod tests;

use bencher::Bencher;
pub use error::BencherError;

#[tokio::main]
async fn main() -> Result<(), String> {
    exec().await.map_err(|e| format!("{e}"))
}

async fn exec() -> Result<(), BencherError> {
    let bencher = Bencher::new()?;
    bencher.exec().await
}
