use bencher_json::ResourceId;
use clap::{Parser, Subcommand};
use uuid::Uuid;

use crate::parser::CliBackend;

#[derive(Subcommand, Debug)]
pub enum CliStatistic {
    /// View a statistic
    #[clap(alias = "cat")]
    View(CliStatisticView),
}

#[derive(Parser, Debug)]
pub struct CliStatisticView {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Statistic UUID
    pub statistic: Uuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}
