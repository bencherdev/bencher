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
    clone_thresholds: bool,
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
            start_point,
            start_point_hash,
            start_point_max_versions,
            start_point_clone_thresholds,
            start_point_reset,
            deprecated: _,
        } = run_branch;
        let branch = try_branch(branch)?;
        let hash = map_hash(hash);
        let start_point = map_start_point(
            start_point,
            start_point_hash,
            start_point_max_versions,
            start_point_clone_thresholds,
            start_point_reset,
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
    start_point: Vec<String>,
    start_point_hash: Option<GitHash>,
    start_point_max_versions: u32,
    start_point_clone_thresholds: bool,
    start_point_reset: bool,
) -> Option<StartPoint> {
    let branch = start_point.first().and_then(|b| {
        // The only invalid `NameId` is an empty string.
        // This allows for "continue on empty" semantics for the branch start point.
        b.parse().ok()
    });
    (branch.is_some() || start_point_reset).then_some(StartPoint {
        branch,
        hash: start_point_hash,
        max_versions: start_point_max_versions,
        clone_thresholds: start_point_clone_thresholds,
        reset: start_point_reset,
    })
}

impl From<Branch>
    for (
        bencher_client::types::NameId,
        Option<bencher_client::types::GitHash>,
        Option<bencher_client::types::JsonUpdateStartPoint>,
    )
{
    fn from(branch: Branch) -> Self {
        let name = branch.branch.into();
        let hash = branch.hash.map(Into::into);
        let start_point =
            branch
                .start_point
                .map(|start_point| bencher_client::types::JsonUpdateStartPoint {
                    branch: start_point.branch.map(Into::into),
                    hash: start_point.hash.map(Into::into),
                    max_versions: Some(start_point.max_versions),
                    clone_thresholds: Some(start_point.clone_thresholds),
                    reset: Some(start_point.reset),
                });
        (name, hash, start_point)
    }
}
