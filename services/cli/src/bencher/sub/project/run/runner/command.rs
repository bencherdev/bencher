use std::convert::{TryFrom, TryInto};

use super::{flag::Flag, shell::Shell};
use crate::{parser::project::run::CliRunShell, CliError};

#[derive(Debug)]
pub struct Command {
    pub shell: Shell,
    pub flag: Flag,
    pub cmd: String,
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
