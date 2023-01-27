use std::convert::{TryFrom, TryInto};

use super::{flag::Flag, output::Output, shell::Shell};
use crate::{cli::project::run::CliRunShell, CliError};

#[derive(Debug)]
pub struct Command {
    shell: Shell,
    flag: Flag,
    cmd: String,
}

impl TryFrom<(CliRunShell, String)> for Command {
    type Error = CliError;

    fn try_from(shell_cmd: (CliRunShell, String)) -> Result<Self, Self::Error> {
        let (shell, cmd) = shell_cmd;
        Ok(Self {
            shell: shell.shell.try_into()?,
            flag: shell.flag.try_into()?,
            cmd,
        })
    }
}

impl TryInto<Output> for &Command {
    type Error = CliError;

    fn try_into(self) -> Result<Output, Self::Error> {
        std::process::Command::new(self.shell.to_string())
            .arg(self.flag.to_string())
            .arg(&self.cmd)
            .output()
            .map(Into::into)
            .map_err(Into::into)
    }
}
