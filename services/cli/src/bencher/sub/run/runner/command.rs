use std::{fmt, process::Stdio};

use chrono::Utc;
use tokio::io::{AsyncBufReadExt as _, BufReader};

use super::build_time::{BuildCommand, BuildTime};
use super::file_path::FilePath;
use super::file_size::FileSize;
use super::{flag::Flag, output::Output, shell::Shell};
use crate::{bencher::sub::RunError, parser::run::CliRunShell};
use crate::{cli_eprintln_quietable, cli_println_quietable};

#[derive(Debug, Clone)]
pub enum Command {
    Shell {
        shell: Shell,
        flag: Flag,
        command: String,
    },
    Exec {
        program: String,
        arguments: Vec<String>,
    },
}

impl fmt::Display for Command {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Shell {
                shell,
                flag,
                command,
            } => write!(f, "{shell} {flag} {command}"),
            Self::Exec { program, arguments } => {
                let args = arguments.join(" ");
                write!(f, "{program} {args}")
            },
        }
    }
}

impl Command {
    pub fn new_shell(sh_c: CliRunShell, command: String) -> Result<Self, RunError> {
        let CliRunShell { shell, flag } = sh_c;
        Ok(Self::Shell {
            shell: shell.try_into()?,
            flag: flag.try_into()?,
            command,
        })
    }

    pub fn new_exec(program: String, arguments: Vec<String>) -> Self {
        Self::Exec { program, arguments }
    }

    /// Return the command as a list of arguments without executing it.
    /// For shell commands: `[shell, flag, command]`
    /// For exec commands: `[program, arg1, arg2, ...]`
    pub fn to_args(&self) -> Vec<String> {
        match self {
            Self::Shell {
                shell,
                flag,
                command,
            } => vec![
                shell.as_ref().to_owned(),
                flag.as_ref().to_owned(),
                command.clone(),
            ],
            Self::Exec { program, arguments } => {
                let mut args = Vec::with_capacity(1 + arguments.len());
                args.push(program.clone());
                args.extend(arguments.iter().cloned());
                args
            },
        }
    }

    pub async fn run(
        &self,
        log: bool,
        build_time: Option<BuildTime>,
    ) -> Result<CommandOutput, RunError> {
        let (output, duration) = self.run_inner(log).await?;
        let build_command = build_time.map(|bt| bt.command(self.to_string(), duration));
        Ok(CommandOutput::new(build_command, output))
    }

    async fn run_inner(&self, log: bool) -> Result<(Output, f64), RunError> {
        let start_time = Utc::now();
        let mut child = match self {
            Self::Shell {
                shell,
                flag,
                command,
            } => tokio::process::Command::new(shell.as_ref())
                .arg(flag.as_ref())
                .arg(command)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn(),
            Self::Exec { program, arguments } => tokio::process::Command::new(program)
                .args(arguments)
                .stdout(Stdio::piped())
                .stderr(Stdio::piped())
                .spawn(),
        }
        .map_err(|err| RunError::SpawnCommand {
            command: self.clone(),
            err,
        })?;

        let child_stdout = child
            .stdout
            .take()
            .ok_or_else(|| RunError::PipeStdout(self.clone()))?;
        let stdout = tokio::spawn(async move {
            let stdout_reader = BufReader::new(child_stdout);
            let mut stdout_lines = stdout_reader.lines();

            let mut stdout = String::new();
            while let Ok(Some(line)) = stdout_lines.next_line().await {
                cli_println_quietable!(log, "{line}");
                if stdout.is_empty() {
                    stdout = line;
                } else {
                    stdout = format!("{stdout}\n{line}");
                }
            }

            stdout
        });

        let child_stderr = child
            .stderr
            .take()
            .ok_or_else(|| RunError::PipeStderr(self.clone()))?;
        let stderr = tokio::spawn(async move {
            let stderr_reader = BufReader::new(child_stderr);
            let mut stderr_lines = stderr_reader.lines();

            let mut stderr = String::new();
            while let Ok(Some(line)) = stderr_lines.next_line().await {
                cli_eprintln_quietable!(log, "{line}");
                if stderr.is_empty() {
                    stderr = line;
                } else {
                    stderr = format!("{stderr}\n{line}");
                }
            }

            stderr
        });

        let (status, stdout, stderr) = tokio::join!(child.wait(), stdout, stderr);
        let end_time = Utc::now();

        let status = status.map_err(|err| RunError::RunCommand {
            command: self.clone(),
            err,
        })?;
        let stdout = stdout.map_err(|err| RunError::StdoutJoinError {
            command: self.clone(),
            err,
        })?;
        let stderr = stderr.map_err(|err| RunError::StderrJoinError {
            command: self.clone(),
            err,
        })?;

        // Calculate the duration of the command.
        // I think seconds are the most reasonable unit of time for this.
        // Two decimal places of sub-second precision should be enough.
        // If not, then users should be using a benchmark instead.
        let duration = end_time
            .timestamp_nanos_opt()
            .and_then(|end_time| end_time.checked_sub(start_time.timestamp_nanos_opt()?))
            .and_then(|d| {
                #[expect(clippy::cast_precision_loss)]
                format!("{:.2}", (d as f64) / 1_000_000_000.0)
                    .parse::<f64>()
                    .ok()
            })
            .unwrap_or_default();

        Ok((
            Output {
                status: status.into(),
                stdout,
                stderr,
                result: None,
            },
            duration,
        ))
    }
}

#[derive(Debug, Default)]
pub struct CommandOutput {
    build_command: Option<BuildCommand>,
    output: Output,
}

impl CommandOutput {
    pub fn new(build_command: Option<BuildCommand>, output: Output) -> Self {
        Self {
            build_command,
            output,
        }
    }

    pub fn build(mut self) -> Result<Output, RunError> {
        if let Some(build_command) = self.build_command {
            let results = build_command.get_results()?;
            self.output.result = Some(results);
        }
        Ok(self.output)
    }

    pub fn build_with_file_path(self, file_path: &FilePath) -> Result<Vec<Output>, RunError> {
        debug_assert!(
            self.build_command.is_none(),
            "Build command should not be set for file path"
        );
        let results = file_path.get_results()?;
        let outputs = results
            .into_iter()
            .map(|result| Output {
                result: Some(result),
                ..self.output.clone()
            })
            .collect();
        Ok(outputs)
    }

    pub fn build_with_file_size(mut self, file_size: &FileSize) -> Result<Output, RunError> {
        let results = file_size.get_results(self.build_command.as_ref())?;
        self.output.result = Some(results);
        Ok(self.output)
    }
}
