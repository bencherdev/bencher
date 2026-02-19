use bencher_json::{
    BranchNameId, DateTime, GitHash, MAX_FILE_PATHS_LEN, ProjectResourceId, TestbedNameId,
};
#[cfg(feature = "plus")]
use bencher_json::{MAX_ENTRYPOINT_LEN, SpecResourceId};
use camino::Utf8PathBuf;
use clap::{ArgGroup, Args, Parser, ValueEnum};

use crate::parser::CliBackend;
#[cfg(feature = "plus")]
use bencher_parser::check_env;

use super::project::report::{
    CliReportAdapter, CliReportAverage, CliReportFold, CliReportThresholds,
};

#[derive(Parser, Debug)]
pub struct CliRun {
    #[clap(flatten)]
    pub project: CliRunProject,

    #[clap(flatten)]
    pub branch: CliRunBranch,

    /// Testbed name, slug, or UUID.
    /// If a name or slug is provided, the testbed will be created if it does not exist.
    #[clap(long, env = "BENCHER_TESTBED")]
    pub testbed: Option<TestbedNameId>,

    /// Benchmark harness adapter
    #[clap(value_enum, long, env = "BENCHER_ADAPTER", default_value = "magic")]
    pub adapter: CliReportAdapter,

    /// Benchmark harness suggested central tendency (ie average)
    #[clap(value_enum, long)]
    pub average: Option<CliReportAverage>,

    /// Number of run iterations
    #[clap(long, value_name = "COUNT", default_value = "1")]
    pub iter: usize,

    /// Fold multiple results into a single result using an aggregate function
    #[clap(value_enum, long, requires = "iter", value_name = "AGGREGATE_FUNCTION")]
    pub fold: Option<CliReportFold>,

    /// Backdate the report (seconds since epoch)
    /// NOTE: This will NOT effect the ordering of past reports
    #[clap(long, value_name = "SECONDS")]
    pub backdate: Option<DateTime>,

    /// Allow benchmark test failure
    #[clap(long)]
    pub allow_failure: bool,

    #[clap(flatten)]
    pub thresholds: CliReportThresholds,

    /// Error on alert
    #[clap(long)]
    pub err: bool,

    #[clap(flatten)]
    pub output: CliRunOutput,

    /// CI integrations
    #[clap(flatten)]
    pub ci: CliRunCi,

    #[clap(flatten)]
    pub cmd: CliRunCommand,

    /// Do a dry run (no data is saved)
    #[clap(long)]
    pub dry_run: bool,

    #[cfg(feature = "plus")]
    #[clap(flatten)]
    pub job: CliRunJob,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Args, Debug)]
pub struct CliRunProject {
    /// Project slug or UUID
    #[clap(long, env = "BENCHER_PROJECT")]
    pub project: Option<ProjectResourceId>,
    /// Allow on-the-fly project creation in CI environments.
    /// Required if the `CI` environment variable is set to `true`.
    #[clap(long)]
    pub ci_on_the_fly: bool,
}

#[derive(Args, Debug)]
pub struct CliRunBranch {
    /// Branch name, slug, or UUID.
    /// If a name or slug is provided, the branch will be created if it does not exist.
    #[clap(long, env = "BENCHER_BRANCH", alias = "if-branch")]
    pub branch: Option<BranchNameId>,

    /// `git` commit hash (default HEAD)
    #[clap(long)]
    pub hash: Option<GitHash>,

    /// Use the specified branch name, slug, or UUID as the start point for `branch`.
    /// If `branch` already exists and the start point is different, a new branch will be created.
    /// Specifying more than one start point is now deprecated.
    /// Only the first start point will be used.
    #[clap(long, alias = "else-if-branch", alias = "branch-start-point")]
    // TODO move this to Option<String> in due time
    pub start_point: Vec<String>,

    /// Use the specified full `git` hash as the start point for `branch` (requires: `--branch-start-point`).
    /// If `branch` already exists and the start point hash is different, a new branch will be created.
    #[clap(long, alias = "branch-start-point-hash", requires = "start_point")]
    pub start_point_hash: Option<GitHash>,

    /// The maximum number of historical branch versions to include (requires: `--branch-start-point`).
    /// Versions beyond this number will be omitted.
    #[clap(long, requires = "start_point", default_value = "255")]
    pub start_point_max_versions: u32,

    /// Clone thresholds from the start point branch (requires: `--branch-start-point`).
    #[clap(long, requires = "start_point")]
    pub start_point_clone_thresholds: bool,

    /// Reset the branch head to an empty state.
    /// If `start_point` is specified, the new branch head will begin at that start point.
    /// Otherwise, the branch head will be reset to an empty state.
    #[clap(long, alias = "branch-reset")]
    pub start_point_reset: bool,

    /// Deprecated: Do not use. This will soon be removed.
    #[clap(
        long,
        hide = true,
        alias = "else-branch",
        alias = "endif-branch",
        alias = "no-hash"
    )]
    pub deprecated: bool,
}

#[derive(Args, Debug)]
pub struct CliRunCommand {
    /// Track the build time of the benchmark command
    #[clap(long, conflicts_with = "file")]
    pub build_time: bool,

    /// Benchmark command output file path
    #[clap(long, conflicts_with = "file_size", num_args = 1..=MAX_FILE_PATHS_LEN)]
    pub file: Option<Vec<Utf8PathBuf>>,

    /// Track the size of a file at the given file path
    #[clap(long, conflicts_with = "file", num_args = 1..=MAX_FILE_PATHS_LEN)]
    pub file_size: Option<Vec<Utf8PathBuf>>,

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
    #[clap(
        env = "BENCHER_CMD",
        trailing_var_arg = true,
        allow_hyphen_values = true
    )]
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

#[derive(Args, Debug)]
pub struct CliRunOutput {
    /// Format for the final Report
    #[clap(long, default_value = "human")]
    pub format: CliRunFormat,
    /// Quite mode, only output the final Report to standard out
    #[clap(short, long)]
    pub quiet: bool,
}

/// Supported Report Formats
#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliRunFormat {
    /// Human
    Human,
    /// JSON
    Json,
    /// HTML
    Html,
}

#[expect(clippy::struct_excessive_bools)]
#[derive(Args, Debug)]
#[clap(group(
    ArgGroup::new("ci_cd")
        .multiple(false)
        .args(&["github_actions"]),
))]
pub struct CliRunCi {
    /// GitHub API authentication token for GitHub Actions to comment on PRs (ie `--github-actions ${{ secrets.GITHUB_TOKEN }}`)
    #[clap(long)]
    pub github_actions: Option<String>,
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
    #[clap(long, requires = "ci_cd", hide = true)]
    pub ci_i_am_vulnerable_to_pwn_requests: bool,
    /// Deprecated: Do not use. This will soon be removed.
    // TODO remove in due time
    #[clap(long, alias = "ci-no-metrics", hide = true)]
    pub ci_deprecated: bool,
}

/// OCI image and remote runner options (Bencher Plus).
#[cfg(feature = "plus")]
#[derive(Args, Debug)]
pub struct CliRunJob {
    /// OCI image reference for remote runner execution (e.g. "alpine:3.18", "ghcr.io/owner/repo:v1")
    #[clap(long)]
    pub image: Option<bencher_json::ImageReference>,

    /// Hardware spec slug or UUID (requires: --image)
    #[clap(long, requires = "image")]
    pub spec: Option<SpecResourceId>,

    /// Container entrypoint override (requires: --image)
    #[clap(long, requires = "image", num_args = 1..=MAX_ENTRYPOINT_LEN)]
    pub entrypoint: Option<Vec<String>>,

    /// Environment variable in KEY=VALUE format (requires: --image)
    #[clap(long, requires = "image", value_parser = check_env)]
    pub env: Option<Vec<String>>,

    /// Maximum job execution time in seconds (requires: --image)
    #[clap(long, requires = "image")]
    pub job_timeout: Option<bencher_json::Timeout>,
}
