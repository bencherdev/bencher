use std::convert::TryFrom;
use std::process::Command;

mod flag;
mod output;
mod shell;

use crate::cli::clap::CliBenchmark;
use crate::BencherError;
pub use flag::Flag;
pub use output::Output;
pub use shell::Shell;

#[derive(Debug)]
pub struct Benchmark {
    shell: Shell,
    flag: Flag,
    cmd: String,
}

impl TryFrom<CliBenchmark> for Benchmark {
    type Error = BencherError;

    fn try_from(benchmark: CliBenchmark) -> Result<Self, Self::Error> {
        Ok(Self {
            shell: Shell::try_from(benchmark.shell)?,
            flag: Flag::try_from(benchmark.flag)?,
            cmd: benchmark.cmd,
        })
    }
}

impl Benchmark {
    pub fn run(&self) -> Result<Output, BencherError> {
        let output = Command::new(self.shell.to_string())
            .arg(self.flag.to_string())
            .arg(&self.cmd)
            .output()?;

        Output::try_from(output)
    }
}
