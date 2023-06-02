use bencher_json::{Jwt, Url};
use clap::{Args, Parser, Subcommand};

pub mod docs;
pub mod mock;
pub mod organization;
pub mod project;
pub mod system;
pub mod user;

#[cfg(feature = "docs")]
use docs::CliDocs;
use mock::CliMock;
use organization::{member::CliMember, CliOrganization};
use project::{
    alert::CliAlert, benchmark::CliBenchmark, branch::CliBranch, metric_kind::CliMetricKind,
    perf::CliPerf, report::CliReport, result::CliResult, run::CliRun, testbed::CliTestbed,
    threshold::CliThreshold, CliProject,
};
use system::{auth::CliAuth, server::CliServer};
use user::{token::CliToken, CliUser};

use self::project::statistic::CliStatistic;

/// Bencher CLI
#[derive(Parser, Debug)]
#[clap(name = "bencher", author, version, about, long_about = None)]
pub struct CliBencher {
    /// Bencher subcommands
    #[clap(subcommand)]
    pub sub: CliSub,
}

#[derive(Subcommand, Debug)]
pub enum CliSub {
    /// Server authentication & authorization
    #[clap(subcommand)]
    Auth(CliAuth),

    /// Manage organization
    #[clap(subcommand, alias = "org")]
    Organization(CliOrganization),
    /// Manage organization members
    #[clap(subcommand)]
    Member(CliMember),

    /// Manage projects
    #[clap(subcommand)]
    Project(CliProject),

    /// Run benchmarks
    Run(CliRun),
    /// Query benchmark data
    Perf(CliPerf),

    /// Manage reports
    #[clap(subcommand)]
    Report(CliReport),
    /// View report results
    #[clap(subcommand)]
    Result(CliResult),

    /// Manage metric kinds
    #[clap(subcommand)]
    MetricKind(CliMetricKind),
    /// Manage branches
    #[clap(subcommand)]
    Branch(CliBranch),
    /// Manage testbeds
    #[clap(subcommand)]
    Testbed(CliTestbed),
    /// View benchmarks
    #[clap(subcommand)]
    Benchmark(CliBenchmark),

    /// Manage thresholds
    #[clap(subcommand)]
    Threshold(CliThreshold),
    /// Manage threshold statistics
    #[clap(subcommand)]
    Statistic(CliStatistic),
    /// View alerts
    #[clap(subcommand)]
    Alert(CliAlert),

    /// View user
    #[clap(subcommand)]
    User(CliUser),
    /// Manage user API tokens
    #[clap(subcommand)]
    Token(CliToken),

    /// Server commands
    #[clap(subcommand)]
    Server(CliServer),

    /// Generate mock benchmark data
    Mock(CliMock),

    #[cfg(feature = "docs")]
    /// Generate documentation
    Docs(CliDocs),
}

#[derive(Args, Debug)]
pub struct CliBackend {
    /// Backend host URL (default https://api.bencher.dev)
    #[clap(long)]
    pub host: Option<Url>,

    /// User API token
    #[clap(long)]
    pub token: Option<Jwt>,

    /// Request attempt(s) (default 10)
    #[clap(long)]
    pub attempts: Option<usize>,

    /// Retry after second(s) (default 3)
    #[clap(long)]
    pub retry_after: Option<u64>,
}
