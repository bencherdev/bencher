use bencher_json::{BranchName, GitHash, NameId, ResourceId, Slug};
use clap::{Args, Parser, Subcommand, ValueEnum};

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
    #[clap(alias = "get")]
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
    pub project: ResourceId,

    /// Branch name
    #[clap(long)]
    pub name: Option<BranchName>,

    /// Branch search string
    #[clap(long)]
    pub search: Option<String>,

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
    pub project: ResourceId,

    /// Branch name
    #[clap(long)]
    pub name: BranchName,

    /// Branch slug
    #[clap(long)]
    pub slug: Option<Slug>,

    /// Soft creation
    /// If the new branch name already exists then return the existing branch
    #[clap(long)]
    pub soft: bool,

    #[clap(flatten)]
    pub start_point: CliBranchStartPoint,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[allow(clippy::struct_field_names)]
#[derive(Args, Debug)]
pub struct CliBranchStartPoint {
    /// Branch name, slug, or UUID to use as the new branch start point
    /// https://git-scm.com/docs/git-branch#Documentation/git-branch.txt-ltstart-pointgt
    #[clap(long)]
    pub start_point_branch: Option<NameId>,

    /// Branch `git` hash to use as the new branch start point
    #[clap(long, requires = "start_point_branch")]
    pub start_point_hash: Option<GitHash>,

    /// Clone thresholds for the new branch start point
    #[clap(long, requires = "start_point_branch")]
    pub start_point_thresholds: bool,
}

#[derive(Parser, Debug)]
pub struct CliBranchView {
    /// Project slug or UUID
    pub project: ResourceId,

    /// Branch slug or UUID
    pub branch: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliBranchUpdate {
    /// Project slug or UUID
    pub project: ResourceId,

    /// Branch slug or UUID
    pub branch: ResourceId,

    /// Branch name
    #[clap(long)]
    pub name: Option<BranchName>,

    /// Branch slug
    #[clap(long)]
    pub slug: Option<Slug>,

    /// Next version of the branch `git` hash
    #[clap(long)]
    pub hash: Option<GitHash>,

    #[clap(flatten)]
    pub backend: CliBackend,
}

#[derive(Parser, Debug)]
pub struct CliBranchDelete {
    /// Project slug or UUID
    pub project: ResourceId,

    /// Branch slug or UUID
    pub branch: ResourceId,

    #[clap(flatten)]
    pub backend: CliBackend,
}
