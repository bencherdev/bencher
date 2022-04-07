use thiserror::Error;

#[derive(Error, Debug)]
pub enum CliError {
    #[error("No default shell command path for target family")]
    Shell,
    #[error("No default shell command flag for target family")]
    Flag,
    #[error("Failed to execute benchmark: {0}")]
    Benchmark(String),
    #[error("Failed to convert from UTF8")]
    FromUtf8(#[from] std::string::FromUtf8Error),
}
