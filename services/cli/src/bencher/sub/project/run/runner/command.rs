use std::{fmt, process::Stdio};

use tokio::io::{AsyncBufReadExt, BufReader};

use super::{flag::Flag, output::Output, shell::Shell};
use crate::{bencher::sub::RunError, parser::project::run::CliRunShell};
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

    pub async fn run(&self, log: bool) -> Result<Output, RunError> {
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

        Ok(Output {
            status: status.into(),
            stdout,
            stderr,
            result: None,
        })
    }
}
