use std::{
    convert::{TryFrom, TryInto},
    fs::File,
    io::{BufRead, BufReader},
    path::PathBuf,
};

use crate::{parser::project::run::CliRunCommand, CliError};

mod command;
mod flag;
pub mod output;
mod pipe;
mod shell;

use command::Command;
use output::Output;
use pipe::Pipe;

use super::BENCHER_CMD;

#[derive(Debug)]
pub enum Runner {
    Pipe(Pipe),
    Command(Command),
    CommandToFile(Command, PathBuf),
}

impl TryFrom<CliRunCommand> for Runner {
    type Error = CliError;

    fn try_from(mut command: CliRunCommand) -> Result<Self, Self::Error> {
        if let Some(cmd) = command.cmd.take() {
            (command, cmd).try_into()
        } else if let Ok(cmd) = std::env::var(BENCHER_CMD) {
            (command, cmd).try_into()
        } else if let Some(pipe) = Pipe::new() {
            Ok(Self::Pipe(pipe))
        } else {
            Err(CliError::NoCommand)
        }
    }
}

impl TryFrom<(CliRunCommand, String)> for Runner {
    type Error = CliError;

    fn try_from((command, cmd): (CliRunCommand, String)) -> Result<Self, Self::Error> {
        let cmd = Command::try_from((command.shell, cmd))?;
        Ok(if let Some(file) = command.file {
            Self::CommandToFile(cmd, file)
        } else {
            Self::Command(cmd)
        })
    }
}

impl Runner {
    pub fn run(&self) -> Result<Output, CliError> {
        Ok(match self {
            Self::Pipe(pipe) => pipe.output(),
            Self::Command(command) => command.try_into()?,
            Self::CommandToFile(command, file_path) => {
                let mut output: Output = command.try_into()?;
                let capacity = usize::try_from(std::fs::metadata(file_path)?.len())?;
                output.stdout = String::with_capacity(capacity);

                let output_file = File::open(file_path)?;
                let buffered = BufReader::new(output_file);
                for line in buffered.lines() {
                    output.stdout.push_str(&line?);
                }

                output
            },
        })
    }
}
