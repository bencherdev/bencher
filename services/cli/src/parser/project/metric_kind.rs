use bencher_json::{NonEmpty, ResourceId, Slug};
use clap::{Parser, Subcommand, ValueEnum};

use crate::parser::{CliBackend, CliPagination};

#[derive(Subcommand, Debug)]
pub enum CliMetricKind {
    /// List metric kinds
    #[clap(alias = "ls")]
    List(CliMetricKindList),
    /// Create a metric kind
    #[clap(alias = "add")]
    Create(CliMetricKindCreate),
    // Update a metric kind
    #[clap(alias = "edit")]
    Update(CliMetricKindUpdate),
    /// View a metric kind
    View(CliMetricKindView),
}

#[derive(Parser, Debug)]
pub struct CliMetricKindList {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Metric kind name
    #[clap(long)]
    pub name: Option<NonEmpty>,

    #[clap(flatten)]
    pub pagination: CliPagination<CliMetricKindsSort>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliMetricKindsSort {
    /// Name of the metric kind
    Name,
}

#[derive(Parser, Debug)]
pub struct CliMetricKindCreate {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Metric kind name
    pub name: NonEmpty,

    /// Metric kind slug
    #[clap(long)]
    pub slug: Option<Slug>,

    /// Metric kind unit of measure
    #[clap(long)]
    pub units: NonEmpty,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliMetricKindView {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Metric kind slug or UUID
    pub metric_kind: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliMetricKindUpdate {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Metric kind slug or UUID
    pub metric_kind: ResourceId,

    /// Metric kind name
    #[clap(long)]
    pub name: Option<NonEmpty>,

    /// Metric kind slug
    #[clap(long)]
    pub slug: Option<Slug>,

    /// Metric kind unit of measure
    #[clap(long)]
    pub units: Option<NonEmpty>,

    #[clap(flatten)]
    pub backend: CliBackend,
}
