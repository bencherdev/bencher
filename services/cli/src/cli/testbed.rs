use clap::{
    Parser,
    Subcommand,
};

use super::CliBackend;

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
