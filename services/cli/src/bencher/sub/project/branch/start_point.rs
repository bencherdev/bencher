use bencher_client::types::JsonUpdateStartPoint;
use bencher_json::{BranchNameId, GitHash};

use crate::parser::project::branch::CliStartPointUpdate;

#[derive(Debug, Clone)]
pub struct StartPoint {
    pub branch: Option<BranchNameId>,
    pub hash: Option<GitHash>,
    pub max_versions: u32,
    pub clone_thresholds: bool,
    pub reset: bool,
}

impl From<CliStartPointUpdate> for StartPoint {
    fn from(start_point: CliStartPointUpdate) -> Self {
        let CliStartPointUpdate {
            start_point_branch,
            start_point_hash,
            start_point_max_versions,
            start_point_clone_thresholds,
            start_point_reset,
        } = start_point;
        Self {
            branch: start_point_branch,
            hash: start_point_hash,
            max_versions: start_point_max_versions,
            clone_thresholds: start_point_clone_thresholds,
            reset: start_point_reset,
        }
    }
}

impl From<StartPoint> for Option<JsonUpdateStartPoint> {
    fn from(start_point: StartPoint) -> Self {
        let StartPoint {
            branch,
            hash,
            max_versions,
            clone_thresholds,
            reset,
        } = start_point;
        (branch.is_some() || reset).then(|| JsonUpdateStartPoint {
            branch: branch.map(Into::into),
            hash: hash.map(Into::into),
            max_versions: Some(max_versions),
            clone_thresholds: Some(clone_thresholds),
            reset: Some(reset),
        })
    }
}
