#![expect(clippy::print_stdout, reason = "CLI tool outputs to stdout")]

mod parser;
mod task;

use task::Task;

fn main() -> anyhow::Result<()> {
    use rustls::crypto::aws_lc_rs;

    aws_lc_rs::default_provider()
        .install_default()
        .map_err(|_err| anyhow::anyhow!("Failed to install default TLS crypto provider"))?;

    let task = Task::new()?;
    task.exec()
}
