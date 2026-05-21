#![expect(
    clippy::expect_used,
    clippy::print_stderr,
    clippy::print_stdout,
    clippy::unwrap_used,
    clippy::unwrap_in_result,
    clippy::use_debug,
    reason = "test harness binary"
)]

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
