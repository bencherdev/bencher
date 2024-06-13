use clap::{Parser, ValueEnum};
use std::error::Error;

#[derive(Parser, Debug)]
pub struct CliUp {
    /// Detached mode: Run containers in the background
    #[clap(short, long)]
    pub detach: bool,

    /// Pull image before running ("always"|"missing"|"never")
    #[clap(long)]
    pub pull: Option<CliUpPull>,

    /// Pass environment variables to containers. Same semantic than docker run --env option.
    #[clap(short, long, value_parser = check_key_value)]
    pub env: Vec<String>,
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

/// check that input argument is in the form 'key=value'
fn check_key_value(s: &str) -> Result<String, Box<dyn Error + Send + Sync + 'static>> {
    s.find('=')
        .ok_or_else(|| format!("invalid KEY=value: no `=` found in `{s}`"))?;

    Ok(String::from(s))
}
