use bencher_json::{BranchName, ResourceId, Slug};
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
    pub name: BranchName,

    /// Branch slug
    #[clap(long)]
    pub slug: Option<Slug>,

    /// Branch slug or UUID to use as the new branch start point
    /// https://git-scm.com/docs/git-branch#Documentation/git-branch.txt-ltstart-pointgt
    #[clap(long)]
    pub start_point_branch: Option<ResourceId>,

    /// Clone thresholds for the new branch start point
    #[clap(long, requires = "start_point_branch")]
    pub start_point_thresholds: bool,

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
