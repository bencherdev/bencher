#![cfg_attr(feature = "plus", expect(clippy::expect_used, clippy::print_stdout))]
#![cfg_attr(not(feature = "plus"), allow(unused_crate_dependencies))]

#[cfg(feature = "plus")]
mod parser;
#[cfg(feature = "plus")]
mod task;

fn main() -> anyhow::Result<()> {
    exec()
}

#[cfg(feature = "plus")]
fn exec() -> anyhow::Result<()> {
    let task = task::Task::new()?;
    task.exec()
}

#[cfg(not(feature = "plus"))]
fn exec() -> anyhow::Result<()> {
    anyhow::bail!("Plus feature required")
}
