#![expect(
    clippy::expect_used,
    clippy::print_stdout,
    clippy::print_stderr,
    clippy::unused_self,
    clippy::unwrap_used,
    clippy::use_debug
)]

mod parser;
mod task;

use task::Task;

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    #[expect(let_underscore_drop, reason = "Optional dotenv file")]
    let _ = dotenvy::from_path("xtask/.env");
    exec().await
}

async fn exec() -> anyhow::Result<()> {
    let task = Task::new()?;
    task.exec().await
}
