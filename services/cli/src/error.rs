use thiserror::Error;

#[derive(Error, Debug)]
pub enum BencherError {
    #[error("No default shell command path for target family")]
    Shell,
    #[error("No default shell command flag for target family")]
    Flag,
    #[error("Invalid adapter")]
    Adapter,
    #[error("Failed I/O")]
    Io(#[from] std::io::Error),
    #[error("Failed to convert from UTF-8")]
    FromUtf8(#[from] std::string::FromUtf8Error),
    #[error("Failed git")]
    Git(#[from] git2::Error),
    #[error("Failed serde json")]
    Serde(#[from] serde_json::Error),
    #[error("Failed to parse URL")]
    Url(#[from] url::ParseError),
    #[error("Failed to parse email: {0}")]
    Email(String),
    #[error("Failed to send report to backend: {0}")]
    Client(#[from] reqwest::Error),
    #[error("Failed to find Bencher API token")]
    Token,
}
