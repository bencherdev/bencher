use bencher_json::ResourceId;
use clap::{
    Parser,
    Subcommand,
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
    pub backend: CliBackend,
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
