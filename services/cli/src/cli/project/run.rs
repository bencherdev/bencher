use bencher_json::ResourceId;
use clap::{Args, Parser, ValueEnum};
use uuid::Uuid;

use crate::cli::CliLocality;

#[derive(Parser, Debug)]
pub struct CliRun {
    /// Project slug or UUID (or set BENCHER_PROJECT)
    #[clap(long)]
    pub project: Option<ResourceId>,

    /// Branch UUID (or set BENCHER_BRANCH)
    #[clap(long)]
    pub branch: Option<Uuid>,

    /// Run if branch name exists (or set BENCHER_BRANCH_NAME)
    #[clap(long, alias = "branch-name", conflicts_with = "branch")]
    pub if_branch: Option<String>,

    /// Software commit hash
    #[clap(long)]
    pub hash: Option<String>,

    /// Testbed UUID (or set BENCHER_TESTBED)
    #[clap(long)]
    pub testbed: Option<Uuid>,

    /// Benchmarking tool output adapter
    #[clap(value_enum, long, alias = "tool")]
    pub adapter: Option<CliRunAdapter>,

    /// Number of run iterations (default is 1)
    #[clap(long)]
    pub iter: Option<usize>,

    /// Fold into a single result value
    #[clap(value_enum, long, requires = "iter")]
    pub fold: Option<CliRunFold>,

    /// Error on alert
    #[clap(long)]
    pub err: bool,

    #[clap(flatten)]
    pub command: CliRunCommand,

    #[clap(flatten)]
    pub locality: CliLocality,
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
    /// Rust 🦀
    Rust,
}

/// Supported Fold Operations
#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliRunFold {
    /// Minimum value
    Min,
    /// Maximum value
    Max,
    /// Mean of values
    Mean,
    /// Median of values
    Median,
}
