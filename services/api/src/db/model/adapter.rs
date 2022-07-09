use chrono::NaiveDateTime;
use diesel::{
    Insertable,
    QueryDsl,
    Queryable,
    RunQueryDsl,
    SqliteConnection,
};
use report::{
    Adapter as JsonAdapter,
    Report as JsonReport,
};
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};
use tokio::sync::{
    Mutex,
    MutexGuard,
};
use uuid::Uuid;

use crate::{
    db::{
        schema,
        schema::{
            adapter as adapter_table,
            report as report_table,
        },
    },
    diesel::ExpressionMethods,
};

#[derive(Queryable, Debug, Deserialize, Serialize, JsonSchema)]
pub struct QueryAdapter {
    pub id:   i32,
    pub uuid: String,
    pub name: String,
}

impl QueryAdapter {
    pub fn get_id(conn: &SqliteConnection, name: String) -> i32 {
        schema::adapter::table
            .filter(schema::adapter::name.eq(name))
            .select(schema::adapter::id)
            .first(conn)
            .unwrap()
    }

    pub fn get_uuid(conn: &SqliteConnection, id: i32) -> String {
        schema::adapter::table
            .filter(schema::adapter::id.eq(id))
            .select(schema::adapter::uuid)
            .first(conn)
            .unwrap()
    }
}
