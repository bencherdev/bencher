use bencher_json::{
    project::{branch::START_POINT_MAX_VERSIONS, report::JsonReportStartPoint},
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
}

impl StartPoint {
    pub async fn new(
        context: &ApiContext,
        query_branch: QueryBranch,
        reference_version: QueryReferenceVersion,
        max_versions: Option<u32>,
    ) -> Result<Self, HttpError> {
        let version = QueryVersion::get(conn_lock!(context), reference_version.version_id)?;
        Ok(Self {
            branch: query_branch,
            reference_version,
            version,
            max_versions,
        })
    }

    pub async fn latest_for_branch(
        context: &ApiContext,
        project_id: ProjectId,
        query_branch: QueryBranch,
        hash: Option<&GitHash>,
        max_versions: Option<u32>,
    ) -> Result<Self, HttpError> {
        let reference_version =
            QueryReferenceVersion::get_latest_for_branch(context, project_id, &query_branch, hash)
                .await?;
        Self::new(context, query_branch, reference_version, max_versions).await
    }

    pub async fn from_json(
        context: &ApiContext,
        project_id: ProjectId,
        json: JsonNewStartPoint,
    ) -> Result<Self, HttpError> {
        let JsonNewStartPoint {
            branch,
            hash,
            max_versions,
        } = json;
        let query_branch = QueryBranch::from_name_id(conn_lock!(context), project_id, &branch)?;
        Self::latest_for_branch(
            context,
            project_id,
            query_branch,
            hash.as_ref(),
            max_versions,
        )
        .await
    }

    pub async fn from_report_json(
        context: &ApiContext,
        project_id: ProjectId,
        json: Option<&JsonReportStartPoint>,
    ) -> Result<Option<Self>, HttpError> {
        // Get the new start point, if there is a branch specified.
        let Some(JsonReportStartPoint {
            branch: Some(branch),
            hash,
            max_versions,
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
