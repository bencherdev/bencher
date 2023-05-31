use std::path::PathBuf;

use bencher_json::{BranchName, ResourceId};
use clap::{Args, Parser, ValueEnum};

use crate::cli::CliBackend;

#[allow(clippy::option_option, clippy::struct_excessive_bools)]
#[derive(Parser, Debug)]
pub struct CliRun {
    /// Project slug or UUID (or set BENCHER_PROJECT)
    #[clap(long)]
    pub project: Option<ResourceId>,

    #[clap(flatten)]
    pub run_branch: CliRunBranch,

    /// Software commit hash
    #[clap(long)]
    pub hash: Option<String>,

    /// Testbed slug or UUID (or set BENCHER_TESTBED) (default is "localhost")
    #[clap(long)]
    pub testbed: Option<ResourceId>,

    /// Benchmark harness adapter
    #[clap(value_enum, long)]
    pub adapter: Option<CliRunAdapter>,

    /// Benchmark harness suggested central tendency (ie average)
    #[clap(value_enum, long)]
    pub average: Option<CliRunAverage>,

    /// Number of run iterations (default is `1`)
    #[clap(long)]
    pub iter: Option<usize>,

    /// Fold multiple metrics into a single metric
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
    pub backend: CliBackend,
}

#[derive(Args, Debug)]
pub struct CliRunBranch {
    /// Branch slug or UUID (or set BENCHER_BRANCH) (default is "main")
    #[clap(long)]
    pub branch: Option<ResourceId>,

    /// Run iff a single instance of the branch name exists
    #[clap(long, conflicts_with = "branch", conflicts_with = "local")]
    pub if_branch: Option<Option<BranchName>>,

    /// Create a new branch, clone data, and run iff a single instance of the start point branch name exists (requires `--if-branch`)
    #[clap(long, requires = "if_branch")]
    pub else_if_branch: Vec<String>,

    /// Create a new branch and run if neither `--if-branch` or `--else-if-branch` exists (requires `--if-branch`)
    #[clap(long, requires = "if_branch")]
    pub else_branch: bool,

    /// An optional marker for the end of the if branch statement. (requires `--if-branch`)
    #[clap(long, requires = "if_branch")]
    pub endif_branch: bool,
}

#[derive(Args, Debug)]
pub struct CliRunCommand {
    #[clap(flatten)]
    pub shell: CliRunShell,

    /// Benchmark command output file path
    #[clap(long, requires = "cmd")]
    pub file: Option<PathBuf>,

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
    /// 🪄 Magic (default)
    Magic,
    /// {...} JSON
    Json,
    /// #️⃣ C#
    CSharp,
    /// #️⃣ C# DotNet
    CSharpDotNet,
    /// ➕ C++
    Cpp,
    /// ➕ C++ Catch2
    CppCatch2,
    /// ➕ C++ Google
    CppGoogle,
    /// 🕳 Go
    Go,
    /// 🕳 Go Bench
    GoBench,
    /// ☕️ Java
    Java,
    /// ☕️ Java JMH
    JavaJmh,
    /// 🕸 JavaScript
    Js,
    /// 🕸 JavaScript Benchmark
    JsBenchmark,
    /// 🕸 JavaScript Time
    JsTime,
    /// 🐍 Python
    Python,
    /// 🐍 Python ASV
    PythonAsv,
    /// 🐍 Python Pytest
    PythonPytest,
    /// ♦️ Ruby
    Ruby,
    /// ♦️ Ruby Benchmark
    RubyBenchmark,
    /// 🦀 Rust
    Rust,
    /// 🦀 Rust Bench
    RustBench,
    /// 🦀 Rust Criterion
    RustCriterion,
}

/// Suggested Central Tendency (Average)
#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliRunAverage {
    /// Mean and standard deviation
    Mean,
    /// Median and interquartile range
    Median,
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
