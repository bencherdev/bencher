use clap::{
    Args,
    Parser,
    ValueEnum,
};
use uuid::Uuid;

use super::CliLocality;

#[derive(Parser, Debug)]
pub struct CliRun {
    #[clap(flatten)]
    pub locality: CliLocality,

    /// Branch UUID
    #[clap(long)]
    pub branch: Option<Uuid>,

    /// Software commit hash
    #[clap(long)]
    pub hash: Option<String>,

    /// Testbed UUID
    #[clap(long)]
    pub testbed: Option<Uuid>,

    /// Benchmark output adapter
    #[clap(value_enum, long)]
    pub adapter: Option<CliRunAdapter>,

    #[clap(flatten)]
    pub command: CliRunCommand,
}

#[derive(Args, Debug)]
pub struct CliRunCommand {
    #[clap(flatten)]
    pub shell: CliRunShell,

    /// Benchmark command
    pub cmd: Option<String>,
}

#[derive(Args, Debug)]
pub struct CliRunShell {
    /// Shell command path
    #[clap(long, requires = "cmd")]
    pub shell: Option<String>,

    /// Shell command flag
    #[clap(long, requires = "cmd")]
    pub flag: Option<String>,
}

/// Supported Adapters
#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliRunAdapter {
    /// JSON (default)
    Json,
    /// Rust `cargo bench` 🦀
    #[clap(alias("rust"), alias("rust_cargo"))]
    RustCargoBench,
}
