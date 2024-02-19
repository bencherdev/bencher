use std::fmt;

use camino::Utf8PathBuf;

use crate::parser::project::run::CliRunCommand;

pub mod command;
mod flag;
pub mod output;
mod pipe;
mod shell;

use command::Command;
use output::Output;
use pipe::Pipe;

use super::{RunError, BENCHER_CMD};

#[derive(Debug, Clone)]
pub enum Runner {
    Pipe(Pipe),
    Command(Command),
    CommandToFile(Command, Utf8PathBuf),
    File(Utf8PathBuf),
}

impl TryFrom<CliRunCommand> for Runner {
    type Error = RunError;

    fn try_from(cmd: CliRunCommand) -> Result<Self, Self::Error> {
        let program_arguments = cmd.command.and_then(|c| {
            let mut c = c.into_iter();
            c.next().map(|program| (program, c.collect::<Vec<_>>()))
        });
        if let Some((program, arguments)) = program_arguments {
            let command = if !cmd.exec && arguments.is_empty() {
                Command::new_shell(cmd.sh_c, program)?
            } else {
                if let Some(shell) = cmd.sh_c.shell {
                    return Err(RunError::ShellWithExec(shell));
                } else if let Some(flag) = cmd.sh_c.flag {
                    return Err(RunError::FlagWithExec(flag));
                }
                Command::new_exec(program, arguments)
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

impl fmt::Display for Runner {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Pipe(pipe) => write!(f, "{pipe}"),
            Self::Command(command) => write!(f, "{command}"),
            Self::CommandToFile(command, file_path) => {
                write!(f, "{command} > {file_path}")
            },
            Self::File(file_path) => write!(f, "{file_path}"),
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
