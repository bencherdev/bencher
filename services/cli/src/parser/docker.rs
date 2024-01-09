use clap::{Parser, ValueEnum};

#[derive(Parser, Debug)]
pub struct CliUp {
    /// Detached mode: Run containers in the background
    #[clap(short, long)]
    pub detach: bool,

    /// Pull image before running ("always"|"missing"|"never")
    #[clap(long)]
    pub pull: Option<CliUpPull>,
}

#[derive(ValueEnum, Debug, Clone, Copy, Default)]
#[clap(rename_all = "snake_case")]
pub enum CliUpPull {
    #[default]
    Always,
    Missing,
    Never,
}

#[derive(Parser, Debug)]
pub struct CliDown {}

#[derive(Parser, Debug)]
pub struct CliLogs {
    /// Docker container name
    pub container: Option<String>,
}
