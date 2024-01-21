use std::{convert::TryFrom, path::PathBuf};

use crate::parser::project::run::CliRunCommand;

mod command;
mod flag;
pub mod output;
mod pipe;
mod shell;

use command::Command;
use output::Output;
use pipe::Pipe;

use super::{RunError, BENCHER_CMD};

#[derive(Debug)]
pub enum Runner {
    Pipe(Pipe),
    Command(Command),
    CommandToFile(Command, PathBuf),
    File(PathBuf),
}

impl TryFrom<CliRunCommand> for Runner {
    type Error = RunError;

    fn try_from(cmd: CliRunCommand) -> Result<Self, Self::Error> {
        let var_arg = if let Some(command) = cmd.command {
            let mut var_arg = command.into_iter();
            var_arg
                .next()
                .map(|command| (command, var_arg.collect::<Vec<_>>()))
        } else {
            None
        };
        if let Some((command, arguments)) = var_arg {
            let command = if !cmd.exec && arguments.is_empty() {
                Command::new_shell(cmd.sh_c, command)?
            } else {
                if let Some(shell) = cmd.sh_c.shell {
                    return Err(RunError::ShellWithExec(shell));
                } else if let Some(flag) = cmd.sh_c.flag {
                    return Err(RunError::FlagWithExec(flag));
                }
                Command::new_exec(command, arguments)
            };
            Ok(if let Some(file) = cmd.file {
                Self::CommandToFile(command, file)
            } else {
                Self::Command(command)
            })
        } else if let Ok(command) = std::env::var(BENCHER_CMD) {
            let command = Command::new_shell(cmd.sh_c, command)?;
            Ok(if let Some(file) = cmd.file {
                Self::CommandToFile(command, file)
            } else {
                Self::Command(command)
            })
        } else if let Some(file) = cmd.file {
            Ok(Self::File(file))
        } else if let Some(pipe) = Pipe::new() {
            Ok(Self::Pipe(pipe))
        } else {
            Err(RunError::NoCommand)
        }
    }
}

impl Runner {
    pub async fn run(&self, log: bool) -> Result<Output, RunError> {
        Ok(match self {
            Self::Pipe(pipe) => pipe.output(),
            Self::Command(command) => command.run(log).await?,
            Self::CommandToFile(command, file_path) => {
                let mut output = command.run(log).await?;
                let result =
                    std::fs::read_to_string(file_path).map_err(RunError::OutputFileRead)?;
                output.result = Some(result);
                output
            },
            Self::File(file_path) => {
                let result =
                    std::fs::read_to_string(file_path).map_err(RunError::OutputFileRead)?;
                Output {
                    result: Some(result),
                    ..Default::default()
                }
            },
        })
    }
}
