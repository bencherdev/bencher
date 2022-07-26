use clap::{
    ArgGroup,
    Args,
    Parser,
    Subcommand,
};

mod auth;
mod run;
mod testbed;

pub use auth::{
    CliAuth,
    CliAuthLogin,
    CliAuthSignup,
};
pub use run::{
    CliAdapter,
    CliCommand,
    CliRun,
    CliShell,
};
pub use testbed::{
    CliTestbed,
    CliTestbedCreate,
};

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
    /// Run a benchmark
    Run(CliRun),
    /// Manage testbeds
    #[clap(subcommand)]
    Testbed(CliTestbed),
}

#[derive(Args, Debug)]
pub struct CliLocality {
    /// Run local only
    #[clap(short, long)]
    pub local: bool,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Args, Debug)]
#[clap(group(
    ArgGroup::new("backend")
        .multiple(true)
        .conflicts_with("local")
        .args(&["email", "token", "url"]),
))]
pub struct CliBackend {
    /// User email
    #[clap(long)]
    pub email: Option<String>,

    /// User API token
    #[clap(long)]
    pub token: Option<String>,

    /// Backend host URL (default http://api.bencher.dev)
    #[clap(long)]
    pub url: Option<String>,
}
