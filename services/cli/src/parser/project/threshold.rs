use bencher_json::{Boundary, ModelUuid, NameId, ResourceId, SampleSize, ThresholdUuid, Window};
use clap::{ArgGroup, Args, Parser, Subcommand, ValueEnum};

use crate::parser::{CliBackend, CliPagination};

#[derive(Subcommand, Debug)]
pub enum CliThreshold {
    /// List thresholds
    #[clap(alias = "ls")]
    List(CliThresholdList),
    /// Create a threshold
    #[clap(alias = "add")]
    Create(CliThresholdCreate),
    /// View a threshold
    #[clap(alias = "get")]
    View(CliThresholdView),
    // Update a threshold
    #[clap(alias = "edit")]
    Update(CliThresholdUpdate),
    /// Delete a threshold
    #[clap(alias = "rm")]
    Delete(CliThresholdDelete),
}

#[derive(Parser, Debug)]
pub struct CliThresholdList {
    /// Project slug or UUID
    pub project: ResourceId,

    /// Branch name, slug, or UUID
    #[clap(long)]
    pub branch: Option<NameId>,

    /// Testbed name, slug, or UUID
    #[clap(long)]
    pub testbed: Option<NameId>,

    /// Measure name, slug, or UUID
    #[clap(long)]
    pub measure: Option<NameId>,

    #[clap(flatten)]
    pub pagination: CliPagination<CliThresholdsSort>,

    /// Filter for thresholds with an archived branch, testbed, or measure
    #[clap(long)]
    pub archived: bool,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliThresholdsSort {
    /// Creation date time of the threshold
    Created,
    /// Modification date time of the threshold
    Modified,
}

#[derive(Parser, Debug)]
pub struct CliThresholdCreate {
    #[clap(flatten)]
    pub project: CliThresholdCreateProject,

    /// Branch name, slug, or UUID
    #[clap(long)]
    pub branch: NameId,

    /// Testbed name, slug, or UUID
    #[clap(long)]
    pub testbed: NameId,

    /// Measure name, slug, or UUID
    #[clap(long)]
    pub measure: NameId,

    #[clap(flatten)]
    pub model: CliModel,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Args, Debug)]
#[clap(group(
    ArgGroup::new("threshold_create_project")
        .required(true)
        .multiple(false)
        .args(&["threshold_project", "project"]),
))]
pub struct CliThresholdCreateProject {
    /// Project slug or UUID
    pub threshold_project: Option<ResourceId>,
    /// Project slug or UUID.
    /// Deprecated: Set the project as the first argument instead.
    // TODO remove in due time
    #[clap(long, env = "BENCHER_PROJECT")]
    pub project: Option<ResourceId>,
}

#[derive(Parser, Debug)]
pub struct CliModel {
    /// Threshold model test
    #[clap(value_enum, long)]
    pub test: CliModelTest,

    /// Min sample size
    #[clap(long, value_name = "SAMPLE_SIZE")]
    pub min_sample_size: Option<SampleSize>,

    /// Max sample size
    #[clap(long, value_name = "SAMPLE_SIZE")]
    pub max_sample_size: Option<SampleSize>,

    /// Window size (seconds)
    #[clap(long, value_name = "SECONDS")]
    pub window: Option<Window>,

    /// Lower boundary
    #[clap(long, value_name = "BOUNDARY")]
    pub lower_boundary: Option<Boundary>,

    /// Upper boundary
    #[clap(long, value_name = "BOUNDARY")]
    pub upper_boundary: Option<Boundary>,
}

/// Supported threshold model tests
#[derive(ValueEnum, Debug, Clone, Copy)]
#[clap(rename_all = "snake_case")]
pub enum CliModelTest {
    /// Static value
    Static,
    /// Percentage change from mean
    Percentage,
    /// z-score (normal distribution)
    #[clap(alias = "z")]
    ZScore,
    /// t-test (normal distribution)
    #[clap(alias = "t")]
    TTest,
    /// Log normal distribution
    LogNormal,
    /// Interquartile range (IQR)
    Iqr,
    /// Delta interquartile range (ΔIQR)
    DeltaIqr,
}

#[derive(Parser, Debug)]
pub struct CliThresholdView {
    /// Project slug or UUID
    pub project: ResourceId,

    /// Threshold UUID
    pub threshold: ThresholdUuid,

    /// Specify the threshold model to view
    #[clap(long)]
    pub model: Option<ModelUuid>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliThresholdUpdate {
    /// Project slug or UUID
    pub project: ResourceId,

    /// Threshold UUID
    pub threshold: ThresholdUuid,

    #[clap(flatten)]
    pub model: CliUpdateModel,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
#[clap(group(
    ArgGroup::new("update_model")
        .required(true)
        .multiple(false)
        .args(&["test", "remove_model"]),
))]
pub struct CliUpdateModel {
    /// Threshold model test
    #[clap(value_enum, long)]
    pub test: Option<CliModelTest>,

    /// Min sample size
    #[clap(long, requires = "test", value_name = "SAMPLE_SIZE")]
    pub min_sample_size: Option<SampleSize>,

    /// Max sample size
    #[clap(long, requires = "test", value_name = "SAMPLE_SIZE")]
    pub max_sample_size: Option<SampleSize>,

    /// Window size (seconds)
    #[clap(long, requires = "test", value_name = "SECONDS")]
    pub window: Option<Window>,

    /// Lower boundary
    #[clap(long, requires = "test", value_name = "BOUNDARY")]
    pub lower_boundary: Option<Boundary>,

    /// Upper boundary
    #[clap(long, requires = "test", value_name = "BOUNDARY")]
    pub upper_boundary: Option<Boundary>,

    /// Remove the threshold model
    #[clap(long)]
    pub remove_model: bool,
}

#[derive(Parser, Debug)]
pub struct CliThresholdDelete {
    /// Project slug or UUID
    pub project: ResourceId,

    /// Threshold UUID
    pub threshold: ThresholdUuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}
