#![expect(clippy::print_stdout, reason = "CLI task output")]

mod parser;
mod task;

use task::Task;

const API_VERSION: &str = env!("CARGO_PKG_VERSION");

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    exec().await
}

async fn exec() -> anyhow::Result<()> {
    use rustls::crypto::aws_lc_rs;

    aws_lc_rs::default_provider()
        .install_default()
        .map_err(|_err| anyhow::anyhow!("Failed to install default TLS crypto provider"))?;

    let task = Task::new()?;
    task.exec().await
}
