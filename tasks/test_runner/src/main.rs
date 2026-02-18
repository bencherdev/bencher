#![expect(clippy::expect_used, clippy::print_stdout)]
// These crates are used behind #[cfg(feature = "plus")] in the task modules.
#![cfg_attr(
    not(feature = "plus"),
    allow(unused_crate_dependencies, unused_imports)
)]

mod docker;
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
