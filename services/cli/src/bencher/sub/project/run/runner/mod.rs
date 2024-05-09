use std::fmt;

use crate::parser::project::run::CliRunCommand;

pub mod command;
mod file_path;
mod file_size;
mod flag;
pub mod output;
mod pipe;
mod shell;

use command::Command;
use file_path::FilePath;
use file_size::FileSize;
use output::Output;
use pipe::Pipe;

use super::{RunError, BENCHER_CMD};

#[derive(Debug, Clone)]
pub enum Runner {
    Pipe(Pipe),
    Command(Command),
    CommandToFile(Command, FilePath),
    CommandToFileSize(Command, FileSize),
    File(FilePath),
    FileSize(FileSize),
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
                Self::CommandToFile(command, FilePath::new(file))
            } else if let Some(file) = cmd.file_size {
                Self::CommandToFileSize(command, FileSize::new(file))
            } else {
                Self::Command(command)
            })
        } else if let Ok(command) = std::env::var(BENCHER_CMD) {
            let command = Command::new_shell(cmd.sh_c, command)?;
            Ok(if let Some(file) = cmd.file {
                Self::CommandToFile(command, FilePath::new(file))
            } else if let Some(file) = cmd.file_size {
                Self::CommandToFileSize(command, FileSize::new(file))
            } else {
                Self::Command(command)
            })
        } else if let Some(file) = cmd.file {
            Ok(Self::File(FilePath::new(file)))
        } else if let Some(file) = cmd.file_size {
            Ok(Self::FileSize(FileSize::new(file)))
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
            Self::CommandToFileSize(command, file_path) => {
                write!(f, "{command} > {file_path} (size)")
            },
            Self::File(file_path) => write!(f, "{file_path}"),
            Self::FileSize(file_path) => write!(f, "{file_path} (size)"),
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
                let results = file_path.get_results()?;
                output.result = Some(results);
                output
            },
            Self::CommandToFileSize(command, file_size) => {
                let mut output = command.run(log).await?;
                let results = file_size.get_results()?;
                output.result = Some(results);
                output
            },
            Self::File(file_path) => {
                let results = file_path.get_results()?;
                Output {
                    result: Some(results),
                    ..Default::default()
                }
            },
            Self::FileSize(file_size) => {
                let results = file_size.get_results()?;
                Output {
                    result: Some(results),
                    ..Default::default()
                }
            },
        })
    }
}
