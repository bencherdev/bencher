use bencher_json::{ResourceId, StatisticUuid};
use clap::{Parser, Subcommand};

use crate::parser::CliBackend;

#[derive(Subcommand, Debug)]
pub enum CliStatistic {
    /// View a statistic
    #[clap(alias = "get")]
    View(CliStatisticView),
}

#[derive(Parser, Debug)]
pub struct CliStatisticView {
    /// Project slug or UUID
    pub project: ResourceId,

    /// Statistic UUID
    pub statistic: StatisticUuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}
