use bencher_json::ResourceId;
use clap::{Parser, Subcommand};

use crate::cli::CliBackend;

pub mod alert;
pub mod benchmark;
pub mod branch;
pub mod perf;
pub mod report;
pub mod run;
pub mod testbed;
pub mod threshold;

#[derive(Subcommand, Debug)]
pub enum CliProject {
    // List projects
    #[clap(alias = "ls")]
    List(CliProjectList),
    // Create a project
    #[clap(alias = "add")]
    Create(CliProjectCreate),
    // View a project
    View(CliProjectView),
}

#[derive(Parser, Debug)]
pub struct CliProjectList {
    /// Organization slug or UUID
    #[clap(long)]
    pub org: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliProjectCreate {
    /// Organization slug or UUID
    #[clap(long)]
    pub org: ResourceId,

    /// Project slug
    #[clap(long)]
    pub slug: Option<String>,

    /// Project description
    #[clap(long)]
    pub description: Option<String>,

    /// Project URL
    #[clap(long)]
    pub url: Option<String>,

    /// Set project as public
    #[clap(long)]
    pub public: bool,

    /// Project name
    pub name: String,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliProjectView {
    /// Organization slug or UUID
    #[clap(long)]
    pub org: Option<ResourceId>,

    /// Project slug or UUID
    pub project: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}
