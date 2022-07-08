use std::convert::TryFrom;
use std::convert::TryInto;

use super::{flag::Flag, shell::Shell};
use crate::{cli::clap::CliShell, BencherError};

#[derive(Debug)]
pub struct Command {
    shell: Shell,
    flag: Flag,
    cmd: String,
}

impl TryFrom<(CliShell, String)> for Command {
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

impl TryInto<String> for &Command {
    type Error = BencherError;

    fn try_into(self) -> Result<String, Self::Error> {
        let output = std::process::Command::new(self.shell.to_string())
            .arg(self.flag.to_string())
            .arg(&self.cmd)
            .output()?;

        Ok(format!(
            "{}{}",
            String::from_utf8(output.stdout)?,
            String::from_utf8(output.stderr)?
        ))
    }
}
