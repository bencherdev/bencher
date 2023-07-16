use bencher_json::{BranchName, ResourceId, Slug};
use clap::{Parser, Subcommand, ValueEnum};

use crate::parser::{CliBackend, CliPagination};

#[derive(Subcommand, Debug)]
pub enum CliBranch {
    /// List branches
    #[clap(alias = "ls")]
    List(CliBranchList),
    /// Create a branch
    #[clap(alias = "add")]
    Create(CliBranchCreate),
    /// View a branch
    #[clap(alias = "cat")]
    View(CliBranchView),
    // Update a branch
    #[clap(alias = "edit")]
    Update(CliBranchUpdate),
    /// Delete a branch
    #[clap(alias = "rm")]
    Delete(CliBranchDelete),
}

#[derive(Parser, Debug)]
pub struct CliBranchList {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Branch name
    #[clap(long)]
    pub name: Option<BranchName>,

    #[clap(flatten)]
    pub pagination: CliPagination<CliBranchesSort>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(ValueEnum, Debug, Clone)]
#[clap(rename_all = "snake_case")]
pub enum CliBranchesSort {
    /// Name of the branch
    Name,
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

    /// Soft creation
    /// If the new branch name already exists then return the existing branch name
    #[clap(long)]
    pub soft: bool,

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

#[derive(Parser, Debug)]
pub struct CliBranchUpdate {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Branch slug or UUID
    pub branch: ResourceId,

    /// Branch name
    #[clap(long)]
    pub name: Option<BranchName>,

    /// Branch slug
    #[clap(long)]
    pub slug: Option<Slug>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliBranchDelete {
    /// Project slug or UUID
    #[clap(long)]
    pub project: ResourceId,

    /// Branch slug or UUID
    pub branch: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}
