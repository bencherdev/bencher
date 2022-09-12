use clap::{ArgGroup, Args, Parser, Subcommand};

pub mod auth;
pub mod benchmark;
pub mod branch;
pub mod organization;
pub mod perf;
pub mod project;
pub mod report;
pub mod run;
pub mod testbed;
pub mod threshold;
pub mod token;

use auth::CliAuth;
use benchmark::CliBenchmark;
use branch::CliBranch;
use organization::CliOrganization;
use perf::CliPerf;
use project::CliProject;
use report::CliReport;
use run::CliRun;
use testbed::CliTestbed;
use threshold::CliThreshold;
use token::CliToken;

/// Time Series Benchmarking
#[derive(Parser, Debug)]
#[clap(name = "bencher", author, version, about, long_about = None)]
pub struct CliBencher {
    /// Bencher CLI wide flags
    #[clap(flatten)]
    pub wide: CliWide,

    /// Bencher subcommands
    #[clap(subcommand)]
    pub sub: Option<CliSub>,
}

#[derive(Args, Debug)]
pub struct CliWide {}

#[derive(Subcommand, Debug)]
pub enum CliSub {
    /// Backend authentication
    #[clap(subcommand)]
    Auth(CliAuth),
    /// Manage organization
    #[clap(subcommand, alias = "org")]
    Organization(CliOrganization),
    /// Manage projects
    #[clap(subcommand)]
    Project(CliProject),
    /// Manage reports
    #[clap(subcommand)]
    Report(CliReport),
    /// Manage Branches
    #[clap(subcommand)]
    Branch(CliBranch),
    /// Manage testbeds
    #[clap(subcommand)]
    Testbed(CliTestbed),
    /// Manage thresholds
    #[clap(subcommand)]
    Threshold(CliThreshold),
    /// Run benchmarks
    Run(CliRun),
    /// Manage benchmarks
    #[clap(subcommand)]
    Benchmark(CliBenchmark),
    /// Query benchmark data
    Perf(CliPerf),

    /// User API tokens
    #[clap(subcommand)]
    Token(CliToken),
}

#[derive(Args, Debug)]
pub struct CliLocality {
    /// Run local only
    #[clap(long)]
    pub local: bool,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Args, Debug)]
#[clap(group(
    ArgGroup::new("backend")
        .multiple(true)
        .conflicts_with("local")
        .args(&["token", "host"]),
))]
pub struct CliBackend {
    /// Backend host URL (default https://api.bencher.dev)
    #[clap(long)]
    pub host: Option<String>,

    /// User API token
    #[clap(long)]
    pub token: Option<String>,
}
