use bencher_json::ResourceId;
use clap::{Parser, Subcommand};

use crate::cli::CliBackend;

#[derive(Subcommand, Debug)]
pub enum CliBranch {
    /// List branches
    #[clap(alias = "ls")]
    List(CliBranchList),
    /// Create a branch
    #[clap(alias = "add")]
    Create(CliBranchCreate),
    /// View a branch
    View(CliBranchView),
}

#[derive(Parser, Debug)]
pub struct CliBranchList {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliBranchCreate {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Branch name
    pub name: String,

    /// Branch slug
    #[clap(long)]
    pub slug: Option<String>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliBranchView {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Branch slug or UUID
    pub branch: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}
