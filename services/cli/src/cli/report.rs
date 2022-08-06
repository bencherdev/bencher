use bencher_json::ResourceId;
use clap::{
    Parser,
    Subcommand,
};

use super::CliBackend;

#[derive(Subcommand, Debug)]
pub enum CliReport {
    /// List reportes
    #[clap(alias = "ls")]
    List(CliReportList),
    // Create a report
    // #[clap(alias = "add")]
    // Create(CliReportCreate),
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
