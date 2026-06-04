#![expect(clippy::print_stdout, clippy::use_debug, reason = "CLI task tool")]

mod parser;
mod task;

use task::Task;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    let _env = dotenvy::from_path("xtask/.env");
    exec().await
}

async fn exec() -> anyhow::Result<()> {
    #[cfg(feature = "plus")]
    {
        use rustls::crypto::aws_lc_rs;

        aws_lc_rs::default_provider()
            .install_default()
            .map_err(|_err| anyhow::anyhow!("Failed to install default TLS crypto provider"))?;
    }

    let task = Task::new()?;
    task.exec().await
}
