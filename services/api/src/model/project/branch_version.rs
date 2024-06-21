use bencher_json::{project::branch::JsonBranchVersion, JsonBranch, JsonStartPoint};
use dropshot::HttpError;

use crate::{
    conn_lock,
    context::{ApiContext, DbConnection},
    error::{assert_parentage, BencherResource},
    schema::branch_version as branch_version_table,
    util::fn_get::fn_get,
};

use super::{
    branch::{BranchId, QueryBranch},
    version::{QueryVersion, VersionId},
    QueryProject,
};

crate::util::typed_id::typed_id!(BranchVersionId);

#[derive(Debug, diesel::Queryable)]
pub struct QueryBranchVersion {
    pub id: BranchVersionId,
    pub branch_id: BranchId,
    pub version_id: VersionId,
}

impl QueryBranchVersion {
    fn_get!(branch_version, BranchVersionId);

    pub fn into_start_point_json(
        self,
        conn: &mut DbConnection,
    ) -> Result<JsonStartPoint, HttpError> {
        Ok(JsonStartPoint {
            branch: QueryBranch::get_uuid(conn, self.branch_id)?,
            version: QueryVersion::get(conn, self.version_id)?.into_json(),
        })
    }

    pub async fn get_json_for_project(
        context: &ApiContext,
        project: &QueryProject,
        branch_id: BranchId,
        version_id: VersionId,
    ) -> Result<JsonBranchVersion, HttpError> {
        let branch = QueryBranch::get(conn_lock!(context), branch_id)?;
        let version = QueryVersion::get(conn_lock!(context), version_id)?;
        Self::into_json_for_project(context, project, branch, version).await
    }

    pub async fn into_json_for_project(
        context: &ApiContext,
        project: &QueryProject,
        branch: QueryBranch,
        version: QueryVersion,
    ) -> Result<JsonBranchVersion, HttpError> {
        let project_id = branch.project_id;
        let JsonBranch {
            uuid,
            project,
            name,
            slug,
            start_point,
            created,
            modified,
        } = branch.into_json_for_project(conn_lock!(context), project)?;
        // Make sure that the version is in the same project as the branch
        assert_parentage(
            BencherResource::Project,
            project_id,
            BencherResource::Version,
            version.project_id,
        );
        let version = version.into_json();
        Ok(JsonBranchVersion {
            uuid,
            project,
            name,
            slug,
            version,
            start_point,
            created,
            modified,
        })
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = branch_version_table)]
pub struct InsertBranchVersion {
    pub branch_id: BranchId,
    pub version_id: VersionId,
}
