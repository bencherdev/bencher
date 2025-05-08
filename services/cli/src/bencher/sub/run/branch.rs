use bencher_json::{GitHash, NameId};

use crate::{bencher::sub::project::branch::start_point::StartPoint, parser::run::CliRunBranch};

#[expect(clippy::struct_field_names)]
#[derive(Debug, Clone)]
pub struct Branch {
    branch: Option<NameId>,
    hash: Option<GitHash>,
    start_point: StartPoint,
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

#[expect(clippy::needless_pass_by_value)]
fn map_start_point(
    start_point: Vec<String>,
    start_point_hash: Option<GitHash>,
    start_point_max_versions: u32,
    start_point_clone_thresholds: bool,
    start_point_reset: bool,
) -> StartPoint {
    let branch = start_point.first().and_then(|b| {
        // The only invalid `NameId` is an empty string.
        // This allows for "continue on empty" semantics for the branch start point.
        b.parse().ok()
    });
    StartPoint {
        branch,
        hash: start_point_hash,
        max_versions: start_point_max_versions,
        clone_thresholds: start_point_clone_thresholds,
        reset: start_point_reset,
    }
}

impl From<Branch>
    for (
        Option<bencher_client::types::NameId>,
        Option<bencher_client::types::GitHash>,
        Option<bencher_client::types::JsonUpdateStartPoint>,
    )
{
    fn from(branch: Branch) -> Self {
        let name = branch.branch.map(Into::into);
        let hash = branch.hash.map(Into::into);
        let start_point = branch.start_point.into();
        (name, hash, start_point)
    }
}
