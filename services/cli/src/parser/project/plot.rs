use bencher_json::{
    BenchmarkUuid, BranchUuid, Index, MeasureUuid, PlotUuid, ResourceId, ResourceName, TestbedUuid,
    Window,
};
use clap::{Parser, Subcommand, ValueEnum};

use crate::parser::{CliBackend, CliPagination};

#[derive(Subcommand, Debug)]
pub enum CliPlot {
    /// List plots
    #[clap(alias = "ls")]
    List(CliPlotList),
    /// Create a plot
    #[clap(alias = "add")]
    Create(CliPlotCreate),
    /// View a plot
    #[clap(alias = "get")]
    View(CliPlotView),
    // Update a plot
    #[clap(alias = "edit")]
    Update(CliPlotUpdate),
    /// Delete a plot
    #[clap(alias = "rm")]
    Delete(CliPlotDelete),
}

#[derive(Parser, Debug)]
pub struct CliPlotList {
    /// Project slug or UUID
    pub project: ResourceId,

    /// Plot title
    #[clap(long)]
    pub title: Option<ResourceName>,

    /// Plot search string
    #[clap(long)]
    pub search: Option<String>,

    #[clap(flatten)]
    pub pagination: CliPagination<CliPlotsSort>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliPlotsSort {
    /// Index of the plot
    Index,
    /// Title of the plot
    Title,
}

#[derive(Parser, Debug)]
#[allow(clippy::struct_excessive_bools)]
pub struct CliPlotCreate {
    /// Project slug or UUID
    pub project: ResourceId,

    /// The index of the plot.
    /// Maximum index is 64.
    #[clap(long)]
    pub index: Option<Index>,

    /// The title of the plot.
    /// Maximum length is 64 characters.
    #[clap(long)]
    pub title: Option<ResourceName>,

    /// Display metric lower values.
    #[clap(long)]
    pub lower_value: bool,

    /// Display metric upper values.
    #[clap(long)]
    pub upper_value: bool,

    /// Display lower boundary limits.
    #[clap(long)]
    pub lower_boundary: bool,

    /// Display upper boundary limits.
    #[clap(long)]
    pub upper_boundary: bool,

    /// The x-axis to use for the plot.
    #[clap(long)]
    pub x_axis: CliXAxis,

    /// The window of time for the plot, in seconds.
    /// Metrics outside of this window will be omitted.
    #[clap(long)]
    pub window: Window,

    /// The branches to include in the plot.
    /// At least one branch must be specified.
    #[clap(long, required = true)]
    pub branches: Vec<BranchUuid>,

    /// The testbeds to include in the plot.
    /// At least one testbed must be specified.
    #[clap(long, required = true)]
    pub testbeds: Vec<TestbedUuid>,

    /// The benchmarks to include in the plot.
    /// At least one benchmark must be specified.
    #[clap(long, required = true)]
    pub benchmarks: Vec<BenchmarkUuid>,

    /// The measures to include in the plot.
    /// At least one measure must be specified.
    #[clap(long, required = true)]
    pub measures: Vec<MeasureUuid>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

/// Supported X-Axises
#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliXAxis {
    DateTime,
    Version,
}

#[derive(Parser, Debug)]
pub struct CliPlotView {
    /// Project slug or UUID
    pub project: ResourceId,

    /// Plot UUID
    pub plot: PlotUuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliPlotUpdate {
    /// Project slug or UUID
    pub project: ResourceId,

    /// Plot UUID
    pub plot: PlotUuid,

    /// The new index for the plot.
    /// Maximum index is 64.
    #[clap(long)]
    pub index: Option<Index>,

    /// The new title of the plot.
    /// Set to `null` to remove the current title.
    /// Maximum length is 64 characters.
    /// (null to remove)
    #[clap(long)]
    #[allow(clippy::option_option)]
    pub title: Option<Option<ResourceName>>,

    /// The window of time for the plot, in seconds.
    /// Metrics outside of this window will be omitted.
    #[clap(long)]
    pub window: Option<Window>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliPlotDelete {
    /// Project slug or UUID
    pub project: ResourceId,

    /// Plot UUID
    pub plot: PlotUuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}
