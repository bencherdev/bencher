use std::str::FromStr;

use bencher_json::{BENCHER_API_URL_STR, Jwt, Url};
use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};

pub mod compose;
pub mod mock;
pub mod organization;
pub mod project;
pub mod run;
pub mod system;
pub mod user;

use compose::{CliDown, CliLogs, CliUp};
use mock::CliMock;
use organization::{CliOrganization, member::CliMember};
#[cfg(feature = "plus")]
use project::job::CliJob;
use project::{
    CliProject, alert::CliAlert, archive::CliArchive, benchmark::CliBenchmark, branch::CliBranch,
    measure::CliMeasure, metric::CliMetric, perf::CliPerf, plot::CliPlot, report::CliReport,
    testbed::CliTestbed, threshold::CliThreshold,
};
use run::CliRun;
#[cfg(feature = "plus")]
use system::runner::CliRunner;
#[cfg(feature = "plus")]
use system::spec::CliSpec;
use system::{auth::CliAuth, server::CliServer};
use user::{CliUser, token::CliToken};

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
    /// Run benchmarks
    Run(Box<CliRun>),
    /// Generate mock benchmark data
    Mock(CliMock),

    /// Archive a dimension
    Archive(CliArchive),
    /// Unarchive a dimension
    Unarchive(CliArchive),

    /// Create and start Bencher Self-Hosted containers
    Up(CliUp),
    /// View output from Bencher Self-Hosted containers
    Logs(CliLogs),
    /// Stop and remove Bencher Self-Hosted containers
    Down(CliDown),

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
    #[cfg(feature = "plus")]
    /// Manage organization SSO domains
    #[clap(subcommand)]
    Sso(organization::sso::CliSso),

    /// Manage projects
    #[clap(subcommand)]
    Project(CliProject),

    /// Manage reports
    #[clap(subcommand)]
    Report(CliReport),
    #[cfg(feature = "plus")]
    /// Manage jobs
    #[clap(subcommand)]
    Job(CliJob),
    /// Query benchmark data
    Perf(CliPerf),
    /// Manage plots
    #[clap(subcommand)]
    Plot(CliPlot),

    /// Manage branches
    #[clap(subcommand)]
    Branch(CliBranch),
    /// Manage testbeds
    #[clap(subcommand)]
    Testbed(CliTestbed),
    /// Manage benchmarks
    #[clap(subcommand)]
    Benchmark(CliBenchmark),
    /// Manage measures
    #[clap(subcommand)]
    Measure(CliMeasure),
    /// Manage metrics
    #[clap(subcommand)]
    Metric(CliMetric),

    /// Manage thresholds
    #[clap(subcommand)]
    Threshold(CliThreshold),
    /// Manage alerts
    #[clap(subcommand)]
    Alert(CliAlert),

    /// Manage user
    #[clap(subcommand)]
    User(CliUser),
    /// Manage user API tokens
    #[clap(subcommand)]
    Token(CliToken),

    #[cfg(feature = "plus")]
    /// Manage runners
    #[clap(subcommand)]
    Runner(CliRunner),
    #[cfg(feature = "plus")]
    /// Manage hardware specs
    #[clap(subcommand)]
    Spec(CliSpec),

    /// Server commands
    #[clap(subcommand)]
    Server(CliServer),

    /// Server authentication & authorization
    #[clap(subcommand, hide = true)]
    Auth(CliAuth),
}

#[derive(Args, Debug)]
pub struct CliBackend {
    /// Backend host URL
    #[clap(long, value_name = "URL", env = "BENCHER_HOST", default_value = BENCHER_API_URL_STR)]
    pub host: Url,

    /// User API token
    #[clap(long, env = "BENCHER_API_TOKEN")]
    pub token: Option<Jwt>,

    /// Allow insecure connections to an HTTPS host
    #[clap(long)]
    pub insecure_host: bool,

    /// Load TLS certificates from the platform's native certificate store
    #[clap(long)]
    pub native_tls: bool,

    /// Request timeout
    #[clap(long, value_name = "SECONDS", default_value = "15")]
    pub timeout: u64,

    /// Request attempt(s)
    #[clap(long, value_name = "COUNT", default_value = "10")]
    pub attempts: usize,

    /// Initial seconds to wait between attempts (exponential backoff)
    #[clap(long, value_name = "SECONDS", default_value = "1")]
    pub retry_after: u64,

    /// Strictly parse JSON responses
    #[clap(long)]
    pub strict: bool,
}

#[derive(Args, Debug)]
pub struct CliPagination<T>
where
    T: ValueEnum + Clone + Send + Sync + 'static,
{
    /// What to sort results by
    #[clap(long, value_name = "BY")]
    pub sort: Option<T>,

    /// The direction to sort the results by
    #[clap(long)]
    pub direction: Option<CliDirection>,

    /// The number of results per page (default 8) (max 255)
    #[clap(long, value_name = "COUNT")]
    pub per_page: Option<u8>,

    /// Page number of the results to fetch
    #[clap(long, value_name = "NUMBER")]
    pub page: Option<u32>,
}

/// The direction to sort the results by
#[derive(ValueEnum, Debug, Clone, Copy)]
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

#[derive(Args, Debug)]
#[clap(group(
    ArgGroup::new("archived")
        .multiple(false)
        .args(&["archive", "unarchive"]),
))]
pub struct CliArchived {
    /// Set as archived
    #[clap(long)]
    pub archive: bool,

    /// Set as unarchived
    #[clap(long)]
    pub unarchive: bool,
}

impl From<CliArchived> for Option<bool> {
    fn from(archived: CliArchived) -> Option<bool> {
        match (archived.archive, archived.unarchive) {
            (false, false) => None,
            (false, true) => Some(false),
            (true, false) => Some(true),
            #[expect(clippy::unreachable)]
            (true, true) => unreachable!("Cannot set both `archive` and `unarchive`"),
        }
    }
}

pub(crate) fn check_env(arg: &str) -> Result<String, String> {
    check_key_value::<'='>("KEY", "VALUE", arg, false)
}

/// Check that input argument is in the form `left<separator>right`
pub(crate) fn check_key_value<const SEPARATOR: char>(
    left: &str,
    right: &str,
    arg: &str,
    require_right: bool,
) -> Result<String, String> {
    let index = arg.find(SEPARATOR)
        .ok_or_else(|| format!("Failed to parse argument, expected format `{left}{SEPARATOR}{right}` but no `{SEPARATOR}` was found in: `{arg}`"))?;
    if index == 0 {
        return Err(format!(
            "Failed to parse argument, expected format `{left}{SEPARATOR}{right}` but no `{left}` was found in: `{arg}`"
        ));
    }
    if require_right && index == arg.len() - 1 {
        return Err(format!(
            "Failed to parse argument, expected format `{left}{SEPARATOR}{right}` but no `{right}` was found in: `{arg}`"
        ));
    }
    Ok(arg.into())
}

#[derive(Debug, Clone)]
pub struct ElidedOption<T>(Option<T>);

impl<T> FromStr for ElidedOption<T>
where
    T: FromStr,
{
    type Err = T::Err;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        if s == "_" {
            Ok(ElidedOption(None))
        } else {
            s.parse().map(Some).map(ElidedOption)
        }
    }
}

impl<T> From<ElidedOption<T>> for Option<T> {
    fn from(elided: ElidedOption<T>) -> Option<T> {
        elided.0
    }
}
