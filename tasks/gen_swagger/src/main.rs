#![allow(clippy::print_stdout)]

mod parser;
mod task;

use task::Task;

const API_VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> anyhow::Result<()> {
    exec()
}

fn exec() -> anyhow::Result<()> {
    let task = Task::new()?;
    task.exec()
}
