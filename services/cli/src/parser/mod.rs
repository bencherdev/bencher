use bencher_json::{Jwt, Url};
use clap::{Args, Parser, Subcommand, ValueEnum};

pub mod mock;
pub mod organization;
pub mod project;
pub mod system;
pub mod up;
pub mod user;

use mock::CliMock;
use organization::{member::CliMember, CliOrganization};
use project::{
    alert::CliAlert, benchmark::CliBenchmark, branch::CliBranch, measure::CliMeasure,
    perf::CliPerf, report::CliReport, run::CliRun, testbed::CliTestbed, threshold::CliThreshold,
    CliProject,
};
use system::{auth::CliAuth, server::CliServer};
use up::CliUp;
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
    #[cfg(feature = "plus")]
    /// Organization metered subscription plan
    #[clap(subcommand)]
    Plan(organization::plan::CliOrganizationPlan),

    /// Manage projects
    #[clap(subcommand)]
    Project(CliProject),

    /// Run benchmarks
    Run(Box<CliRun>),
    /// Manage reports
    #[clap(subcommand)]
    Report(CliReport),
    /// Query benchmark data
    Perf(CliPerf),

    /// Manage branches
    #[clap(subcommand)]
    Branch(CliBranch),
    /// Manage testbeds
    #[clap(subcommand)]
    Testbed(CliTestbed),
    /// View benchmarks
    #[clap(subcommand)]
    Benchmark(CliBenchmark),
    /// Manage measures
    #[clap(subcommand)]
    Measure(CliMeasure),

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

    /// Run `docker compose up` for Bencher Self-Hosted
    Up(CliUp),
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

    /// Initial seconds to wait between attempts (exponential backoff) (default 1)
    #[clap(long)]
    pub retry_after: Option<u64>,

    /// Strictly parse JSON responses (default false)
    #[clap(long)]
    pub strict: bool,
}

#[derive(Args, Debug)]
pub struct CliPagination<T>
where
    T: ValueEnum + Clone + Send + Sync + 'static,
{
    /// What to sort results by
    #[clap(long)]
    pub sort: Option<T>,

    /// The direction to sort the results by
    #[clap(long)]
    pub direction: Option<CliDirection>,

    /// The number of results per page (default 8 max 255)
    #[clap(long)]
    pub per_page: Option<u8>,

    /// Page number of the results to fetch
    #[clap(long)]
    pub page: Option<u32>,
}

/// The direction to sort the results by
#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliDirection {
    /// Ascending
    Asc,
    /// Descending
    Desc,
}

impl From<CliDirection> for bencher_client::types::JsonDirection {
    fn from(direction: CliDirection) -> Self {
        match direction {
            CliDirection::Asc => Self::Asc,
            CliDirection::Desc => Self::Desc,
        }
    }
}
