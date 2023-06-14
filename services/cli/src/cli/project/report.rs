use std::path::PathBuf;

use bencher_json::ResourceId;
use clap::{Parser, Subcommand};
use uuid::Uuid;

use super::run::CliRun;
use crate::cli::CliBackend;

#[derive(Subcommand, Debug)]
pub enum CliReport {
    /// List reports
    #[clap(alias = "ls")]
    List(CliReportList),
    /// Create a report (alias to `bencher run`)
    #[clap(alias = "add")]
    Create(Box<CliRun>),
    /// View a report
    View(CliReportView),
    /// Upload report
    Upload(CliReportUpload)
}

#[derive(Parser, Debug)]
pub struct CliReportList {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliReportView {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Report UUID
    pub report: Uuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliReportUpload {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Report UUID
    pub report: Uuid,

    #[clap(flatten)]
    pub backend: CliBackend,

    /// Perf data file path
    pub perf_data_path: PathBuf
}