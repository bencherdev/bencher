use std::{fmt, process};

use crate::bencher::sub::RunError;

use super::command::Command;

#[derive(Debug, Clone, Default)]
pub struct Output {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Default)]
pub struct ExitStatus(i32);

impl TryFrom<&Command> for Output {
    type Error = RunError;

    fn try_from(command: &Command) -> Result<Self, Self::Error> {
        std::process::Command::new(command.shell.to_string())
            .arg(command.flag.to_string())
            .arg(&command.cmd)
            .output()
            .map(Into::into)
            .map_err(RunError::RunCommand)
    }
}

impl From<process::Output> for Output {
    fn from(output: process::Output) -> Self {
        let process::Output {
            status,
            stdout,
            stderr,
        } = output;
        Self {
            status: status.into(),
            stdout: String::from_utf8_lossy(&stdout).to_string(),
            stderr: String::from_utf8_lossy(&stderr).to_string(),
        }
    }
}

impl From<process::ExitStatus> for ExitStatus {
    fn from(exit_status: process::ExitStatus) -> Self {
        Self(exit_status.code().unwrap_or_default())
    }
}

impl fmt::Display for Output {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}\n{}\n{}", self.status, self.stdout, self.stderr)
    }
}

impl fmt::Display for ExitStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl Output {
    pub fn is_success(&self) -> bool {
        self.status.is_success()
    }
}

impl ExitStatus {
    pub fn is_success(&self) -> bool {
        self.0 == 0
    }
}
