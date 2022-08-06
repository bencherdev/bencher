use std::str::FromStr;

use bencher_json::JsonAdapter;
use diesel::{
    expression_methods::BoolExpressionMethods,
    QueryDsl,
    Queryable,
    RunQueryDsl,
    SqliteConnection,
};
use dropshot::HttpError;
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};
use uuid::Uuid;

use crate::{
    db::{
        schema,
        schema::version as version_table,
    },
    diesel::ExpressionMethods,
    util::http_error,
};

const VERSION_ERROR: &str = "Failed to get version.";

#[derive(Queryable, Debug, Deserialize, Serialize, JsonSchema)]
pub struct QueryVersion {
    pub id:        i32,
    pub uuid:      String,
    pub branch_id: i32,
    pub number:    i32,
    pub hash:      Option<String>,
}

impl QueryVersion {
    pub fn get_uuid(conn: &SqliteConnection, id: i32) -> Result<Uuid, HttpError> {
        let uuid: String = schema::version::table
            .filter(schema::version::id.eq(id))
            .select(schema::version::uuid)
            .first(conn)
            .map_err(|_| http_error!(VERSION_ERROR))?;
        Uuid::from_str(&uuid).map_err(|_| http_error!(VERSION_ERROR))
    }
}

#[derive(Insertable)]
#[table_name = "version_table"]
pub struct InsertVersion {
    pub uuid:      String,
    pub branch_id: i32,
    pub number:    i32,
    pub hash:      Option<String>,
}

impl InsertVersion {
    pub fn increment(
        conn: &SqliteConnection,
        branch_id: i32,
        hash: Option<String>,
    ) -> Result<i32, HttpError> {
        // Get the most recent code version number for this branch and increment it.
        // Otherwise, start a new branch code version number count from zero.
        let number = if let Ok(number) = schema::version::table
            .filter(schema::version::branch_id.eq(branch_id))
            .select(schema::version::number)
            .order(schema::version::number.desc())
            .first::<i32>(&*conn)
        {
            number + 1
        } else {
            0
        };

        let insert_version = InsertVersion {
            uuid: Uuid::new_v4().to_string(),
            branch_id,
            number,
            hash,
        };

        diesel::insert_into(schema::version::table)
            .values(&insert_version)
            .execute(&*conn)
            .map_err(|_| http_error!("Failed to create version."))?;

        schema::version::table
            .filter(
                schema::version::branch_id
                    .eq(branch_id)
                    .and(schema::version::number.eq(number)),
            )
            .select(schema::version::id)
            .first::<i32>(&*conn)
            .map_err(|_| http_error!("Failed to create version."))
    }
}
