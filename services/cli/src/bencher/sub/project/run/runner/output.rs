use std::process;

#[derive(Debug, Clone)]
pub struct Output {
    pub status: ExitStatus,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Clone, Default)]
pub struct ExitStatus(i32);

impl From<process::Output> for Output {
    fn from(output: process::Output) -> Self {
        let Output {
            status,
            stdout,
            stderr,
        } = output;
        Self {
            status: status.into(),
            stdout: stdout.into_iter().collect(),
            stderr: stdout.into_iter().collect(),
        }
    }
}

impl From<process::ExitStatus> for ExitStatus {
    fn from(exit_status: process::ExitStatus) -> Self {
        Self(exit_status.code().unwrap_or_default())
    }
}
