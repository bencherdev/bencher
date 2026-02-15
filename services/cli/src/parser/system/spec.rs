use bencher_json::{Architecture, ResourceName, SpecResourceId, SpecSlug};
use clap::{Parser, Subcommand, ValueEnum};

use crate::parser::{CliArchived, CliBackend, CliPagination};

#[derive(Subcommand, Debug)]
pub enum CliSpec {
    /// List specs
    #[clap(alias = "ls")]
    List(CliSpecList),
    /// Create a spec
    #[clap(alias = "add")]
    Create(CliSpecCreate),
    /// View a spec
    #[clap(alias = "get")]
    View(CliSpecView),
    /// Update a spec
    #[clap(alias = "edit")]
    Update(CliSpecUpdate),
    /// Delete a spec
    #[clap(alias = "rm")]
    Delete(CliSpecDelete),
}

#[derive(Parser, Debug)]
pub struct CliSpecList {
    /// Spec name
    #[clap(long)]
    pub name: Option<ResourceName>,

    /// Spec search string
    #[clap(long, value_name = "QUERY")]
    pub search: Option<String>,

    /// Include archived specs
    #[clap(long)]
    pub archived: bool,

    #[clap(flatten)]
    pub pagination: CliPagination<CliSpecsSort>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliSpecsSort {
    /// Name of the spec
    Name,
    /// Date time the spec was created
    Created,
}

#[derive(Parser, Debug)]
pub struct CliSpecCreate {
    /// Spec name
    #[clap(long)]
    pub name: ResourceName,

    /// Spec slug
    #[clap(long)]
    pub slug: Option<SpecSlug>,

    /// CPU architecture
    #[clap(long)]
    pub architecture: Architecture,

    /// Number of CPUs
    #[clap(long)]
    pub cpu: u32,

    /// Memory size in bytes
    #[clap(long)]
    pub memory: u64,

    /// Disk size in bytes
    #[clap(long)]
    pub disk: u64,

    /// Whether the VM has network access
    #[clap(long)]
    pub network: bool,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliSpecView {
    /// Spec slug or UUID
    pub spec: SpecResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliSpecUpdate {
    /// Spec slug or UUID
    pub spec: SpecResourceId,

    /// Spec name
    #[clap(long)]
    pub name: Option<ResourceName>,

    /// Spec slug
    #[clap(long)]
    pub slug: Option<SpecSlug>,

    #[clap(flatten)]
    pub archived: CliArchived,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliSpecDelete {
    /// Spec slug or UUID
    pub spec: SpecResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}
