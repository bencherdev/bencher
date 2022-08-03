use bencher_json::ResourceId;
use clap::{
    Parser,
    Subcommand,
};

use super::CliBackend;

#[derive(Subcommand, Debug)]
pub enum CliTestbed {
    /// List testbeds
    #[clap(alias = "ls")]
    List(CliTestbedList),
    /// Create a testbed
    #[clap(alias = "add")]
    Create(CliTestbedCreate),
    /// View a testbed
    View(CliTestbedView),
}

#[derive(Parser, Debug)]
pub struct CliTestbedList {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliTestbedCreate {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Testbed name
    pub name: String,

    /// Testbed slug
    #[clap(long)]
    pub slug: Option<String>,

    /// Testbed OS
    #[clap(long)]
    pub os_name: Option<String>,

    /// Testbed OS Version
    #[clap(long, requires = "os-name")]
    pub os_version: Option<String>,

    /// Testbed Runtime
    #[clap(long)]
    pub runtime_name: Option<String>,

    /// Testbed Runtime Version
    #[clap(long, requires = "runtime-name")]
    pub runtime_version: Option<String>,

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

#[derive(Parser, Debug)]
pub struct CliTestbedView {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Testbed slug or UUID
    pub testbed: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}
