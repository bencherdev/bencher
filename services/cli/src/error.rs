use thiserror::Error;

#[derive(Error, Debug)]
pub enum BencherError {
    // Wide
    #[error("Failed to find Bencher user email. Set the `-e`/`--email` flag or the `BENCHER_EMAIL` environment variable.")]
    EmailNotFound,
    #[error("Failed to parse user email: {0}")]
    Email(String),
    #[error("Failed to find Bencher user API token. Set the `-t`/`--token` flag or the `BENCHER_TOKEN` environment variable.")]
    TokenNotFound,
    #[error("Failed to parse backend URL.")]
    Url(#[from] url::ParseError),

    #[error("No default shell command path for target family. Try setting a custom shell with the `-s`/`--shell` flag.")]
    Shell,
    #[error("No default shell command flag for target family. Try setting a custom shell command flag with the `-f`/`--flag` flag.")]
    Flag,
    #[error("The subcommand `run` requires either a command argument to run or the result of a run via stdin.")]
    Benchmark,
    #[error("Failed I/O")]
    Io(#[from] std::io::Error),
    #[error("Failed to convert from UTF-8")]
    FromUtf8(#[from] std::string::FromUtf8Error),
    #[error("Failed git")]
    Git(#[from] git2::Error),
    #[error("Failed serde json")]
    Serde(#[from] serde_json::Error),
    #[error("Failed to send report to backend: {0}")]
    Client(#[from] reqwest::Error),
}
