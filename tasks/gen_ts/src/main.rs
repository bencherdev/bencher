#![allow(clippy::print_stdout)]

mod parser;
mod task;

use task::Task;

fn main() -> anyhow::Result<()> {
    exec()
}

fn exec() -> anyhow::Result<()> {
    let task = Task::new()?;
    task.exec()
}
