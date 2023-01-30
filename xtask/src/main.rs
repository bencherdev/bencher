use anyhow::Result;

mod cli;
mod task;

use task::Task;

#[tokio::main(flavor = "current_thread")]
async fn main() -> Result<()> {
    exec().await
}

async fn exec() -> Result<()> {
    let task = Task::new()?;
    task.exec().await
}
