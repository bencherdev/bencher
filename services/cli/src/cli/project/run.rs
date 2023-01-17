use bencher_json::{BranchName, ResourceId};
use clap::{Args, Parser, ValueEnum};

use crate::cli::CliLocality;

#[derive(Parser, Debug)]
pub struct CliRun {
    /// Project slug or UUID (or set BENCHER_PROJECT)
    #[clap(long)]
    pub project: Option<ResourceId>,

    /// Branch slug or UUID (or set BENCHER_BRANCH) (default is "main")
    #[clap(long)]
    pub branch: Option<ResourceId>,

    /// Run iff a single instance of the branch name exists
    #[clap(long, conflicts_with = "branch", conflicts_with = "local")]
    pub if_branch: Option<Option<BranchName>>,

    /// Create a new branch, clone data, and run iff a single instance of the start point branch name exists (requires `--if-branch`)
    #[clap(long, requires = "if_branch")]
    pub else_if_branch: Option<Option<BranchName>>,

    /// Create a new branch and run if neither `--if-branch` or `--else-if-branch` exists (requires `--if-branch`)
    #[clap(long, requires = "if_branch")]
    pub else_branch: bool,

    /// An optional marker for the end of the if branch statement.
    /// This is useful for `--if-branch` and `--else-if-branch` to exit successfully in the case that an environment variable is empty (requires `--if-branch`)
    #[clap(long, requires = "if_branch")]
    pub endif_branch: bool,

    /// Software commit hash
    #[clap(long)]
    pub hash: Option<String>,

    /// Testbed slug or UUID (or set BENCHER_TESTBED) (default is "localhost")
    #[clap(long)]
    pub testbed: Option<ResourceId>,

    /// Benchmarking tool output adapter
    #[clap(value_enum, long, alias = "tool")]
    pub adapter: Option<CliRunAdapter>,

    /// Number of run iterations (default is `1`)
    #[clap(long)]
    pub iter: Option<usize>,

    /// Fold into a single result value
    #[clap(value_enum, long, requires = "iter")]
    pub fold: Option<CliRunFold>,

    /// Allow test failure
    #[clap(long)]
    pub allow_failure: bool,

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
    /// Magic 🪄 (default)
    Magic,
    /// JSON {...}
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
