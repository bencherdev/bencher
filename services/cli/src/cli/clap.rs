use clap::{Args, Parser, Subcommand, ValueEnum};

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
pub struct CliWide {
    /// User email
    #[clap(short, long)]
    pub email: String,

    /// User API token
    #[clap(short, long)]
    pub token: Option<String>,

    /// Backend URL
    #[clap(short, long)]
    pub url: Option<String>,
}

#[derive(Subcommand, Debug)]
pub enum CliSub {
    /// Run a benchmark
    Run(CliRun),
}

#[derive(Parser, Debug)]
pub struct CliRun {
    /// Shell to run benchmark command
    #[clap(flatten)]
    pub shell: CliShell,

    /// Benchmark output adapter
    #[clap(value_enum, short, long)]
    pub adapter: Option<CliAdapter>,

    /// Benchmark project name
    #[clap(short, long)]
    pub project: Option<String>,

    /// Benchmark testbed
    #[clap(flatten)]
    pub testbed: CliTestbed,

    /// Benchmark command
    pub cmd: String,
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
#[clap(rename_all = snake_case)]
pub enum CliAdapter {
    /// JSON (default)
    Json,
    /// Rust `cargo bench` 🦀
    #[clap(alias("rust"), alias("rust_cargo"))]
    RustCargoBench,
}

#[derive(Args, Debug)]
pub struct CliTestbed {
    /// Testbed ID
    #[clap(long)]
    pub testbed: Option<String>,

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
