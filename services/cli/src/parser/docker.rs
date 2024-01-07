use clap::Parser;

#[derive(Parser, Debug)]
pub struct CliUp {}

#[derive(Parser, Debug)]
pub struct CliDown {}

#[derive(Parser, Debug)]
pub struct CliLogs {
    /// Docker container name
    pub container: Option<String>,
}
