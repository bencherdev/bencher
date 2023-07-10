use bencher_json::{NonEmpty, ResourceId, Slug, Url};
use clap::{Parser, Subcommand, ValueEnum};

use crate::cli::CliBackend;

use super::CliPagination;

pub mod alert;
pub mod benchmark;
pub mod branch;
pub mod metric_kind;
pub mod perf;
pub mod report;
pub mod run;
pub mod statistic;
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
    // Delete a project
    #[clap(alias = "rm")]
    Delete(CliProjectDelete),
}

#[derive(Parser, Debug)]
pub struct CliProjectList {
    /// Organization slug or UUID
    #[clap(long)]
    pub org: Option<ResourceId>,

    ///  Public projects only
    #[clap(long, conflicts_with = "org")]
    pub public: bool,

    /// Project name
    #[clap(long)]
    pub name: Option<NonEmpty>,

    #[clap(flatten)]
    pub pagination: CliPagination<CliProjectsSort>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliProjectsSort {
    /// Name of the project
    Name,
}

#[derive(Parser, Debug)]
pub struct CliProjectCreate {
    /// Organization slug or UUID
    #[clap(long)]
    pub org: ResourceId,

    /// Project name
    pub name: NonEmpty,

    /// Project slug
    #[clap(long)]
    pub slug: Option<Slug>,

    /// Project URL
    #[clap(long)]
    pub url: Option<Url>,

    /// Project visibility (default public)
    #[clap(long)]
    pub visibility: Option<CliProjectVisibility>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliProjectVisibility {
    /// Public Project
    Public,
    #[cfg(feature = "plus")]
    /// Private Project
    Private,
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

#[derive(Parser, Debug)]
pub struct CliProjectDelete {
    /// Organization slug or UUID
    #[clap(long)]
    pub org: ResourceId,

    /// Project slug or UUID
    pub project: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}
