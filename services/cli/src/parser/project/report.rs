use bencher_json::{ReportUuid, ResourceId};
use clap::{Parser, Subcommand, ValueEnum};

use super::run::CliRun;
use crate::parser::{CliBackend, CliPagination};

#[derive(Subcommand, Debug)]
pub enum CliReport {
    /// List reports
    #[clap(alias = "ls")]
    List(CliReportList),
    /// Create a report (alias to `bencher run`)
    #[clap(alias = "add")]
    Create(Box<CliRun>),
    /// View a report
    #[clap(alias = "cat")]
    View(CliReportView),
    /// Delete a report
    #[clap(alias = "rm")]
    Delete(CliReportDelete),
}

#[derive(Parser, Debug)]
pub struct CliReportList {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Branch slug or UUID
    #[clap(long)]
    pub branch: Option<ResourceId>,

    /// Testbed slug or UUID
    #[clap(long)]
    pub testbed: Option<ResourceId>,

    /// Start time (seconds since epoch)
    #[clap(long)]
    pub start_time: Option<i64>,

    /// End time (seconds since epoch)
    #[clap(long)]
    pub end_time: Option<i64>,

    #[clap(flatten)]
    pub pagination: CliPagination<CliReportsSort>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliReportsSort {
    /// Date time of the report
    DateTime,
}

#[derive(Parser, Debug)]
pub struct CliReportView {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Report UUID
    pub report: ReportUuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliReportDelete {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Report UUID
    pub report: ReportUuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}
