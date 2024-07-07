use bencher_json::{
    project::branch::JsonBranchVersion, GitHash, JsonBranch, JsonStartPoint, NameId,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, SelectableHelper};
use dropshot::HttpError;

use crate::{
    conn_lock,
    context::{ApiContext, DbConnection},
    error::{assert_parentage, resource_not_found_err, BencherResource},
    schema::{self, branch_version as branch_version_table},
    util::fn_get::fn_get,
};

use super::{
    branch::{BranchId, QueryBranch},
    version::{QueryVersion, VersionId},
    ProjectId, QueryProject,
};

crate::util::typed_id::typed_id!(BranchVersionId);

#[derive(Debug, diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = branch_version_table)]
pub struct QueryBranchVersion {
    pub id: BranchVersionId,
    pub branch_id: BranchId,
    pub version_id: VersionId,
}

impl QueryBranchVersion {
    fn_get!(branch_version, BranchVersionId);

    pub async fn get_start_point(
        context: &ApiContext,
        project_id: ProjectId,
        branch: &NameId,
        hash: Option<&GitHash>,
    ) -> Result<Self, HttpError> {
        // Get the start point branch
        let start_point_branch =
            QueryBranch::from_name_id(conn_lock!(context), project_id, branch)?;
        let mut query = schema::branch_version::table
            .inner_join(schema::version::table)
            // Filter for the start point branch
            .filter(schema::branch_version::branch_id.eq(start_point_branch.id))
            // Sanity check that we are in the right project
            .filter(schema::version::project_id.eq(project_id))
            .into_boxed();

        if let Some(hash) = hash {
            // Make sure the start point version has the correct hash, if specified.
            query = query.filter(schema::version::hash.eq(hash));
        }

        query
            // If the hash is not specified, get the most recent version.
            .order(schema::version::number.desc())
            .select(Self::as_select())
            .first::<Self>(conn_lock!(context))
            .map_err(resource_not_found_err!(
                BranchVersion,
                (branch, hash)
            ))
    }

    pub async fn to_start_point(&self, context: &ApiContext) -> Result<StartPoint, HttpError> {
        let branch = QueryBranch::get(conn_lock!(context), self.branch_id)?;
        let version = QueryVersion::get(conn_lock!(context), self.version_id)?;
        Ok(StartPoint { branch, version })
    }

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
            archived,
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
            archived,
        })
    }
}

#[derive(Debug, Clone)]
pub struct StartPoint {
    pub branch: QueryBranch,
    pub version: QueryVersion,
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = branch_version_table)]
pub struct InsertBranchVersion {
    pub branch_id: BranchId,
    pub version_id: VersionId,
}
