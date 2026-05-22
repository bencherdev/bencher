#![expect(clippy::print_stdout, reason = "CLI tool outputs to stdout")]

mod parser;
mod task;

use task::Task;

fn main() -> anyhow::Result<()> {
    let task = Task::new()?;
    task.exec()
}
