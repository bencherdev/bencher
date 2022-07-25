#![feature(derive_default_enum)]
#![feature(test)]
extern crate test;

mod cli;
mod cmd;
mod error;
mod tests;

use cli::Bencher;
pub use error::BencherError;

#[tokio::main]
async fn main() -> Result<(), String> {
    exec().await.map_err(|e| format!("{e}"))
}

async fn exec() -> Result<(), BencherError> {
    let bencher = Bencher::new()?;
    bencher.exec().await
}
