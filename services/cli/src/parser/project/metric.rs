use bencher_json::{MetricUuid, ResourceId};
use clap::{Parser, Subcommand};

use crate::parser::CliBackend;

#[derive(Subcommand, Debug)]
pub enum CliMetric {
    /// View a metric
    #[clap(alias = "get")]
    View(CliMetricView),
}

#[derive(Parser, Debug)]
pub struct CliMetricView {
    /// Project slug or UUID
    pub project: ResourceId,

    /// Metric UUID
    pub metric: MetricUuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}
