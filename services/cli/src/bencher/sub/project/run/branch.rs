use bencher_json::{project::branch::BRANCH_MAIN_STR, GitHash, NameId};

use crate::parser::project::run::{CliRunBranch, CliRunHash};

#[allow(clippy::struct_field_names)]
#[derive(Debug, Clone)]
pub struct Branch {
    branch: NameId,
    hash: Option<GitHash>,
    start_point: Option<StartPoint>,
}

#[derive(Debug, Clone)]
struct StartPoint {
    branch: Option<NameId>,
    hash: Option<GitHash>,
    max_versions: u32,
    reset: bool,
}

#[derive(thiserror::Error, Debug)]
pub enum BranchError {
    #[error("Failed to parse UUID, slug, or name for the branch: {0}")]
    ParseBranch(bencher_json::ValidError),
}

impl TryFrom<CliRunBranch> for Branch {
    type Error = BranchError;

    fn try_from(run_branch: CliRunBranch) -> Result<Self, Self::Error> {
        let CliRunBranch {
            branch,
            hash,
            branch_start_point,
            branch_start_point_hash,
            branch_start_point_max_versions,
            branch_reset,
            deprecated: _,
        } = run_branch;
        let branch = try_branch(branch)?;
        let hash = map_hash(hash);
        let start_point = map_start_point(
            branch_start_point,
            branch_start_point_hash,
            branch_start_point_max_versions,
            branch_reset,
        );
        Ok(Self {
            branch,
            hash,
            start_point,
        })
    }
}

fn try_branch(branch: Option<NameId>) -> Result<NameId, BranchError> {
    if let Some(branch) = branch {
        Ok(branch)
    } else if let Some(branch) = find_branch() {
        Ok(branch)
    } else {
        BRANCH_MAIN_STR.parse().map_err(BranchError::ParseBranch)
    }
}

fn find_branch() -> Option<NameId> {
    if let Some(repo) = find_repo() {
        if let Ok(Some(branch)) = repo.head_name() {
            return branch.shorten().to_string().parse().ok();
        }
    }
    None
}

fn map_hash(CliRunHash { hash, no_hash }: CliRunHash) -> Option<GitHash> {
    if let Some(hash) = hash {
        return Some(hash);
    } else if no_hash {
        return None;
    }
    let repo = find_repo()?;
    let head_id = repo.head_id().ok()?;
    let head_object = head_id.object().ok()?;
    Some(head_object.id.into())
}

fn find_repo() -> Option<gix::Repository> {
    let current_dir = std::env::current_dir().ok()?;
    for directory in current_dir.ancestors() {
        if let Ok(repo) = gix::open(directory) {
            return Some(repo);
        }
    }
    None
}

#[allow(clippy::needless_pass_by_value)]
fn map_start_point(
    branch_start_point: Vec<String>,
    branch_start_point_hash: Option<GitHash>,
    branch_start_point_max_versions: u32,
    branch_reset: bool,
) -> Option<StartPoint> {
    let branch_start_point = branch_start_point.first().and_then(|b| {
        // The only invalid `NameId` is an empty string.
        // This allows for "continue on empty" semantics for the branch start point.
        b.parse().ok()
    });
    (branch_start_point.is_some() || branch_reset).then_some(StartPoint {
        branch: branch_start_point,
        hash: branch_start_point_hash,
        max_versions: branch_start_point_max_versions,
        reset: branch_reset,
    })
}

impl From<Branch>
    for (
        bencher_client::types::NameId,
        Option<bencher_client::types::GitHash>,
        Option<bencher_client::types::JsonReportStartPoint>,
    )
{
    fn from(branch: Branch) -> Self {
        let name = branch.branch.into();
        let hash = branch.hash.map(Into::into);
        let start_point =
            branch
                .start_point
                .map(|start_point| bencher_client::types::JsonReportStartPoint {
                    branch: start_point.branch.map(Into::into),
                    hash: start_point.hash.map(Into::into),
                    max_versions: Some(start_point.max_versions),
                    reset: Some(start_point.reset),
                });
        (name, hash, start_point)
    }
}
