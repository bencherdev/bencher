#![cfg_attr(
    feature = "plus",
    expect(clippy::print_stdout, reason = "CLI task output")
)]
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
    use rustls::crypto::aws_lc_rs;

    aws_lc_rs::default_provider()
        .install_default()
        .map_err(|_err| anyhow::anyhow!("Failed to install the default TLS crypto provider"))?;

    let task = task::Task::new()?;
    task.exec()
}

#[cfg(not(feature = "plus"))]
fn exec() -> anyhow::Result<()> {
    anyhow::bail!("Plus feature required")
}
