use clap::Parser;

#[derive(Parser, Debug)]
pub struct CliUp {
    /// Detached mode: Run containers in the background
    #[clap(short, long)]
    pub detach: bool,
}

#[derive(Parser, Debug)]
pub struct CliDown {}

#[derive(Parser, Debug)]
pub struct CliLogs {
    /// Docker container name
    pub container: Option<String>,
}
