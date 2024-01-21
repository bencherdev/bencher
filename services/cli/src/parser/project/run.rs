use std::path::PathBuf;

use bencher_json::{BranchName, DateTime, GitHash, NameId, ResourceId};
use clap::{ArgGroup, Args, Parser, ValueEnum};

use crate::parser::CliBackend;

#[derive(Parser, Debug)]
#[allow(clippy::option_option, clippy::struct_excessive_bools)]
pub struct CliRun {
    /// Project slug or UUID (or set BENCHER_PROJECT)
    #[clap(long)]
    pub project: Option<ResourceId>,

    #[clap(flatten)]
    pub run_branch: CliRunBranch,

    /// Software commit hash
    #[clap(long)]
    pub hash: Option<GitHash>,

    /// Testbed name, slug, or UUID (or set BENCHER_TESTBED) (default is "localhost")
    #[clap(long)]
    pub testbed: Option<NameId>,

    /// Benchmark harness adapter (or set BENCHER_ADAPTER) (default is "magic")
    #[clap(value_enum, long)]
    pub adapter: Option<CliRunAdapter>,

    /// Benchmark harness suggested central tendency (ie average)
    #[clap(value_enum, long)]
    pub average: Option<CliRunAverage>,

    /// Number of run iterations (default is `1`)
    #[clap(long)]
    pub iter: Option<usize>,

    /// Fold multiple results into a single result
    #[clap(value_enum, long, requires = "iter")]
    pub fold: Option<CliRunFold>,

    /// Backdate the report (seconds since epoch)
    /// NOTE: This will *not* effect the ordering of past reports
    #[clap(long)]
    pub backdate: Option<DateTime>,

    /// Allow test failure
    #[clap(long)]
    pub allow_failure: bool,

    /// Error on alert
    #[clap(long)]
    pub err: bool,

    #[clap(flatten)]
    pub fmt: CliRunFmt,

    /// CI integrations
    #[clap(flatten)]
    pub ci: CliRunCi,

    #[clap(flatten)]
    pub cmd: CliRunCommand,

    /// Do a dry run (no data is saved)
    #[clap(long)]
    pub dry_run: bool,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Args, Debug)]
#[allow(clippy::option_option)]
pub struct CliRunBranch {
    /// Branch name, slug, or UUID (or set BENCHER_BRANCH) (default is "main")
    #[clap(long)]
    pub branch: Option<NameId>,

    /// Run using the given branch name if it exists
    #[clap(long, conflicts_with = "branch")]
    pub if_branch: Option<Option<BranchName>>,

    /// If `--else-if-branch` exists, create a new branch named after `--if-branch`
    /// with a clone the data and thresholds from `--else-if-branch` (requires `--if-branch`)
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
    /// Benchmark command output file path
    #[clap(long)]
    pub file: Option<PathBuf>,

    #[clap(flatten)]
    pub sh_c: CliRunShell,

    /// Hint to run as an executable (not a shell command)
    #[clap(long)]
    #[clap(
        requires = "command",
        conflicts_with = "shell",
        conflicts_with = "flag"
    )]
    pub exec: bool,

    /// Benchmark command
    pub command: Option<String>,

    /// Benchmark command arguments
    #[clap(
        requires = "command",
        conflicts_with = "shell",
        conflicts_with = "flag"
    )]
    pub arguments: Vec<String>,
}

#[derive(Args, Debug)]
pub struct CliRunShell {
    /// Shell command path
    #[clap(long, requires = "command")]
    pub shell: Option<String>,

    /// Shell command flag
    #[clap(long, requires = "command")]
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
    /// 🦀 Rust Iai
    RustIai,
    /// ❯_ Shell
    Shell,
    /// ❯_ Shell Hyperfine
    ShellHyperfine,
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

#[derive(Args, Debug)]
#[clap(group(
    ArgGroup::new("run_fmt")
        .multiple(false)
        .args(&["html", "quiet"]),
))]
pub struct CliRunFmt {
    /// Output results as HTML
    #[clap(long)]
    pub html: bool,
    /// Quite mode, only output the final Report JSON
    #[clap(short, long)]
    pub quiet: bool,
}

#[allow(clippy::struct_excessive_bools)]
#[derive(Args, Debug)]
#[clap(group(
    ArgGroup::new("ci_cd")
        .multiple(false)
        .args(&["github_actions"]),
))]
pub struct CliRunCi {
    /// Omit Benchmark Metrics and Boundary Limits (requires: `--github-actions`)
    #[clap(long, requires = "ci_cd")]
    pub ci_no_metrics: bool,
    /// Only post results to CI if a Threshold exists for the Branch, Testbed, and Measure (requires: `--github-actions`)
    #[clap(long, requires = "ci_cd")]
    pub ci_only_thresholds: bool,
    /// Only start posting results to CI if an Alert is generated (requires: `--github-actions`)
    #[clap(long, requires = "ci_cd")]
    pub ci_only_on_alert: bool,
    /// All links should be to public URLs that do not require a login (requires: `--github-actions`)
    #[clap(long, requires = "ci_cd")]
    pub ci_public_links: bool,
    /// Custom ID for posting results to CI (requires: `--github-actions`)
    #[clap(long, requires = "ci_cd")]
    pub ci_id: Option<String>,
    /// Issue number for posting results to CI (requires: `--github-actions`)
    #[clap(long, requires = "ci_cd")]
    pub ci_number: Option<u64>,
    /// GitHub API authentication token for GitHub Actions to comment on PRs (ie `--github-actions ${{ secrets.GITHUB_TOKEN }}`)
    #[clap(long)]
    pub github_actions: Option<String>,
}
