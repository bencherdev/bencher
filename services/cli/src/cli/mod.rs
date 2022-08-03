use clap::{
    ArgGroup,
    Args,
    Parser,
    Subcommand,
};

pub mod auth;
pub mod branch;
pub mod project;
pub mod run;
pub mod testbed;

use auth::CliAuth;
use branch::CliBranch;
use project::CliProject;
use run::CliRun;
use testbed::CliTestbed;

/// Time Series Benchmarking
#[derive(Parser, Debug)]
#[clap(author, version, about, long_about = None)]
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
    /// Run benchmarks
    Run(CliRun),
    /// Manage projects
    #[clap(subcommand)]
    Project(CliProject),
    /// Manage Branches
    #[clap(subcommand)]
    Branch(CliBranch),
    /// Manage testbeds
    #[clap(subcommand)]
    Testbed(CliTestbed),
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
    /// User API token
    #[clap(long)]
    pub token: Option<String>,

    /// Backend host URL (default http://api.bencher.dev)
    #[clap(long)]
    pub host: Option<String>,
}
