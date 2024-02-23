use super::runner::{command::Command, Runner};

#[derive(thiserror::Error, Debug)]
pub enum RunError {
    #[error("Failed to find Bencher project. Set the `--project` flag or the `BENCHER_PROJECT` environment variable.")]
    NoProject,
    #[error("Failed to parse UUID or slug for the project: {0}")]
    ParseProject(bencher_json::ValidError),

    #[error("Failed to check API version: {0}")]
    ApiVersion(crate::BackendError),

    #[error("{0}")]
    Branch(#[from] super::branch::BranchError),
    #[error("{0}")]
    Testbed(#[from] super::testbed::TestbedError),

    #[error("No default shell command path for target family. Try setting a custom shell with the `--shell` flag.")]
    Shell,
    #[error("No default shell command flag for target family. Try setting a custom shell command flag with the `--flag` flag.")]
    Flag,
    #[error("The subcommand `run` requires either a command argument or results via stdin.")]
    NoCommand,

    #[error("Set shell ({0}) when running command in exec mode")]
    ShellWithExec(String),
    #[error("Set shell flag ({0}) when running command in exec mode")]
    FlagWithExec(String),
    #[error("Failed to spawn command `{command}`: {err}")]
    SpawnCommand {
        command: Command,
        err: std::io::Error,
    },
    #[error("Failed to pipe stdout for command `{0}`")]
    PipeStdout(Command),
    #[error("Failed to pipe stderr for command `{0}`")]
    PipeStderr(Command),
    #[error("Failed to run command `{command}: {err}")]
    RunCommand {
        command: Command,
        err: std::io::Error,
    },
    #[error("Failed to join stdout for command `{command}`: {err}")]
    StdoutJoinError {
        command: Command,
        err: tokio::task::JoinError,
    },
    #[error("Failed to join stderr for command `{command}`: {err}")]
    StderrJoinError {
        command: Command,
        err: tokio::task::JoinError,
    },
    #[error("Failed to run command due to a non-zero exit code for runner `{runner}`: {output}")]
    ExitStatus {
        runner: Box<Runner>,
        output: crate::bencher::sub::Output,
    },
    #[error("Failed to read from output file: {0}")]
    OutputFileRead(std::io::Error),

    #[error("Failed to serialize report JSON: {0}")]
    SerializeReport(serde_json::Error),
    #[error("Failed to create new report: {0}")]
    SendReport(crate::bencher::BackendError),
    #[error("Failed to get console endpoint: {0}")]
    GetEndpoint(crate::bencher::BackendError),
    #[error("Invalid console endpoint: {0}")]
    BadEndpoint(bencher_json::ValidError),
    #[error("Alerts detected ({0})")]
    Alerts(usize),

    #[error("{0}")]
    Ci(#[from] super::ci::CiError),
}
