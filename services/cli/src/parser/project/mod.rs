use bencher_json::{ResourceId, ResourceName, Slug, Url};
use clap::{Parser, Subcommand, ValueEnum};

use crate::parser::CliBackend;

use super::CliPagination;

pub mod alert;
pub mod benchmark;
pub mod branch;
pub mod measure;
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
    // Update a project
    #[clap(alias = "edit")]
    Update(CliProjectUpdate),
    // Delete a project
    #[clap(alias = "rm")]
    Delete(CliProjectDelete),
    /// Check project permission
    Allowed(CliProjectAllowed),
}

#[derive(Parser, Debug)]
pub struct CliProjectList {
    /// Organization slug or UUID
    #[clap(long)]
    pub org: Option<ResourceId>,

    /// Project name
    #[clap(long)]
    pub name: Option<ResourceName>,

    /// Project search string
    #[clap(long)]
    pub search: Option<String>,

    #[clap(flatten)]
    pub pagination: CliPagination<CliProjectsSort>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
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
    pub name: ResourceName,

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
    /// Project slug or UUID
    pub project: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]

pub struct CliProjectUpdate {
    /// Project slug or UUID
    pub project: ResourceId,

    /// Project name
    #[clap(long)]
    pub name: Option<ResourceName>,

    /// Project slug
    #[clap(long)]
    pub slug: Option<Slug>,

    #[allow(clippy::option_option)]
    /// Project URL (null to remove)
    #[clap(long)]
    pub url: Option<Option<Url>>,

    /// Project visibility (default public)
    #[clap(long)]
    pub visibility: Option<CliProjectVisibility>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliProjectDelete {
    /// Project slug or UUID
    pub project: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliProjectAllowed {
    /// Project slug or UUID
    pub project: ResourceId,

    /// Project permission
    #[clap(long)]
    pub perm: CliProjectPermission,

    #[clap(flatten)]
    pub backend: CliBackend,
}

/// Project permission
#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliProjectPermission {
    View,
    Create,
    Edit,
    Delete,
    Manage,
    ViewRole,
    CreateRole,
    EditRole,
    DeleteRole,
}
