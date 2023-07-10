use bencher_json::ResourceId;
use clap::{Parser, Subcommand, ValueEnum};
use uuid::Uuid;

use crate::cli::{CliBackend, CliPagination};

#[derive(Subcommand, Debug)]
pub enum CliThreshold {
    /// List thresholds
    #[clap(alias = "ls")]
    List(CliThresholdList),
    /// Create a threshold
    #[clap(alias = "add")]
    Create(CliThresholdCreate),
    /// View a threshold
    View(CliThresholdView),
}

#[derive(Parser, Debug)]
pub struct CliThresholdList {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    #[clap(flatten)]
    pub pagination: CliPagination<CliThresholdsSort>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliThresholdsSort {
    /// Creation date time of the threshold
    Created,
}

#[derive(Parser, Debug)]
pub struct CliThresholdCreate {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Metric kind slug or UUID
    #[clap(value_enum, long)]
    pub metric_kind: ResourceId,

    /// Branch slug or UUID
    #[clap(long)]
    pub branch: ResourceId,

    /// Threshold slug or UUID
    #[clap(long)]
    pub testbed: ResourceId,

    #[clap(flatten)]
    pub statistic: CliStatisticCreate,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliStatisticCreate {
    /// Statistic test kind
    #[clap(value_enum, long)]
    pub test: CliStatisticKind,

    /// Min sample size
    #[clap(long)]
    pub min_sample_size: Option<u32>,

    /// Max sample size
    #[clap(long)]
    pub max_sample_size: Option<u32>,

    /// Window size (seconds)
    #[clap(long)]
    pub window: Option<u32>,

    /// Lower statistical boundary
    #[clap(long)]
    pub lower_boundary: Option<f64>,

    /// Upper statistical boundary
    #[clap(long)]
    pub upper_boundary: Option<f64>,
}

/// Supported kinds of statistic
#[derive(ValueEnum, Debug, Clone)]
pub enum CliStatisticKind {
    /// z-score
    Z,
    /// t-test
    T,
}

#[derive(Parser, Debug)]
pub struct CliThresholdView {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Threshold UUID
    pub threshold: Uuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}
