use std::str::FromStr;

use bencher_json::GitHash;
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl, Queryable, RunQueryDsl};
use uuid::Uuid;

use crate::{
    context::DbConnection,
    schema,
    schema::version as version_table,
    util::query::{fn_get, fn_get_id},
    ApiError,
};

use super::{branch::BranchId, branch_version::InsertBranchVersion, ProjectId, QueryProject};

crate::util::typed_id::typed_id!(VersionId);

#[derive(Queryable, Identifiable, Associations)]
#[diesel(table_name = version_table)]
#[diesel(belongs_to(QueryProject, foreign_key = project_id))]
pub struct QueryVersion {
    pub id: VersionId,
    pub uuid: String,
    pub project_id: ProjectId,
    pub number: i32,
    pub hash: Option<String>,
}

impl QueryVersion {
    fn_get!(version);
    fn_get_id!(version, VersionId);

    pub fn get_uuid(conn: &mut DbConnection, id: VersionId) -> Result<Uuid, ApiError> {
        let uuid: String = schema::version::table
            .filter(schema::version::id.eq(id))
            .select(schema::version::uuid)
            .first(conn)
            .map_err(ApiError::from)?;
        Uuid::from_str(&uuid).map_err(ApiError::from)
    }
}

#[derive(Insertable)]
#[diesel(table_name = version_table)]
pub struct InsertVersion {
    pub uuid: String,
    pub project_id: ProjectId,
    pub number: i32,
    pub hash: Option<String>,
}

impl InsertVersion {
    pub fn increment(
        conn: &mut DbConnection,
        project_id: ProjectId,
        branch_id: BranchId,
        hash: Option<GitHash>,
    ) -> Result<VersionId, ApiError> {
        // Get the most recent code version number for this branch and increment it.
        // Otherwise, start a new branch code version number count from zero.
        let number = if let Ok(number) = schema::version::table
            .left_join(
                schema::branch_version::table
                    .on(schema::version::id.eq(schema::branch_version::version_id)),
            )
            .filter(schema::branch_version::branch_id.eq(branch_id))
            .select(schema::version::number)
            .order(schema::version::number.desc())
            .first::<i32>(conn)
        {
            number.checked_add(1).ok_or(ApiError::BadMath)?
        } else {
            0
        };

        let uuid = Uuid::new_v4();
        let insert_version = InsertVersion {
            uuid: uuid.to_string(),
            project_id,
            number,
            hash: hash.map(Into::into),
        };

        diesel::insert_into(schema::version::table)
            .values(&insert_version)
            .execute(conn)
            .map_err(ApiError::from)?;

        let version_id = QueryVersion::get_id(conn, &uuid)?;

        let insert_branch_version = InsertBranchVersion {
            branch_id,
            version_id,
        };

        diesel::insert_into(schema::branch_version::table)
            .values(&insert_branch_version)
            .execute(conn)
            .map_err(ApiError::from)?;

        Ok(version_id)
    }
}
