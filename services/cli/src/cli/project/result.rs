use bencher_json::ResourceId;
use clap::{Parser, Subcommand};
use uuid::Uuid;

use crate::cli::CliBackend;

#[derive(Subcommand, Debug)]
pub enum CliResult {
    /// View a result
    View(CliResultView),
}

#[derive(Parser, Debug)]
pub struct CliResultView {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Result UUID
    pub result: Uuid,

    #[clap(flatten)]
    pub backend: CliBackend,
}
