use bencher_json::{BranchUuid, GitHash, JsonStartPoint, ReferenceUuid};
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl, SelectableHelper};
use dropshot::HttpError;

use crate::{
    conn_lock,
    context::{ApiContext, DbConnection},
    error::resource_not_found_err,
    schema::{self, reference_version as reference_version_table},
    util::fn_get::fn_get,
};

use super::{
    reference::ReferenceId,
    version::{QueryVersion, VersionId},
    ProjectId, QueryBranch,
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

    pub fn into_start_point_json(
        self,
        conn: &mut DbConnection,
    ) -> Result<JsonStartPoint, HttpError> {
        let (branch, reference) = schema::branch::table
            .inner_join(
                schema::reference::table.on(schema::reference::branch_id.eq(schema::branch::id)),
            )
            .filter(schema::reference::id.eq(self.reference_id))
            .select((schema::branch::uuid, schema::reference::uuid))
            .first::<(BranchUuid, ReferenceUuid)>(conn)
            .map_err(resource_not_found_err!(Reference, self.reference_id))?;
        let version = QueryVersion::get(conn, self.version_id)?.into_json();
        Ok(JsonStartPoint {
            branch,
            reference,
            version,
        })
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = reference_version_table)]
pub struct InsertReferenceVersion {
    pub reference_id: ReferenceId,
    pub version_id: VersionId,
}
