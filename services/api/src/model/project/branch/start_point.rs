use bencher_json::{
    project::branch::{JsonUpdateStartPoint, START_POINT_MAX_VERSIONS},
    GitHash, JsonNewStartPoint,
};
use dropshot::HttpError;

use crate::{conn_lock, context::ApiContext, model::project::ProjectId};

use super::{
    reference_version::{QueryReferenceVersion, ReferenceVersionId},
    version::QueryVersion,
    QueryBranch,
};

#[derive(Debug, Clone)]
pub struct StartPoint {
    pub branch: QueryBranch,
    pub reference_version: QueryReferenceVersion,
    pub version: QueryVersion,
    pub max_versions: Option<u32>,
    pub clone_thresholds: Option<bool>,
}

impl StartPoint {
    pub async fn new(
        context: &ApiContext,
        query_branch: QueryBranch,
        reference_version: QueryReferenceVersion,
        max_versions: Option<u32>,
        clone_thresholds: Option<bool>,
    ) -> Result<Self, HttpError> {
        let version = QueryVersion::get(conn_lock!(context), reference_version.version_id)?;
        Ok(Self {
            branch: query_branch,
            reference_version,
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
        let reference_version =
            QueryReferenceVersion::get_latest_for_branch(context, project_id, &query_branch, hash)
                .await?;
        Self::new(
            context,
            query_branch,
            reference_version,
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
        let query_branch = QueryBranch::from_name_id(conn_lock!(context), project_id, &branch)?;
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
        let Ok(query_branch) = QueryBranch::from_name_id(conn_lock!(context), project_id, branch)
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

    pub fn reference_version_id(&self) -> ReferenceVersionId {
        self.reference_version.id
    }

    pub fn max_versions(&self) -> u32 {
        self.max_versions.unwrap_or(START_POINT_MAX_VERSIONS)
    }
}
