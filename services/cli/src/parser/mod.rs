use std::str::FromStr;

use bencher_json::{BENCHER_API_URL_STR, BencherKey, Jwt, Url};
use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};

pub mod compose;
pub mod mock;
pub mod noise;
pub mod organization;
pub mod project;
pub mod run;
pub mod system;
pub mod user;

use compose::{CliDown, CliLogs, CliUp};
use mock::CliMock;
use noise::CliNoise;
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
    /// Measure environment noise
    Noise(CliNoise),

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
    /// Manage user API tokens. Deprecated: prefer user API keys
    /// (`bencher user key`); existing tokens can still be managed.
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
#[clap(group(ArgGroup::new("bencher_credential").args(["token", "key"]).multiple(false)))]
pub struct CliBackend {
    /// Backend host URL
    #[clap(long, value_name = "URL", env = "BENCHER_HOST", default_value = BENCHER_API_URL_STR)]
    pub host: Url,

    /// User API token (JWT). Deprecated: prefer `--key`/`BENCHER_API_KEY` with a
    /// `bencher_user_*` API key.
    #[clap(long, env = "BENCHER_API_TOKEN", value_parser = parse_token)]
    pub token: Option<Jwt>,

    /// Bencher API key. Either a user-scoped key (`bencher_user_*`) or a
    /// project-scoped key (`bencher_run_*`).
    #[clap(long, env = "BENCHER_API_KEY", value_parser = parse_key)]
    pub key: Option<BencherKey>,

    /// Allow insecure connections to an HTTPS host
    #[clap(long)]
    pub insecure_host: bool,

    /// Load TLS certificates from the platform's native certificate store
    #[clap(long)]
    pub native_tls: bool,

    /// Request timeout
    #[clap(long, value_name = "SECONDS", default_value = "15", value_parser = clap::value_parser!(u16).range(1..=900))]
    pub timeout: u16,

    /// Request attempt(s)
    #[clap(long, value_name = "COUNT", default_value = "35", value_parser = clap::value_parser!(u16).range(1..))]
    pub attempts: u16,

    /// Initial seconds to wait between attempts (exponential backoff)
    #[clap(long, value_name = "SECONDS", default_value = "1", value_parser = clap::value_parser!(u16).range(1..=900))]
    pub retry_after: u16,

    /// Max seconds to wait between attempts (caps exponential backoff)
    #[clap(long, value_name = "SECONDS", default_value = "30", value_parser = clap::value_parser!(u16).range(1..=900))]
    pub max_retry_after: u16,

    /// Strictly parse JSON responses
    #[clap(long)]
    pub strict: bool,
}

/// Parse a `--token` value, suggesting `--key` if a Bencher API key was supplied.
///
/// JWTs and Bencher API keys are structurally disjoint, so a value that fails to
/// parse as a `Jwt` but parses as a `BencherKey` is unambiguously the wrong flag.
fn parse_token(value: &str) -> Result<Jwt, String> {
    value.parse::<Jwt>().map_err(|err| {
        if value.parse::<BencherKey>().is_ok() {
            "You supplied a Bencher API key to `--token`/`BENCHER_API_TOKEN`. \
             Use `--key`/`BENCHER_API_KEY` instead."
                .to_owned()
        } else {
            err.to_string()
        }
    })
}

/// Parse a `--key` value, suggesting `--token` if a JWT API token was supplied.
///
/// JWTs and Bencher API keys are structurally disjoint, so a value that fails to
/// parse as a `BencherKey` but parses as a `Jwt` is unambiguously the wrong flag.
fn parse_key(value: &str) -> Result<BencherKey, String> {
    value.parse::<BencherKey>().map_err(|err| {
        if value.parse::<Jwt>().is_ok() {
            "You supplied a JWT API token to `--key`/`BENCHER_API_KEY`. \
             Use `--token`/`BENCHER_API_TOKEN` instead."
                .to_owned()
        } else {
            err.to_string()
        }
    })
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
            #[expect(clippy::unreachable, reason = "clap ArgGroup prevents both flags")]
            (true, true) => unreachable!("Cannot set both `archive` and `unarchive`"),
        }
    }
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

#[cfg(test)]
mod tests {
    use super::{parse_key, parse_token};

    const VALID_USER_KEY: &str = "bencher_user_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh";
    const VALID_PROJECT_KEY: &str = "bencher_run_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh";
    const VALID_JWT: &str = "eyJ0eXAiOiJKV1QiLCJhbGciOiJIUzI1NiJ9.eyJhdWQiOiJhdXRoIiwiZXhwIjoxNjY5Mjk5NjExLCJpYXQiOjE2NjkyOTc4MTEsImlzcyI6ImJlbmNoZXIuZGV2Iiwic3ViIjoiYUBhLmNvIiwib3JnIjpudWxsfQ.jJmb_nCVJYLD5InaIxsQfS7x87fUsnCYpQK9SrWrKTc";

    #[test]
    fn parse_token_accepts_jwt() {
        assert_eq!(parse_token(VALID_JWT).unwrap().as_ref(), VALID_JWT);
    }

    #[test]
    fn parse_token_suggests_key_for_user_key() {
        let err = parse_token(VALID_USER_KEY).unwrap_err();
        assert!(err.contains("Use `--key`/`BENCHER_API_KEY`"), "{err}");
    }

    #[test]
    fn parse_token_suggests_key_for_project_key() {
        let err = parse_token(VALID_PROJECT_KEY).unwrap_err();
        assert!(err.contains("Use `--key`/`BENCHER_API_KEY`"), "{err}");
    }

    #[test]
    fn parse_token_rejects_garbage_without_hint() {
        let err = parse_token("garbage").unwrap_err();
        assert!(!err.contains("Use `--key`"), "{err}");
        assert!(err.contains("JWT"), "{err}");
    }

    #[test]
    fn parse_key_accepts_user_key() {
        assert_eq!(parse_key(VALID_USER_KEY).unwrap().as_ref(), VALID_USER_KEY);
    }

    #[test]
    fn parse_key_accepts_project_key() {
        assert_eq!(
            parse_key(VALID_PROJECT_KEY).unwrap().as_ref(),
            VALID_PROJECT_KEY
        );
    }

    #[test]
    fn parse_key_suggests_token_for_jwt() {
        let err = parse_key(VALID_JWT).unwrap_err();
        assert!(err.contains("Use `--token`/`BENCHER_API_TOKEN`"), "{err}");
    }

    #[test]
    fn parse_key_rejects_garbage_without_hint() {
        let err = parse_key("garbage").unwrap_err();
        assert!(!err.contains("Use `--token`"), "{err}");
        assert!(err.contains("Bencher API key"), "{err}");
    }
}
