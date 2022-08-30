use thiserror::Error;

#[derive(Error, Debug)]
pub enum BencherError {
    #[error("Failed to find Bencher user API token. Set the `--token` flag or the `BENCHER_TOKEN` environment variable.")]
    TokenNotFound,
    #[error("Failed to find Bencher project branch. Set the `--branch` flag or the `BENCHER_BRANCH` environment variable.")]
    BranchNotFound,
    #[error("Failed to find Bencher project testbed. Set the `--testbed` flag or the `BENCHER_TESTBED` environment variable.")]
    TestbedNotFound,
    #[error("No default shell command path for target family. Try setting a custom shell with the `--shell` flag.")]
    Shell,
    #[error("No default shell command flag for target family. Try setting a custom shell command flag with the `--flag` flag.")]
    Flag,
    #[error("The subcommand `run` requires either a command argument or results via stdin.")]
    NoPerf,
    #[error("Alerts detected.")]
    Alerts,

    #[error("Failed to parse URL: {0}")]
    Url(#[from] url::ParseError),
    #[error("Failed to parse UUID: {0}")]
    Uuid(#[from] uuid::Error),
    #[error("Failed to parse git commit: {0}")]
    Git(#[from] git2::Error),
    #[error("Failed to (de)serialize JSON: {0}")]
    Serde(#[from] serde_json::Error),
    #[error("Failed to send request: {0}")]
    Client(#[from] reqwest::Error),
    #[error("Failed to convert from UTF-8: {0}")]
    FromUtf8(#[from] std::string::FromUtf8Error),
    #[error("Failed to run benchmark command: {0}")]
    Io(#[from] std::io::Error),
}
