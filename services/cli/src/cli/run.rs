use clap::{
    Args,
    Parser,
    ValueEnum,
};

use super::CliLocality;

#[derive(Parser, Debug)]
pub struct CliRun {
    #[clap(flatten)]
    pub locality: CliLocality,

    /// Branch UUID
    #[clap(long)]
    pub branch: String,

    /// Software commit hash
    #[clap(long)]
    pub hash: Option<String>,

    /// Testbed UUID
    #[clap(long)]
    pub testbed: Option<String>,

    /// Benchmark output adapter
    #[clap(value_enum, long)]
    pub adapter: Option<CliAdapter>,

    #[clap(flatten)]
    pub command: CliCommand,
}

#[derive(Args, Debug)]
pub struct CliCommand {
    #[clap(flatten)]
    pub shell: CliShell,

    /// Benchmark command
    pub cmd: Option<String>,
}

#[derive(Args, Debug)]
pub struct CliShell {
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
pub enum CliAdapter {
    /// JSON (default)
    Json,
    /// Rust `cargo bench` 🦀
    #[clap(alias("rust"), alias("rust_cargo"))]
    RustCargoBench,
}
