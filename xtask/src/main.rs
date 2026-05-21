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
    let task = Task::new()?;
    task.exec().await
}
