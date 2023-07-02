use std::str::FromStr;

use bencher_json::GitHash;
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl, Queryable, RunQueryDsl};
use uuid::Uuid;

use crate::{
    context::DbConnection,
    error::api_error,
    schema,
    schema::branch_version as branch_version_table,
    schema::version as version_table,
    util::query::{fn_get, fn_get_id},
    ApiError,
};

#[derive(Queryable)]
pub struct QueryVersion {
    pub id: i32,
    pub uuid: String,
    pub project_id: i32,
    pub number: i32,
    pub hash: Option<String>,
}

#[derive(Queryable)]
pub struct QueryBranchVersion {
    pub id: i32,
    pub branch_id: i32,
    pub version_id: i32,
}

impl QueryVersion {
    fn_get!(version);
    fn_get_id!(version);

    pub fn get_uuid(conn: &mut DbConnection, id: i32) -> Result<Uuid, ApiError> {
        let uuid: String = schema::version::table
            .filter(schema::version::id.eq(id))
            .select(schema::version::uuid)
            .first(conn)
            .map_err(api_error!())?;
        Uuid::from_str(&uuid).map_err(api_error!())
    }
}

impl QueryBranchVersion {
    fn_get!(branch_version);
}

#[derive(Insertable)]
#[diesel(table_name = version_table)]
pub struct InsertVersion {
    pub uuid: String,
    pub project_id: i32,
    pub number: i32,
    pub hash: Option<String>,
}

#[derive(Insertable)]
#[diesel(table_name = branch_version_table)]
pub struct InsertBranchVersion {
    pub branch_id: i32,
    pub version_id: i32,
}

impl InsertVersion {
    pub fn increment(
        conn: &mut DbConnection,
        project_id: i32,
        branch_id: i32,
        hash: Option<GitHash>,
    ) -> Result<i32, ApiError> {
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
            .map_err(api_error!())?;

        let version_id = QueryVersion::get_id(conn, &uuid)?;

        let insert_branch_version = InsertBranchVersion {
            branch_id,
            version_id,
        };

        diesel::insert_into(schema::branch_version::table)
            .values(&insert_branch_version)
            .execute(conn)
            .map_err(api_error!())?;

        Ok(version_id)
    }
}
