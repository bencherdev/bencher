use bencher_json::{ProjectResourceId, ResourceName, TestbedResourceId, TestbedSlug};
use clap::{Parser, Subcommand, ValueEnum};

use crate::parser::{CliArchived, CliBackend, CliPagination};

#[derive(Subcommand, Debug)]
pub enum CliTestbed {
    /// List testbeds
    #[clap(alias = "ls")]
    List(CliTestbedList),
    /// Create a testbed
    #[clap(alias = "add")]
    Create(CliTestbedCreate),
    /// View a testbed
    #[clap(alias = "get")]
    View(CliTestbedView),
    // Update a testbed
    #[clap(alias = "edit")]
    Update(CliTestbedUpdate),
    /// Delete a testbed
    #[clap(alias = "rm")]
    Delete(CliTestbedDelete),
}

#[derive(Parser, Debug)]
pub struct CliTestbedList {
    /// Project slug or UUID
    pub project: ProjectResourceId,

    /// Testbed name
    #[clap(long)]
    pub name: Option<ResourceName>,

    /// Testbed search string
    #[clap(long, value_name = "QUERY")]
    pub search: Option<String>,

    #[clap(flatten)]
    pub pagination: CliPagination<CliTestbedsSort>,

    /// Filter for archived testbeds
    #[clap(long)]
    pub archived: bool,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliTestbedsSort {
    /// Name of the testbed
    Name,
}

#[derive(Parser, Debug)]
pub struct CliTestbedCreate {
    /// Project slug or UUID
    pub project: ProjectResourceId,

    /// Testbed name
    #[clap(long)]
    pub name: ResourceName,

    /// Testbed slug
    #[clap(long)]
    pub slug: Option<TestbedSlug>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliTestbedView {
    /// Project slug or UUID
    pub project: ProjectResourceId,

    /// Testbed slug or UUID
    pub testbed: TestbedResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliTestbedUpdate {
    /// Project slug or UUID
    pub project: ProjectResourceId,

    /// Testbed slug or UUID
    pub testbed: TestbedResourceId,

    /// Testbed name
    #[clap(long)]
    pub name: Option<ResourceName>,

    /// Testbed slug
    #[clap(long)]
    pub slug: Option<TestbedSlug>,

    #[clap(flatten)]
    pub archived: CliArchived,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliTestbedDelete {
    /// Project slug or UUID
    pub project: ProjectResourceId,

    /// Testbed slug or UUID
    pub testbed: TestbedResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}
