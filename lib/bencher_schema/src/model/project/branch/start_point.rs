use bencher_json::{
    GitHash, JsonNewStartPoint,
    project::branch::{JsonUpdateStartPoint, START_POINT_MAX_VERSIONS},
};
use dropshot::HttpError;

use crate::{auth_conn, context::ApiContext, model::project::ProjectId};

use super::{
    QueryBranch,
    head_version::{HeadVersionId, QueryHeadVersion},
    version::QueryVersion,
};

#[derive(Debug, Clone)]
pub struct StartPoint {
    pub branch: QueryBranch,
    pub head_version: QueryHeadVersion,
    pub version: QueryVersion,
    pub max_versions: Option<u32>,
    pub clone_thresholds: Option<bool>,
}

impl StartPoint {
    pub async fn new(
        context: &ApiContext,
        query_branch: QueryBranch,
        head_version: QueryHeadVersion,
        max_versions: Option<u32>,
        clone_thresholds: Option<bool>,
    ) -> Result<Self, HttpError> {
        let version = QueryVersion::get(auth_conn!(context), head_version.version_id)?;
        Ok(Self {
            branch: query_branch,
            head_version,
            version,
            max_versions,
            clone_thresholds,
        })
    }

    pub async fn latest_for_branch(
        context: &ApiContext,
        project_id: ProjectId,
        query_branch: QueryBranch,
        hash: Option<&GitHash>,
        max_versions: Option<u32>,
        clone_thresholds: Option<bool>,
    ) -> Result<Self, HttpError> {
        let head_version =
            QueryHeadVersion::get_latest_for_branch(context, project_id, &query_branch, hash)
                .await?;
        Self::new(
            context,
            query_branch,
            head_version,
            max_versions,
            clone_thresholds,
        )
        .await
    }

    pub async fn from_new_json(
        context: &ApiContext,
        project_id: ProjectId,
        json: JsonNewStartPoint,
    ) -> Result<Self, HttpError> {
        let JsonNewStartPoint {
            branch,
            hash,
            max_versions,
            clone_thresholds,
        } = json;
        let query_branch = QueryBranch::from_name_id(auth_conn!(context), project_id, &branch)?;
        Self::latest_for_branch(
            context,
            project_id,
            query_branch,
            hash.as_ref(),
            max_versions,
            clone_thresholds,
        )
        .await
    }

    pub async fn from_update_json(
        context: &ApiContext,
        project_id: ProjectId,
        json: Option<&JsonUpdateStartPoint>,
    ) -> Result<Option<Self>, HttpError> {
        // Get the new start point, if there is a branch specified.
        let Some(JsonUpdateStartPoint {
            branch: Some(branch),
            hash,
            max_versions,
            clone_thresholds,
            reset: _,
        }) = json
        else {
            return Ok(None);
        };
        // If updating the start point, it is okay if it does not exist.
        // This avoids a race condition when creating both the branch and start point in CI.
        let Ok(query_branch) = QueryBranch::from_name_id(auth_conn!(context), project_id, branch)
        else {
            return Ok(None);
        };
        Self::latest_for_branch(
            context,
            project_id,
            query_branch,
            hash.as_ref(),
            *max_versions,
            *clone_thresholds,
        )
        .await
        .map(Some)
    }

    pub fn head_version_id(&self) -> HeadVersionId {
        self.head_version.id
    }

    pub fn max_versions(&self) -> u32 {
        self.max_versions.unwrap_or(START_POINT_MAX_VERSIONS)
    }
}
