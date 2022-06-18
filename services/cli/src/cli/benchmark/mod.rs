use std::convert::TryFrom;
use std::process::Command;

mod flag;
mod output;
mod shell;

use crate::cli::clap::CliShell;
use crate::BencherError;
pub use flag::Flag;
pub use output::Output as BenchmarkOutput;
pub use shell::Shell;

#[derive(Debug)]
pub struct Benchmark {
    shell: Shell,
    flag: Flag,
    cmd: String,
}

impl TryFrom<(CliShell, String)> for Benchmark {
    type Error = BencherError;

    fn try_from(shell_cmd: (CliShell, String)) -> Result<Self, Self::Error> {
        let (shell, cmd) = shell_cmd;
        Ok(Self {
            shell: Shell::try_from(shell.shell)?,
            flag: Flag::try_from(shell.flag)?,
            cmd,
        })
    }
}

impl Benchmark {
    pub fn run(&self) -> Result<BenchmarkOutput, BencherError> {
        let output = Command::new(self.shell.to_string())
            .arg(self.flag.to_string())
            .arg(&self.cmd)
            .output()?;

        BenchmarkOutput::try_from(output)
    }
}
