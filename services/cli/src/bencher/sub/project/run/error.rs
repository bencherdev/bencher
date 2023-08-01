#[derive(thiserror::Error, Debug)]
pub enum RunError {
    #[error("Failed to find Bencher project. Set the `--project` flag or the `BENCHER_PROJECT` environment variable.")]
    ProjectNotFound,
    #[error("Failed to parse UUID or slug for the project: {0}")]
    ParseProject(bencher_json::ValidError),

    #[error("Failed to parse UUID or slug for the branch: {0}")]
    ParseBranch(bencher_json::ValidError),
    #[error(
        "{count} branches were found with name \"{branch_name}\" in project \"{project}\"! Exactly one was expected."
    )]
    BranchName {
        project: String,
        branch_name: String,
        count: usize,
    },
    #[error("Failed to get branches: {0}")]
    GetBranches(crate::bencher::BackendError),
    #[error("Failed to create new branch: {0}")]
    CreateBranch(crate::bencher::BackendError),

    #[error("Failed to parse UUID or slug for the testbed: {0}")]
    ParseTestbed(bencher_json::ValidError),

    #[error("No default shell command path for target family. Try setting a custom shell with the `--shell` flag.")]
    Shell,
    #[error("No default shell command flag for target family. Try setting a custom shell command flag with the `--flag` flag.")]
    Flag,
    #[error("The subcommand `run` requires either a command argument or results via stdin.")]
    NoCommand,

    #[error("Failed to run command: {0}")]
    RunCommand(std::io::Error),
    #[error("Failed to run command due to a non-zero exit code: {0}")]
    ExitStatus(crate::bencher::sub::Output),

    #[error("Failed to open output file: {0}")]
    OutputFileOpen(std::io::Error),
    #[error("Failed to read from output file: {0}")]
    OutputFileRead(std::io::Error),

    #[error("Failed to serialize report JSON: {0}")]
    SerializeReport(serde_json::Error),
    #[error("Failed to create new report: {0}")]
    SendReport(crate::bencher::BackendError),
    #[error("Failed to get console endpoint: {0}")]
    GetEndpoint(crate::bencher::BackendError),
    #[error("Alerts detected ({0})")]
    Alerts(usize),

    #[error("GitHub Action repository is not valid: {0}")]
    GitHubActionRepository(String),
    #[error("GitHub Action repository not found for pull request")]
    NoGithubRepository,
    #[error("GitHub Action ref is not for a pull request: {0}")]
    GitHubActionRef(String),
    #[error("GitHub Action ref not found for pull request")]
    NoGitHubActionRef,
    #[error("Failed to authenticate as GitHub Action: {0}")]
    GitHubActionAuth(octocrab::Error),
    #[error("Failed to post GitHub Action comment: {0}")]
    GitHubActionComment(octocrab::Error),
}
