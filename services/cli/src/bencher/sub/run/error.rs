#![expect(clippy::absolute_paths)]

use super::runner::{Runner, command::Command};

#[derive(thiserror::Error, Debug)]
pub enum RunError {
    #[error("Failed to check API version: {0}")]
    ApiVersion(crate::BackendError),

    #[error(
        "Attempting to create an on-the-fly project when the `CI` environment variable is set. Use the `--project` option to specify the project slug or UUID. Otherwise, set the `--ci-on-the-fly` flag."
    )]
    CiOnTheFly,

    #[error("{0}")]
    Branch(#[from] super::branch::BranchError),
    #[error("{0}")]
    Thresholds(#[from] crate::bencher::sub::ThresholdsError),

    #[error(
        "The `--build-time` flag requires either a benchmark command or `--image` for remote execution."
    )]
    BuildTimeNoCommandOrImage,

    #[error(
        "No default shell command path for target family. Try setting a custom shell with the `--shell` option."
    )]
    Shell,
    #[error(
        "No default shell command flag for target family. Try setting a custom shell command flag with the `--flag` option."
    )]
    Flag,
    #[error(
        "The subcommand `run` requires either a command argument, results file, or results via stdin."
    )]
    NoCommand,
    /// Defensive guard: `generate_local_report` is only called when `job.is_none()`,
    /// and in that path `runner` is always `Some`. This error exists to protect
    /// against future code changes that might break that invariant.
    #[error(
        "No runner available to generate a local report. The subcommand `run` requires either a command argument, results file, or results via stdin."
    )]
    NoRunner,

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
    #[error("Failed to parse command name: {0}")]
    CommandName(bencher_json::ValidError),
    #[error("Failed to serialize build time results: {0}")]
    SerializeBuildTime(serde_json::Error),
    #[error("Too many file paths ({len}), maximum is {max}")]
    TooManyFilePaths { len: usize, max: usize },
    #[error("Failed to read from output file: {0}")]
    OutputFileRead(std::io::Error),
    #[error("Failed to parse the output file name: {0}")]
    OutputFileName(bencher_json::ValidError),
    #[error("Failed to read size of output file: {0}")]
    OutputFileSize(std::io::Error),
    #[error("Failed to serialize file size results: {0}")]
    SerializeFileSize(serde_json::Error),

    #[error("Failed to serialize report JSON: {0}")]
    SerializeReport(serde_json::Error),
    #[error("Failed to create new report: {0}")]
    SendReport(crate::bencher::BackendError),
    #[error("Failed to get console URL: {0}")]
    ConsoleUrl(crate::bencher::BackendError),
    #[error("Alerts detected ({0})")]
    Alerts(usize),

    #[cfg(feature = "plus")]
    #[error("Failed to poll job status: {0}")]
    PollJob(crate::BackendError),
    #[cfg(feature = "plus")]
    #[error("Failed to fetch updated report: {0}")]
    FetchReport(crate::BackendError),
    #[cfg(feature = "plus")]
    #[error("Remote job failed: {0}")]
    JobFailed(String),
    #[cfg(feature = "plus")]
    #[error("Remote job was canceled")]
    JobCanceled,
    #[cfg(feature = "plus")]
    #[error("Timed out waiting for remote job after {0} seconds")]
    JobTimeout(u64),

    #[error("{0}")]
    Ci(#[from] super::ci::CiError),
}
