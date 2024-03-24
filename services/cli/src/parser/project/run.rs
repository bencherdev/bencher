use bencher_json::{DateTime, GitHash, NameId, ResourceId};
use camino::Utf8PathBuf;
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

    /// Git commit hash (defaults to HEAD)
    #[clap(long)]
    pub hash: Option<GitHash>,

    /// Do not try to find a Git commit hash
    #[clap(long, conflicts_with = "hash")]
    pub no_hash: bool,

    /// Testbed name, slug, or UUID (or set BENCHER_TESTBED) (default is "localhost")
    /// If a name or slug is provided, the testbed will be created if it does not exist
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
    /// If `branch` does not already exist, it will be created if a name or slug is provided
    #[clap(long, alias = "if-branch")]
    pub branch: Option<NameId>,

    /// Use the given branch name, slug, or UUID as the start point for `branch`.
    /// If `branch` does not exist, it will be created using this start point.
    /// If `branch` does exist and the start point is not different, then this argument is effectively ignored.
    /// If `branch` does exist and the start point is different, the old version of `branch` will be archived
    /// and a new `branch` will be created using this updated start point.
    /// Specifying more than one start point is now deprecated.
    /// Only the first start point will be used.
    #[clap(long, alias = "else-if-branch")]
    // TODO move this to Option<NameId> in due time
    pub branch_start_point: Vec<NameId>,

    /// Use the given full Git hash as the start point for `branch` (requires: `--branch-start-point`)
    /// If not specified, the most recent version for the branch start point will be used.
    /// If specified, this will set the Git hash for the branch start point.
    #[clap(long, requires = "branch_start_point")]
    pub branch_start_point_hash: Option<GitHash>,

    /// Deprecated: Do not use. This will soon be removed.
    #[clap(long, alias = "else-branch")]
    pub endif_branch: bool,
}

#[derive(Args, Debug)]
pub struct CliRunCommand {
    /// Benchmark command output file path
    #[clap(long)]
    pub file: Option<Utf8PathBuf>,

    #[clap(flatten)]
    pub sh_c: CliRunShell,

    /// Run as an executable not a shell command (default if args > 1)
    #[clap(long)]
    #[clap(
        requires = "command",
        conflicts_with = "shell",
        conflicts_with = "flag"
    )]
    pub exec: bool,

    /// Benchmark command
    #[clap(trailing_var_arg = true, allow_hyphen_values = true)]
    pub command: Option<Vec<String>>,
}

#[derive(Args, Debug)]
pub struct CliRunShell {
    /// Shell command path
    #[clap(long)]
    pub shell: Option<String>,

    /// Shell command flag
    #[clap(long)]
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
    /// 🦀 Rust Iai-Callgrind
    RustIaiCallgrind,
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
    /// CAUTION: Override safety checks and accept that you are vulnerable to pwn requests (requires: `--github-actions`)
    #[clap(long, requires = "ci_cd")]
    pub ci_i_am_vulnerable_to_pwn_requests: bool,
    /// GitHub API authentication token for GitHub Actions to comment on PRs (ie `--github-actions ${{ secrets.GITHUB_TOKEN }}`)
    #[clap(long)]
    pub github_actions: Option<String>,
}
