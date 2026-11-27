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

use rustls::crypto::aws_lc_rs;
use task::Task;

const API_VERSION: &str = env!("CARGO_PKG_VERSION");

fn main() -> anyhow::Result<()> {
    let _provider = aws_lc_rs::default_provider().install_default();
    exec()
}

fn exec() -> anyhow::Result<()> {
    let task = Task::new()?;
    task.exec()
}
