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

    fn try_from(command: CliRunCommand) -> Result<Self, Self::Error> {
        let cmd_str = command.cmd.or_else(|| std::env::var(BENCHER_CMD).ok());
        if let Some(cmd_str) = cmd_str {
            let cmd = Command::try_from((command.shell, cmd_str))?;
            Ok(if let Some(file) = command.file {
                Self::CommandToFile(cmd, file)
            } else {
                Self::Command(cmd)
            })
        } else if let Some(file) = command.file {
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
