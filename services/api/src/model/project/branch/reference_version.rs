use bencher_json::{BranchUuid, GitHash, JsonBranch, JsonStartPoint, NameId};
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl, SelectableHelper};
use dropshot::HttpError;
use http::StatusCode;

use crate::{
    conn_lock,
    context::{ApiContext, DbConnection},
    error::{assert_parentage, issue_error, resource_not_found_err, BencherResource},
    schema::{self, reference_version as reference_version_table},
    util::fn_get::fn_get,
};

use super::{
    reference::ReferenceId,
    version::{QueryVersion, VersionId},
    BranchId, ProjectId, QueryBranch, QueryProject,
};

crate::util::typed_id::typed_id!(ReferenceVersionId);

#[derive(Debug, Clone, diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = reference_version_table)]
pub struct QueryReferenceVersion {
    pub id: ReferenceVersionId,
    pub reference_id: ReferenceId,
    pub version_id: VersionId,
}

impl QueryReferenceVersion {
    fn_get!(reference_version, ReferenceVersionId);

    pub async fn get_latest_for_branch_name(
        context: &ApiContext,
        project_id: ProjectId,
        branch: &NameId,
        hash: Option<&GitHash>,
    ) -> Result<Self, HttpError> {
        let query_branch = QueryBranch::from_name_id(conn_lock!(context), project_id, branch)?;
        Self::get_latest_for_branch(context, project_id, &query_branch, hash).await
    }

    pub async fn get_latest_for_branch(
        context: &ApiContext,
        project_id: ProjectId,
        query_branch: &QueryBranch,
        hash: Option<&GitHash>,
    ) -> Result<Self, HttpError> {
        let head_id = query_branch.head_id()?;
        let mut query = schema::reference_version::table
            .inner_join(schema::version::table)
            // Filter for the branch head reference
            .filter(schema::reference_version::reference_id.eq(head_id))
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
                ReferenceVersion,
                (query_branch, hash)
            ))
    }

    pub async fn to_start_point(&self, context: &ApiContext) -> Result<StartPoint, HttpError> {
        let branch = schema::branch::table
            .inner_join(
                schema::reference::table.on(schema::reference::branch_id.eq(schema::branch::id)),
            )
            .filter(schema::reference::id.eq(self.reference_id))
            .select(QueryBranch::as_select())
            .first::<QueryBranch>(conn_lock!(context))
            .map_err(resource_not_found_err!(Reference, self.reference_id))?;
        let version = QueryVersion::get(conn_lock!(context), self.version_id)?;
        Ok(StartPoint {
            id: self.id,
            branch,
            version,
        })
    }

    pub fn into_start_point_json(
        self,
        conn: &mut DbConnection,
    ) -> Result<JsonStartPoint, HttpError> {
        let branch = schema::branch::table
            .inner_join(
                schema::reference::table.on(schema::reference::branch_id.eq(schema::branch::id)),
            )
            .filter(schema::reference::id.eq(self.reference_id))
            .select(schema::branch::uuid)
            .first::<BranchUuid>(conn)
            .map_err(resource_not_found_err!(Reference, self.reference_id))?;
        Ok(JsonStartPoint {
            branch,
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
    ) -> Result<JsonBranch, HttpError> {
        let project_id = branch.project_id;
        let JsonBranch {
            uuid,
            project,
            name,
            slug,
            head,
            created,
            modified,
            archived,
        } = branch.into_json_for_head(conn_lock!(context), project)?;
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
    pub id: ReferenceVersionId,
    pub branch: QueryBranch,
    pub version: QueryVersion,
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = reference_version_table)]
pub struct InsertReferenceVersion {
    pub reference_id: ReferenceId,
    pub version_id: VersionId,
}