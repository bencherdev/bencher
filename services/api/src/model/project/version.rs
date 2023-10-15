use bencher_json::{
    project::branch::{JsonVersion, VersionNumber},
    GitHash, VersionUuid,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    error::resource_conflict_err,
    schema,
    schema::version as version_table,
    util::query::{fn_get, fn_get_id, fn_get_uuid},
};

use super::{branch::BranchId, branch_version::InsertBranchVersion, ProjectId, QueryProject};

crate::util::typed_id::typed_id!(VersionId);

#[derive(Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations)]
#[diesel(table_name = version_table)]
#[diesel(belongs_to(QueryProject, foreign_key = project_id))]
pub struct QueryVersion {
    pub id: VersionId,
    pub uuid: VersionUuid,
    pub project_id: ProjectId,
    pub number: VersionNumber,
    pub hash: Option<GitHash>,
}

impl QueryVersion {
    fn_get!(version, VersionId);
    fn_get_id!(version, VersionId, VersionUuid);
    fn_get_uuid!(version, VersionId, VersionUuid);

    pub fn get_or_increment(
        conn: &mut DbConnection,
        project_id: ProjectId,
        branch_id: BranchId,
        hash: Option<&GitHash>,
    ) -> Result<VersionId, HttpError> {
        if let Some(hash) = hash {
            if let Ok(version_id) = schema::version::table
                .inner_join(schema::branch_version::table)
                .filter(schema::branch_version::branch_id.eq(branch_id))
                .filter(schema::version::hash.eq(hash.as_ref()))
                .order(schema::version::number.desc())
                .select(schema::version::id)
                .first::<VersionId>(conn)
            {
                Ok(version_id)
            } else {
                InsertVersion::increment(conn, project_id, branch_id, Some(hash.clone()))
            }
        } else {
            InsertVersion::increment(conn, project_id, branch_id, None)
        }
    }

    pub fn into_json(self) -> JsonVersion {
        let Self { number, hash, .. } = self;
        JsonVersion { number, hash }
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = version_table)]
pub struct InsertVersion {
    pub uuid: VersionUuid,
    pub project_id: ProjectId,
    pub number: VersionNumber,
    pub hash: Option<GitHash>,
}

impl InsertVersion {
    pub fn increment(
        conn: &mut DbConnection,
        project_id: ProjectId,
        branch_id: BranchId,
        hash: Option<GitHash>,
    ) -> Result<VersionId, HttpError> {
        // Get the most recent code version number for this branch and increment it.
        // Otherwise, start a new branch code version number count from zero.
        let number = if let Ok(number) = schema::version::table
            .inner_join(schema::branch_version::table)
            .filter(schema::branch_version::branch_id.eq(branch_id))
            .select(schema::version::number)
            .order(schema::version::number.desc())
            .first::<VersionNumber>(conn)
        {
            number.increment()
        } else {
            VersionNumber::default()
        };

        let version_uuid = VersionUuid::new();
        let insert_version = InsertVersion {
            uuid: version_uuid,
            project_id,
            number,
            hash: hash.map(Into::into),
        };

        diesel::insert_into(schema::version::table)
            .values(&insert_version)
            .execute(conn)
            .map_err(resource_conflict_err!(Version, insert_version))?;

        let version_id = QueryVersion::get_id(conn, version_uuid)?;

        let insert_branch_version = InsertBranchVersion {
            branch_id,
            version_id,
        };

        diesel::insert_into(schema::branch_version::table)
            .values(&insert_branch_version)
            .execute(conn)
            .map_err(resource_conflict_err!(BranchVersion, insert_branch_version))?;

        Ok(version_id)
    }
}
