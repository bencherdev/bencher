#![expect(clippy::print_stdout)]

mod parser;
mod task;

use task::Task;

fn main() -> anyhow::Result<()> {
    let task = Task::new()?;
    task.exec()
}
