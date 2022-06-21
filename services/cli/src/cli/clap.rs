use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};

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
    /// Run a benchmark
    Run(CliRun),
    /// Manage testbeds
    Testbed(CliTestbed),
}

#[derive(Parser, Debug)]
pub struct CliRun {
    /// Shell to run benchmark command
    #[clap(flatten)]
    pub locality: CliLocality,

    /// Shell to run benchmark command
    #[clap(flatten)]
    pub shell: CliShell,

    /// Benchmark output adapter
    #[clap(value_enum, short, long)]
    pub adapter: Option<CliAdapter>,

    /// Bencher project name or ID
    #[clap(short, long)]
    pub project: Option<String>,

    /// Bencher testbed name or ID
    #[clap(short, long)]
    pub testbed: Option<String>,

    /// Benchmark command
    pub cmd: String,
}

#[derive(Args, Debug)]
pub struct CliLocality {
    /// Run local only
    #[clap(short, long)]
    pub local: bool,

    /// Backend config
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
pub struct CliShell {
    /// Shell command path
    #[clap(short, long)]
    pub shell: Option<String>,

    /// Shell command flag
    #[clap(short, long)]
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

// TODO flesh this out as a subcommand with its own subcommands
// `bencher testbed ls`, `add`, `update`, `delete`, etc
#[derive(Parser, Debug)]
pub struct CliTestbed {
    /// Testbed ID
    #[clap(long)]
    pub name: Option<String>,

    /// Testbed OS
    #[clap(long, requires = "testbed")]
    pub os: Option<String>,

    /// Testbed OS Version
    #[clap(long, requires = "testbed")]
    pub os_version: Option<String>,

    /// Testbed CPU
    #[clap(long, requires = "testbed")]
    pub cpu: Option<String>,

    /// Testbed RAM
    #[clap(long, requires = "testbed")]
    pub ram: Option<String>,

    /// Testbed Disk Size
    #[clap(long, requires = "testbed")]
    pub disk: Option<String>,

    /// Testbed Architecture
    #[clap(long, requires = "testbed")]
    pub arch: Option<String>,
}
