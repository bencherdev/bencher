#![cfg(feature = "plus")]

use bencher_json::{JobUuid, ProjectResourceId};
use clap::{Parser, Subcommand, ValueEnum};

use crate::parser::{CliBackend, CliPagination};

#[derive(Subcommand, Debug)]
pub enum CliJob {
    /// List jobs
    #[clap(alias = "ls")]
    List(CliJobList),
    /// View a job
    #[clap(alias = "get")]
    View(CliJobView),
}

#[derive(Parser, Debug)]
pub struct CliJobList {
    /// Project slug or UUID
    pub project: ProjectResourceId,

    /// Filter by job status
    #[clap(value_enum, long)]
    pub status: Option<CliJobStatus>,

    #[clap(flatten)]
    pub pagination: CliPagination<CliJobsSort>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone, Copy)]
#[clap(rename_all = "snake_case")]
pub enum CliJobsSort {
    /// Date time the job was created
    Created,
}

/// Job status
#[derive(ValueEnum, Debug, Clone, Copy)]
#[clap(rename_all = "snake_case")]
pub enum CliJobStatus {
    Pending,
    Claimed,
    Running,
    Completed,
    Failed,
    Canceled,
}

#[derive(Parser, Debug)]
pub struct CliJobView {
    /// Project slug or UUID
    pub project: ProjectResourceId,

    /// Job UUID
    pub job: JobUuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}
