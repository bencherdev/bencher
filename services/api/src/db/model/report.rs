use std::str::FromStr;

use bencher_json::{
    JsonNewReport,
    JsonReport,
};
use chrono::NaiveDateTime;
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
    pub project:    Option<String>,
    pub testbed_id: Option<i32>,
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
            project,
            testbed_id,
            adapter_id,
            start_time,
            end_time,
        } = self;
        Ok(JsonReport {
            uuid: Uuid::from_str(&uuid).map_err(|_| http_error!("Failed to get report."))?,
            user_uuid: QueryUser::get_uuid(conn, user_id)?,
            project,
            testbed_uuid: QueryTestbed::get_uuid(conn, testbed_id),
            adapter_uuid: QueryAdapter::get_uuid(conn, adapter_id),
            start_time,
            end_time,
        })
    }
}

#[derive(Insertable)]
#[table_name = "report_table"]
pub struct InsertReport {
    pub uuid:       String,
    pub user_id:    i32,
    pub project:    String,
    pub testbed_id: Option<i32>,
    pub adapter_id: i32,
    pub start_time: NaiveDateTime,
    pub end_time:   NaiveDateTime,
}

impl InsertReport {
    pub fn new(conn: &SqliteConnection, user_uuid: &Uuid, report: JsonNewReport) -> Self {
        let JsonNewReport {
            project,
            testbed,
            adapter,
            start_time,
            end_time,
            // TODO actually insert benchmarks
            benchmarks,
        } = report;
        Self {
            uuid:       Uuid::new_v4().to_string(),
            user_id:    QueryUser::get_id(conn, user_uuid),
            project:    unwrap_project(project.as_deref()),
            testbed_id: QueryTestbed::get_id(conn, testbed),
            adapter_id: QueryAdapter::get_id(conn, adapter.to_string()),
            start_time: start_time.naive_utc(),
            end_time:   end_time.naive_utc(),
        }
    }
}

fn unwrap_project(project: Option<&str>) -> String {
    if let Some(project) = project {
        slug::slugify(project)
    } else {
        DEFAULT_PROJECT.into()
    }
}
