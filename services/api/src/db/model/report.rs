use std::{
    collections::HashMap,
    str::FromStr,
};

use bencher_json::{
    report::JsonNewAdapter,
    JsonReport,
};
use chrono::{
    DateTime,
    NaiveDateTime,
    Utc,
};
use diesel::{
    Insertable,
    Queryable,
    SqliteConnection,
};
use dropshot::HttpError;
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};
use uuid::Uuid;

use super::{
    adapter::QueryAdapter,
    testbed::QueryTestbed,
    user::QueryUser,
    version::QueryVersion,
};
use crate::{
    db::schema::report as report_table,
    util::http_error,
};

pub const DEFAULT_PROJECT: &str = "default";

#[derive(Queryable, Debug, Deserialize, Serialize, JsonSchema)]
pub struct QueryReport {
    pub id:         i32,
    pub uuid:       String,
    pub user_id:    i32,
    pub version_id: i32,
    pub testbed_id: i32,
    pub adapter_id: i32,
    pub start_time: NaiveDateTime,
    pub end_time:   NaiveDateTime,
}

impl QueryReport {
    pub fn to_json(self, conn: &SqliteConnection) -> Result<JsonReport, HttpError> {
        let Self {
            id: _,
            uuid,
            user_id,
            version_id,
            testbed_id,
            adapter_id,
            start_time,
            end_time,
        } = self;
        Ok(JsonReport {
            uuid: Uuid::from_str(&uuid).map_err(|_| http_error!("Failed to get report."))?,
            user_uuid: QueryUser::get_uuid(conn, user_id)?,
            version_uuid: QueryVersion::get_uuid(conn, version_id)?,
            testbed_uuid: QueryTestbed::get_uuid(conn, testbed_id)?,
            adapter_uuid: QueryAdapter::get_uuid(conn, adapter_id)?,
            start_time,
            end_time,
            benchmarks: HashMap::new(),
        })
    }
}

#[derive(Insertable)]
#[table_name = "report_table"]
pub struct InsertReport {
    pub uuid:       String,
    pub user_id:    i32,
    pub version_id: i32,
    pub testbed_id: i32,
    pub adapter_id: i32,
    pub start_time: NaiveDateTime,
    pub end_time:   NaiveDateTime,
}

impl InsertReport {
    pub fn new(
        conn: &SqliteConnection,
        user_id: i32,
        version_id: i32,
        testbed_id: i32,
        adapter: &JsonNewAdapter,
        start_time: &DateTime<Utc>,
        end_time: &DateTime<Utc>,
    ) -> Result<Self, HttpError> {
        Ok(Self {
            uuid: Uuid::new_v4().to_string(),
            user_id,
            version_id,
            testbed_id,
            adapter_id: QueryAdapter::get_id_from_name(conn, &adapter.to_string())?,
            start_time: start_time.naive_utc(),
            end_time: end_time.naive_utc(),
        })
    }
}
