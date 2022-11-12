mod bencher;
mod cli;
mod error;

use bencher::{sub::SubCmd, Bencher};
pub use error::CliError;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<(), String> {
    exec().await.map_err(|e| format!("{e}"))
}

async fn exec() -> Result<(), CliError> {
    let bencher = Bencher::new()?;
    bencher.exec().await
}
