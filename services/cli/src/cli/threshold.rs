use bencher_json::ResourceId;
use clap::{
    Parser,
    Subcommand,
    ValueEnum,
};
use uuid::Uuid;

use super::CliBackend;

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
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliThresholdCreate {
    /// Branch UUID
    #[clap(long)]
    pub branch: Uuid,

    /// Threshold UUID
    #[clap(long)]
    pub testbed: Uuid,

    #[clap(flatten)]
    pub statistic: CliStatisticCreate,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliStatisticCreate {
    /// Statistic kind
    #[clap(value_enum, long)]
    pub kind: CliStatisticKind,

    /// Limit sample size
    #[clap(long)]
    pub sample_size: Option<u32>,

    /// Limit sampling window in seconds
    #[clap(long)]
    pub window: Option<u32>,

    /// Left side percentage
    #[clap(long)]
    pub left_side: Option<f32>,

    /// Right side percentage
    #[clap(long)]
    pub right_side: Option<f32>,
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
