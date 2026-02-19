use std::fmt;

use crate::parser::run::CliRunCommand;

mod build_time;
pub mod command;
mod file_path;
mod file_size;
mod flag;
pub mod output;
mod pipe;
mod shell;

use build_time::BuildTime;
use command::{Command, CommandOutput};
use file_path::FilePath;
use file_size::FileSize;
use output::Output;
use pipe::Pipe;

use super::RunError;

#[derive(Debug, Clone)]
pub enum Runner {
    Pipe(Pipe),
    Command(Command, Option<BuildTime>),
    CommandToFile(Command, FilePath),
    CommandToFileSize(Command, Option<BuildTime>, FileSize),
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
            let build_time = cmd.build_time.then_some(BuildTime);
            Ok(if let Some(file_paths) = cmd.file {
                Self::CommandToFile(command, FilePath::new(file_paths))
            } else if let Some(file_paths) = cmd.file_size {
                Self::CommandToFileSize(command, build_time, FileSize::new(file_paths))
            } else {
                Self::Command(command, build_time)
            })
        } else if let Some(file_paths) = cmd.file {
            Ok(Self::File(FilePath::new(file_paths)))
        } else if let Some(file_paths) = cmd.file_size {
            Ok(Self::FileSize(FileSize::new(file_paths)))
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
            Self::Command(command, build_time) => write!(
                f,
                "{command}{build_time}",
                build_time = if build_time.is_some() {
                    " (build time)"
                } else {
                    ""
                }
            ),
            Self::CommandToFile(command, file_path) => {
                write!(f, "{command} > {file_path}")
            },
            Self::CommandToFileSize(command, build_time, file_path) => {
                write!(
                    f,
                    "{command}{build_time} > {file_path} (size)",
                    build_time = if build_time.is_some() {
                        " (build time)"
                    } else {
                        ""
                    }
                )
            },
            Self::File(file_path) => write!(f, "{file_path}"),
            Self::FileSize(file_path) => write!(f, "{file_path} (size)"),
        }
    }
}

impl Runner {
    /// Extract the command as a list of arguments without executing it.
    /// Returns `None` if there is no command (pipe, file-only modes).
    pub fn cmd_args(&self) -> Option<Vec<String>> {
        match self {
            Self::Pipe(_) | Self::File(_) | Self::FileSize(_) => None,
            Self::Command(command, _)
            | Self::CommandToFile(command, _)
            | Self::CommandToFileSize(command, _, _) => Some(command.to_args()),
        }
    }

    /// Extract file paths as strings (for remote job `file_paths` field).
    /// Returns `None` if no file paths are configured.
    pub fn file_paths(&self) -> Option<Vec<String>> {
        match self {
            Self::CommandToFile(_, file_path) | Self::File(file_path) => {
                Some(file_path.paths().iter().map(ToString::to_string).collect())
            },
            Self::CommandToFileSize(_, _, file_size) | Self::FileSize(file_size) => {
                Some(file_size.paths().iter().map(ToString::to_string).collect())
            },
            Self::Pipe(_) | Self::Command(..) => None,
        }
    }

    /// Whether build time tracking is enabled.
    pub fn build_time(&self) -> bool {
        matches!(
            self,
            Self::Command(_, Some(_)) | Self::CommandToFileSize(_, Some(_), _)
        )
    }

    /// Whether file size tracking is enabled.
    pub fn file_size(&self) -> bool {
        matches!(self, Self::CommandToFileSize(..) | Self::FileSize(_))
    }

    pub async fn run(&self, log: bool) -> Result<Vec<Output>, RunError> {
        match self {
            Self::Pipe(pipe) => Ok(pipe.output().into()),
            Self::Command(command, build_time) => {
                command.run(log, *build_time).await?.build().map(Into::into)
            },
            Self::CommandToFile(command, file_path) => command
                .run(log, None)
                .await?
                .build_with_file_path(file_path),
            Self::CommandToFileSize(command, build_time, file_size) => command
                .run(log, *build_time)
                .await?
                .build_with_file_size(file_size)
                .map(Into::into),
            Self::File(file_path) => CommandOutput::default().build_with_file_path(file_path),
            Self::FileSize(file_size) => CommandOutput::default()
                .build_with_file_size(file_size)
                .map(Into::into),
        }
    }
}
