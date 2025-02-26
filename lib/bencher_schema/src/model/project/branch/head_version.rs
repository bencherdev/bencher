use bencher_json::{BranchUuid, GitHash, HeadUuid, JsonStartPoint};
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl, SelectableHelper};
use dropshot::HttpError;

use crate::{
    conn_lock,
    context::{ApiContext, DbConnection},
    error::resource_not_found_err,
    macros::fn_get::fn_get,
    schema::{self, head_version as head_version_table},
};

use super::{
    head::HeadId,
    version::{QueryVersion, VersionId},
    ProjectId, QueryBranch,
};

crate::macros::typed_id::typed_id!(HeadVersionId);

#[derive(Debug, Clone, diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = head_version_table)]
pub struct QueryHeadVersion {
    pub id: HeadVersionId,
    pub head_id: HeadId,
    pub version_id: VersionId,
}

impl QueryHeadVersion {
    fn_get!(head_version, HeadVersionId);

    pub async fn get_latest_for_branch(
        context: &ApiContext,
        project_id: ProjectId,
        query_branch: &QueryBranch,
        hash: Option<&GitHash>,
    ) -> Result<Self, HttpError> {
        let head_id = query_branch.head_id()?;
        let mut query = schema::head_version::table
            .inner_join(schema::version::table)
            // Filter for the branch head
            .filter(schema::head_version::head_id.eq(head_id))
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
                HeadVersion,
                (query_branch, hash)
            ))
    }

    pub fn into_start_point_json(
        self,
        conn: &mut DbConnection,
    ) -> Result<JsonStartPoint, HttpError> {
        let (branch, head) = schema::branch::table
            .inner_join(schema::head::table.on(schema::head::branch_id.eq(schema::branch::id)))
            .filter(schema::head::id.eq(self.head_id))
            .select((schema::branch::uuid, schema::head::uuid))
            .first::<(BranchUuid, HeadUuid)>(conn)
            .map_err(resource_not_found_err!(Head, self.head_id))?;
        let version = QueryVersion::get(conn, self.version_id)?.into_json();
        Ok(JsonStartPoint {
            branch,
            head,
            version,
        })
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = head_version_table)]
pub struct InsertHeadVersion {
    pub head_id: HeadId,
    pub version_id: VersionId,
}
