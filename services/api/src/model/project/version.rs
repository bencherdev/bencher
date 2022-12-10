use std::str::FromStr;

use bencher_json::GitHash;
use diesel::{
    expression_methods::BoolExpressionMethods, ExpressionMethods, QueryDsl, Queryable, RunQueryDsl,
    SqliteConnection,
};
use uuid::Uuid;

use crate::{error::api_error, schema, schema::version as version_table, ApiError};

#[derive(Queryable)]
pub struct QueryVersion {
    pub id: i32,
    pub uuid: String,
    pub branch_id: i32,
    pub number: i32,
    pub hash: Option<String>,
}

impl QueryVersion {
    pub fn get_uuid(conn: &mut SqliteConnection, id: i32) -> Result<Uuid, ApiError> {
        let uuid: String = schema::version::table
            .filter(schema::version::id.eq(id))
            .select(schema::version::uuid)
            .first(conn)
            .map_err(api_error!())?;
        Uuid::from_str(&uuid).map_err(api_error!())
    }
}

#[derive(Insertable)]
#[diesel(table_name = version_table)]
pub struct InsertVersion {
    pub uuid: String,
    pub branch_id: i32,
    pub number: i32,
    pub hash: Option<String>,
}

impl InsertVersion {
    pub fn increment(
        conn: &mut SqliteConnection,
        branch_id: i32,
        hash: Option<GitHash>,
    ) -> Result<i32, ApiError> {
        // Get the most recent code version number for this branch and increment it.
        // Otherwise, start a new branch code version number count from zero.
        let number = if let Ok(number) = schema::version::table
            .filter(schema::version::branch_id.eq(branch_id))
            .select(schema::version::number)
            .order(schema::version::number.desc())
            .first::<i32>(conn)
        {
            number + 1
        } else {
            0
        };

        let insert_version = InsertVersion {
            uuid: Uuid::new_v4().to_string(),
            branch_id,
            number,
            hash: hash.map(Into::into),
        };

        diesel::insert_into(schema::version::table)
            .values(&insert_version)
            .execute(conn)
            .map_err(api_error!())?;

        schema::version::table
            .filter(
                schema::version::branch_id
                    .eq(branch_id)
                    .and(schema::version::number.eq(number)),
            )
            .select(schema::version::id)
            .first::<i32>(conn)
            .map_err(api_error!())
    }
}
