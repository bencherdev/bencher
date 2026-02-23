use bencher_json::{OrganizationResourceId, ProjectResourceId, ProjectSlug, ResourceName, Url};
use clap::{Parser, Subcommand, ValueEnum};

use crate::parser::CliBackend;

use super::{CliPagination, ElidedOption};

pub mod alert;
pub mod archive;
pub mod benchmark;
pub mod branch;
pub mod job;
pub mod measure;
pub mod metric;
pub mod perf;
pub mod plot;
pub mod report;
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
    pub organization: Option<OrganizationResourceId>,

    /// Project name
    #[clap(long)]
    pub name: Option<ResourceName>,

    /// Project search string
    #[clap(long, value_name = "QUERY")]
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
    pub organization: OrganizationResourceId,

    /// Project name
    #[clap(long)]
    pub name: ResourceName,

    /// Project slug
    #[clap(long)]
    pub slug: Option<ProjectSlug>,

    /// Project URL
    #[clap(long)]
    pub url: Option<Url>,

    /// Project visibility
    #[clap(long, default_value = "public")]
    pub visibility: CliProjectVisibility,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliProjectVisibility {
    Public,
    #[cfg(feature = "plus")]
    Private,
}

#[derive(Parser, Debug)]
pub struct CliProjectView {
    /// Project slug or UUID
    pub project: ProjectResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]

pub struct CliProjectUpdate {
    /// Project slug or UUID
    pub project: ProjectResourceId,

    /// Project name
    #[clap(long)]
    pub name: Option<ResourceName>,

    /// Project slug
    #[clap(long)]
    pub slug: Option<ProjectSlug>,

    /// Project URL
    /// To remove the current project URL without replacing it, use an underscore (`_`).
    #[clap(long)]
    pub url: Option<ElidedOption<Url>>,

    /// Project visibility
    #[clap(long)]
    pub visibility: Option<CliProjectVisibility>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliProjectDelete {
    /// Project slug or UUID
    pub project: ProjectResourceId,

    /// Hard delete the project (requires server admin)
    #[clap(long)]
    pub hard: bool,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliProjectAllowed {
    /// Project slug or UUID
    pub project: ProjectResourceId,

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
