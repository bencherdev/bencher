use bencher_json::{ResourceId, ResourceName, Slug};
use clap::{Parser, Subcommand, ValueEnum};

use crate::parser::{CliArchived, CliBackend, CliPagination};

#[derive(Subcommand, Debug)]
pub enum CliMeasure {
    /// List measures
    #[clap(alias = "ls")]
    List(CliMeasureList),
    /// Create a measure
    #[clap(alias = "add")]
    Create(CliMeasureCreate),
    // Update a measure
    #[clap(alias = "edit")]
    Update(CliMeasureUpdate),
    /// View a measure
    #[clap(alias = "get")]
    View(CliMeasureView),
    /// Delete a measure
    #[clap(alias = "rm")]
    Delete(CliMeasureDelete),
}

#[derive(Parser, Debug)]
pub struct CliMeasureList {
    /// Project slug or UUID
    pub project: ResourceId,

    /// Measure name
    #[clap(long)]
    pub name: Option<ResourceName>,

    /// Measure search string
    #[clap(long)]
    pub search: Option<String>,

    #[clap(flatten)]
    pub pagination: CliPagination<CliMeasuresSort>,

    /// Filter for archived measures
    #[clap(long)]
    pub archived: bool,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliMeasuresSort {
    /// Name of the measure
    Name,
}

#[derive(Parser, Debug)]
pub struct CliMeasureCreate {
    /// Project slug or UUID
    pub project: ResourceId,

    /// Measure name
    #[clap(long)]
    pub name: ResourceName,

    /// Measure slug
    #[clap(long)]
    pub slug: Option<Slug>,

    /// Units of measure
    #[clap(long)]
    pub units: ResourceName,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliMeasureView {
    /// Project slug or UUID
    pub project: ResourceId,

    /// Measure slug or UUID
    pub measure: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliMeasureUpdate {
    /// Project slug or UUID
    pub project: ResourceId,

    /// Measure slug or UUID
    pub measure: ResourceId,

    /// Measure name
    #[clap(long)]
    pub name: Option<ResourceName>,

    /// Measure slug
    #[clap(long)]
    pub slug: Option<Slug>,

    /// Units of measure
    #[clap(long)]
    pub units: Option<ResourceName>,

    #[clap(flatten)]
    pub archived: CliArchived,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliMeasureDelete {
    /// Project slug or UUID
    pub project: ResourceId,

    /// Measure slug or UUID
    pub measure: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}
