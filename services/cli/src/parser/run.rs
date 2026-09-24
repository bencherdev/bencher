#[cfg(feature = "plus")]
use bencher_json::SpecResourceId;
use bencher_json::{
    BranchNameId, DateTime, GitHash, ProjectResourceId, TestbedNameId, project::report::Iteration,
};
#[cfg(feature = "plus")]
use bencher_parser::check_env;
use camino::Utf8PathBuf;
use clap::{ArgGroup, Args, Parser, ValueEnum};

use crate::parser::CliBackend;

use super::project::report::{
    CliReportAdapter, CliReportAverage, CliReportFold, CliReportThresholds,
};

// `--key` requires `--project` only for project-scoped keys (`bencher_run_*`)
// without an `--image` named after the project to derive the project from.
// User-scoped keys (`bencher_user_*`) authenticate as the owning user and can
// auto-create a project on the fly just like a JWT. Because clap groups can't
// inspect the parsed value, this constraint is enforced at runtime in the
// `bencher run` handler instead of as a `clap` ArgGroup.
#[derive(Parser, Debug)]
#[cfg_attr(
    feature = "plus",
    expect(
        clippy::struct_excessive_bools,
        reason = "each bool is an independent user flag"
    )
)]
pub struct CliRun {
    #[clap(flatten)]
    pub project: CliRunProject,

    #[clap(flatten)]
    pub branch: CliRunBranch,

    /// Testbed name, slug, or UUID.
    /// If a name or slug is provided, the testbed will be created if it does not exist.
    #[clap(long, env = "BENCHER_TESTBED")]
    pub testbed: Option<TestbedNameId>,

    /// Reset the testbed spec, removing its hardware specification.
    #[cfg(feature = "plus")]
    #[clap(long, conflicts_with = "image", requires = "testbed")]
    pub spec_reset: bool,

    /// Benchmark harness adapter
    #[clap(value_enum, long, env = "BENCHER_ADAPTER", default_value = "magic")]
    pub adapter: CliReportAdapter,

    /// Benchmark harness suggested central tendency (ie average)
    #[clap(value_enum, long)]
    pub average: Option<CliReportAverage>,

    /// Number of run iterations
    #[clap(long, value_name = "COUNT", default_value = "1")]
    pub iter: Iteration,

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
    #[clap(long, alias = "err")]
    pub error_on_alert: bool,

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
    #[clap(long, alias = "else-if-branch", alias = "branch-start-point")]
    pub start_point: Option<String>,

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
    #[clap(long, conflicts_with = "file_size")]
    pub file: Option<Vec<Utf8PathBuf>>,

    /// Track the size of a file at the given file path
    #[clap(long, conflicts_with = "file")]
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

impl CliRunCommand {
    pub fn has_local_input(&self) -> bool {
        self.command.is_some() || self.file.is_some() || self.file_size.is_some()
    }
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

#[expect(
    clippy::struct_excessive_bools,
    reason = "each bool is an independent CI flag"
)]
#[derive(Args, Debug)]
#[clap(group(
    ArgGroup::new("ci_cd")
        .multiple(false)
        .args(&["github_actions"]),
))]
pub struct CliRunCi {
    /// GitHub API authentication token for GitHub Actions to create a GitHub Check and comment on PRs (ie `--github-actions ${{ secrets.GITHUB_TOKEN }}`)
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
    /// Custom ID for posting results to CI, used in place of the Project name in the GitHub Check name (ie `Bencher Report (<ID>)`) (requires: `--github-actions`)
    #[clap(long, requires = "ci_cd")]
    pub ci_id: Option<String>,
    /// Issue number for posting results to CI (requires: `--github-actions`)
    #[clap(long, requires = "ci_cd")]
    pub ci_number: Option<u64>,
    /// CAUTION: Override safety checks and accept that you are vulnerable to pwn requests (requires: `--github-actions`)
    #[clap(long, requires = "ci_cd", hide = true)]
    pub ci_i_am_vulnerable_to_pwn_requests: bool,
}

/// Remote runner options: submit a job with `--image` or attach to one with `--job` (Bencher Plus).
#[cfg(feature = "plus")]
#[derive(Args, Debug)]
#[clap(group(
    ArgGroup::new("remote_job")
        .multiple(false)
        .args(["image", "job"]),
))]
pub struct CliRunJob {
    /// OCI image reference for remote runner execution (e.g. "alpine:3.18", "ghcr.io/owner/repo:v1")
    #[clap(long)]
    pub image: Option<bencher_json::ImageReference>,

    /// Attach to a submitted remote job: wait for it, then post its results (requires: --project).
    /// It refuses the report options, including from `BENCHER_BRANCH`, `BENCHER_TESTBED`, `BENCHER_ADAPTER`, and `BENCHER_CMD`.
    #[clap(
        long,
        value_name = "UUID",
        requires = "project",
        conflicts_with_all = [
            "ci_on_the_fly",
            "branch",
            "hash",
            "start_point",
            "start_point_hash",
            "start_point_max_versions",
            "start_point_clone_thresholds",
            "start_point_reset",
            "deprecated",
            "testbed",
            "spec_reset",
            "adapter",
            "average",
            "iter",
            "fold",
            "backdate",
            "allow_failure",
            "threshold_measure",
            "threshold_test",
            "threshold_min_sample_size",
            "threshold_max_sample_size",
            "threshold_window",
            "threshold_lower_boundary",
            "threshold_upper_boundary",
            "thresholds_reset",
            "build_time",
            "file",
            "file_size",
            "shell",
            "flag",
            "exec",
            "command",
            "dry_run",
            "spec",
            "entrypoint",
            "env",
            "detach",
        ]
    )]
    pub job: Option<bencher_json::JobUuid>,

    /// Hardware spec slug or UUID (requires: --image)
    #[clap(long, requires = "image")]
    pub spec: Option<SpecResourceId>,

    /// Container entrypoint override (requires: --image)
    // Single string to match `docker run --entrypoint` semantics.
    // Watch https://github.com/docker/cli/issues/4870 for multi-arg support.
    #[clap(long, requires = "image")]
    pub entrypoint: Option<String>,

    /// Environment variable in KEY=VALUE format (requires: --image)
    #[clap(long, requires = "image", value_parser = check_env)]
    pub env: Option<Vec<String>>,

    /// Maximum job execution time in seconds, or with `--job` the maximum seconds to wait (requires: --image or --job)
    #[clap(long, requires = "remote_job")]
    pub job_timeout: Option<bencher_json::Timeout>,

    /// Poll interval in seconds when waiting for remote job completion (requires: --image or --job)
    // TODO remove in due time
    #[clap(long, alias = "poll-interval", requires = "remote_job")]
    pub job_poll_interval: Option<bencher_json::PollTimeout>,

    /// Detach after submitting the remote job, without waiting for completion (requires: --image).
    #[clap(long, requires = "image", conflicts_with = "job_poll_interval")]
    pub detach: bool,
}

#[cfg(all(test, feature = "plus"))]
mod tests {
    use clap::{CommandFactory as _, Parser as _, error::ErrorKind};

    use super::CliRun;

    const JOB: &str = "8d2b6c4e-5f3a-4b1c-9e7d-0a1b2c3d4e5f";
    const HASH: &str = "0123456789abcdef0123456789abcdef01234567";
    const PROJECT_KEY: &str = "bencher_run_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh";
    const JWT: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhdXRoIiwiZXhwIjoxNjY5Mjk5NjExLCJpYXQiOjE2NjkyOTc4MTEsImlzcyI6ImJlbmNoZXIuZGV2Iiwic3ViIjoiYUBhLmNvIiwib3JnIjpudWxsfQ.jJmb_nCVJYLD5InaIxsQfS7x87fUsnCYpQK9SrWrKTc";

    // Every `bencher run` argument that shapes a new report, or runs or submits a benchmark,
    // with a value that parses on its own.
    const CONFLICTING: &[(&str, &[&str])] = &[
        ("ci_on_the_fly", &["--ci-on-the-fly"]),
        ("branch", &["--branch", "main"]),
        ("hash", &["--hash", HASH]),
        ("start_point", &["--start-point", "main"]),
        ("start_point_hash", &["--start-point-hash", HASH]),
        (
            "start_point_max_versions",
            &["--start-point-max-versions", "8"],
        ),
        (
            "start_point_clone_thresholds",
            &["--start-point-clone-thresholds"],
        ),
        ("start_point_reset", &["--start-point-reset"]),
        ("deprecated", &["--else-branch"]),
        ("testbed", &["--testbed", "base"]),
        ("spec_reset", &["--spec-reset"]),
        ("adapter", &["--adapter", "json"]),
        ("average", &["--average", "median"]),
        ("iter", &["--iter", "3"]),
        ("fold", &["--fold", "min"]),
        ("backdate", &["--backdate", "1700000000"]),
        ("allow_failure", &["--allow-failure"]),
        ("threshold_measure", &["--threshold-measure", "latency"]),
        ("threshold_test", &["--threshold-test", "t_test"]),
        (
            "threshold_min_sample_size",
            &["--threshold-min-sample-size", "2"],
        ),
        (
            "threshold_max_sample_size",
            &["--threshold-max-sample-size", "64"],
        ),
        ("threshold_window", &["--threshold-window", "60"]),
        (
            "threshold_lower_boundary",
            &["--threshold-lower-boundary", "0.95"],
        ),
        (
            "threshold_upper_boundary",
            &["--threshold-upper-boundary", "0.99"],
        ),
        ("thresholds_reset", &["--thresholds-reset"]),
        ("build_time", &["--build-time"]),
        ("file", &["--file", "results.json"]),
        ("file_size", &["--file-size", "binary"]),
        ("shell", &["--shell", "sh"]),
        ("flag", &["--flag", "-c"]),
        ("exec", &["--exec"]),
        ("command", &["bencher", "mock"]),
        ("dry_run", &["--dry-run"]),
        ("image", &["--image", "alpine:3.18"]),
        ("spec", &["--spec", "test-spec"]),
        ("entrypoint", &["--entrypoint", "sh"]),
        ("env", &["--env", "KEY=VALUE"]),
        ("detach", &["--detach"]),
    ];

    // Every `bencher run` argument that works beside `--job`.
    const ACCEPTED: &[(&str, &[&str])] = &[
        ("project", &[]),
        ("job", &[]),
        ("error_on_alert", &["--error-on-alert"]),
        ("format", &["--format", "json"]),
        ("quiet", &["--quiet"]),
        ("github_actions", &["--github-actions", "token"]),
        (
            "ci_only_thresholds",
            &["--github-actions", "token", "--ci-only-thresholds"],
        ),
        (
            "ci_only_on_alert",
            &["--github-actions", "token", "--ci-only-on-alert"],
        ),
        (
            "ci_public_links",
            &["--github-actions", "token", "--ci-public-links"],
        ),
        ("ci_id", &["--github-actions", "token", "--ci-id", "suite"]),
        (
            "ci_number",
            &["--github-actions", "token", "--ci-number", "7"],
        ),
        (
            "ci_i_am_vulnerable_to_pwn_requests",
            &[
                "--github-actions",
                "token",
                "--ci-i-am-vulnerable-to-pwn-requests",
            ],
        ),
        ("job_timeout", &["--job-timeout", "7"]),
        ("job_poll_interval", &["--job-poll-interval", "3"]),
        ("host", &["--host", "http://localhost:61016"]),
        ("token", &["--token", JWT]),
        ("key", &["--key", PROJECT_KEY]),
        ("insecure_host", &["--insecure-host"]),
        ("native_tls", &["--native-tls"]),
        ("timeout", &["--timeout", "30"]),
        ("attempts", &["--attempts", "3"]),
        ("retry_after", &["--retry-after", "2"]),
        ("max_retry_after", &["--max-retry-after", "4"]),
        ("strict", &["--strict"]),
    ];

    fn parse(args: &[&str]) -> Result<CliRun, clap::Error> {
        CliRun::try_parse_from(args)
    }

    fn parse_job(args: &[&str]) -> Result<CliRun, clap::Error> {
        let args = ["run", "--project", "my-project", "--job", JOB]
            .into_iter()
            .chain(args.iter().copied())
            .collect::<Vec<_>>();
        parse(&args)
    }

    #[test]
    fn job_classifies_every_argument() {
        let ids = CliRun::command()
            .get_arguments()
            .map(|arg| arg.get_id().as_str().to_owned())
            .collect::<Vec<_>>();
        for id in &ids {
            let conflicting = CONFLICTING.iter().any(|(c, _)| c == id);
            let accepted = ACCEPTED.iter().any(|(a, _)| a == id);
            assert!(
                conflicting ^ accepted,
                "`{id}` must be either accepted or conflicting beside `--job`"
            );
        }
        for (id, _) in CONFLICTING.iter().chain(ACCEPTED) {
            assert!(ids.iter().any(|arg| arg == id), "`{id}` is not an argument");
        }
    }

    #[test]
    fn job_conflicts() {
        for (id, args) in CONFLICTING {
            let err = parse_job(args).expect_err(id);
            assert_eq!(err.kind(), ErrorKind::ArgumentConflict, "{id}: {err}");
        }
    }

    #[test]
    fn job_accepts() {
        for (id, args) in ACCEPTED {
            let run = parse_job(args).unwrap_or_else(|err| panic!("{id}: {err}"));
            assert_eq!(
                run.job.job.map(|job| job.to_string()).as_deref(),
                Some(JOB),
                "{id}"
            );
            assert!(run.cmd.command.is_none(), "{id}");
        }
    }

    // Assumes `BENCHER_PROJECT` is unset.
    #[test]
    fn job_requires_project() {
        let err = parse(&["run", "--job", JOB]).unwrap_err();
        assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument, "{err}");
    }

    #[test]
    fn job_ignores_defaults() {
        let run = parse_job(&[]).unwrap();
        assert!(matches!(run.adapter, super::CliReportAdapter::Magic));
        assert_eq!(run.iter.as_usize(), 1);
        for explicit in [["--adapter", "magic"], ["--iter", "1"]] {
            let err = parse_job(&explicit).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::ArgumentConflict, "{err}");
        }
    }

    #[test]
    fn job_wait_options_require_image_or_job() {
        for args in [["--job-timeout", "7"], ["--job-poll-interval", "3"]] {
            let args = ["run", "--project", "my-project"]
                .into_iter()
                .chain(args)
                .chain(["bencher", "mock"])
                .collect::<Vec<_>>();
            let err = parse(&args).unwrap_err();
            assert_eq!(err.kind(), ErrorKind::MissingRequiredArgument, "{err}");
        }
    }
}
