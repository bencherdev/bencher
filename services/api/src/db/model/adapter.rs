use std::str::FromStr;

use diesel::{
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
    db::schema,
    diesel::ExpressionMethods,
    util::http_error,
};

const ADAPTER_ERROR: &str = "Failed to get adapter.";

#[derive(Queryable, Debug, Deserialize, Serialize, JsonSchema)]
pub struct QueryAdapter {
    pub id:   i32,
    pub uuid: String,
    pub name: String,
}

impl QueryAdapter {
    pub fn get_id(conn: &SqliteConnection, name: String) -> Result<i32, HttpError> {
        schema::adapter::table
            .filter(schema::adapter::name.eq(&name))
            .select(schema::adapter::id)
            .first(conn)
            .map_err(|_| http_error!(ADAPTER_ERROR))
    }

    pub fn get_uuid(conn: &SqliteConnection, id: i32) -> Result<Uuid, HttpError> {
        let uuid: String = schema::adapter::table
            .filter(schema::adapter::id.eq(id))
            .select(schema::adapter::uuid)
            .first(conn)
            .map_err(|_| http_error!(ADAPTER_ERROR))?;
        Uuid::from_str(&uuid).map_err(|_| http_error!(ADAPTER_ERROR))
    }
}
