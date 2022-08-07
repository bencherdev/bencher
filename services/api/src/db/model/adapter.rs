use std::str::FromStr;

use bencher_json::JsonAdapter;
use diesel::{
    QueryDsl,
    Queryable,
    RunQueryDsl,
    SqliteConnection,
};
use dropshot::HttpError;
use uuid::Uuid;

use crate::{
    db::schema,
    diesel::ExpressionMethods,
    util::http_error,
};

const ADAPTER_ERROR: &str = "Failed to get adapter.";

#[derive(Queryable)]
pub struct QueryAdapter {
    pub id:   i32,
    pub uuid: String,
    pub name: String,
}

impl QueryAdapter {
    pub fn get_id(conn: &SqliteConnection, uuid: &Uuid) -> Result<i32, HttpError> {
        schema::adapter::table
            .filter(schema::adapter::uuid.eq(&uuid.to_string()))
            .select(schema::adapter::id)
            .first(conn)
            .map_err(|_| http_error!(ADAPTER_ERROR))
    }

    pub fn get_id_from_name(conn: &SqliteConnection, name: &str) -> Result<i32, HttpError> {
        schema::adapter::table
            .filter(schema::adapter::name.eq(name))
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

    pub fn to_json(self) -> Result<JsonAdapter, HttpError> {
        let Self { id: _, uuid, name } = self;
        Ok(JsonAdapter {
            uuid: Uuid::from_str(&uuid).map_err(|_| http_error!(ADAPTER_ERROR))?,
            name,
        })
    }
}
