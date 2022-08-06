use bencher_json::ResourceId;
use clap::{
    Parser,
    Subcommand,
};

use super::{
    run::CliRun,
    CliBackend,
};

#[derive(Subcommand, Debug)]
pub enum CliReport {
    /// List reports
    #[clap(alias = "ls")]
    List(CliReportList),
    /// Create a report (alias to `bencher run`)
    #[clap(alias = "add")]
    Create(CliRun),
    // View a report
    // View(CliReportView),
}

#[derive(Parser, Debug)]
pub struct CliReportList {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}
