use clap::{ArgGroup, Args, Parser, Subcommand};

pub mod organization;
pub mod project;
pub mod system;
pub mod user;

use organization::{member::CliMember, CliOrganization};
use project::{
    alert::CliAlert, benchmark::CliBenchmark, branch::CliBranch, perf::CliPerf, report::CliReport,
    run::CliRun, testbed::CliTestbed, threshold::CliThreshold, CliProject,
};
use system::{auth::CliAuth, server::CliServer};
use user::token::CliToken;

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
    /// Server commands
    #[clap(subcommand)]
    Server(CliServer),

    /// Backend authentication
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
    /// View alerts
    #[clap(subcommand)]
    Alert(CliAlert),

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
