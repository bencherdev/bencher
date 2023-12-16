#[derive(thiserror::Error, Debug)]
pub enum RunError {
    #[error("Failed to find Bencher project. Set the `--project` flag or the `BENCHER_PROJECT` environment variable.")]
    ProjectNotFound,
    #[error("Failed to parse UUID or slug for the project: {0}")]
    ParseProject(bencher_json::ValidError),

    #[error("{0}")]
    Branch(#[from] super::branch::BranchError),

    #[error("Failed to parse UUID, slug, or name for the testbed: {0}")]
    ParseTestbed(bencher_json::ValidError),

    #[error("No default shell command path for target family. Try setting a custom shell with the `--shell` flag.")]
    Shell,
    #[error("No default shell command flag for target family. Try setting a custom shell command flag with the `--flag` flag.")]
    Flag,
    #[error("The subcommand `run` requires either a command argument or results via stdin.")]
    NoCommand,

    #[error("Failed to spawn command: {0}")]
    SpawnCommand(std::io::Error),
    #[error("Failed to run command: {0}")]
    RunCommand(std::io::Error),
    #[error("Failed to pipe stdout")]
    PipeStdout,
    #[error("Failed to pipe stderr")]
    PipeStderr,
    #[error("Failed to join stdout: {0}")]
    StdoutJoinError(tokio::task::JoinError),
    #[error("Failed to join stderr: {0}")]
    StderrJoinError(tokio::task::JoinError),
    #[error("Failed to run command due to a non-zero exit code: {0}")]
    ExitStatus(crate::bencher::sub::Output),

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
