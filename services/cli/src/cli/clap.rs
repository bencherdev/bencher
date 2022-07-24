use clap::{
    ArgGroup,
    Args,
    Parser,
    Subcommand,
    ValueEnum,
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

#[derive(Subcommand, Debug)]
pub enum CliAuth {
    // Create a user account
    // Signup(CliAuthSignup),
}

#[derive(Parser, Debug)]
pub struct CliRun {
    #[clap(flatten)]
    pub locality: CliLocality,

    /// Benchmark output adapter
    #[clap(value_enum, short, long)]
    pub adapter: Option<CliAdapter>,

    /// Bencher project name or ID
    #[clap(short, long)]
    pub project: Option<String>,

    /// Bencher testbed name or ID
    #[clap(short, long)]
    pub testbed: Option<String>,

    #[clap(flatten)]
    pub command: CliCommand,
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

#[derive(Args, Debug)]
pub struct CliCommand {
    #[clap(flatten)]
    pub shell: CliShell,

    /// Benchmark command
    pub cmd: Option<String>,
}

#[derive(Args, Debug)]
pub struct CliShell {
    /// Shell command path
    #[clap(short, long, requires = "cmd")]
    pub shell: Option<String>,

    /// Shell command flag
    #[clap(short, long, requires = "cmd")]
    pub flag: Option<String>,
}

/// Supported Adapters
#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliAdapter {
    /// JSON (default)
    Json,
    /// Rust `cargo bench` 🦀
    #[clap(alias("rust"), alias("rust_cargo"))]
    RustCargoBench,
}

#[derive(Subcommand, Debug)]
pub enum CliTestbed {
    /// Create a testbed
    Create(CliTestbedCreate),
}

#[derive(Parser, Debug)]
pub struct CliTestbedCreate {
    /// Testbed name
    pub name: String,

    /// Testbed OS
    #[clap(long)]
    pub os_name: Option<String>,

    /// Testbed OS Version
    #[clap(long)]
    pub os_version: Option<String>,

    /// Testbed CPU
    #[clap(long)]
    pub cpu: Option<String>,

    /// Testbed RAM
    #[clap(long)]
    pub ram: Option<String>,

    /// Testbed Disk
    #[clap(long)]
    pub disk: Option<String>,

    #[clap(flatten)]
    pub backend: CliBackend,
}
