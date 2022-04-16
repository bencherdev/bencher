use std::convert::TryFrom;
use std::process::Command;

mod flag;
mod output;
mod shell;

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

impl TryFrom<(Option<String>, Option<String>, String)> for Benchmark {
    type Error = BencherError;

    fn try_from(cli: (Option<String>, Option<String>, String)) -> Result<Self, Self::Error> {
        let (shell, flag, cmd) = cli;
        Ok(Self {
            shell: Shell::try_from(shell)?,
            flag: Flag::try_from(flag)?,
            cmd,
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
