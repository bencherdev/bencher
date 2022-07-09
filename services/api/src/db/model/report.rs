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

use super::adapter::QueryAdapter;
use crate::{
    api::headers::CorsHeaders,
    db::{
        schema,
        schema::{
            adapter as adapter_table,
            report as report_table,
        },
    },
    diesel::ExpressionMethods,
};

pub const DEFAULT_PROJECT: &str = "default";

#[derive(Queryable, Debug, Deserialize, Serialize, JsonSchema)]
pub struct QueryReport {
    pub id:         i32,
    pub uuid:       String,
    pub project:    Option<String>,
    pub testbed:    Option<String>,
    pub adapter_id: i32,
    pub start_time: NaiveDateTime,
    pub end_time:   NaiveDateTime,
}

#[derive(Insertable)]
#[table_name = "report_table"]
pub struct InsertReport {
    pub uuid:       String,
    pub project:    String,
    pub testbed:    Option<String>,
    pub adapter_id: i32,
    pub start_time: NaiveDateTime,
    pub end_time:   NaiveDateTime,
}

impl InsertReport {
    pub fn new(conn: &SqliteConnection, report: JsonReport) -> Self {
        let JsonReport {
            project,
            testbed,
            adapter,
            start_time,
            end_time,
            metrics: _,
        } = report;
        Self {
            uuid: Uuid::new_v4().to_string(),
            project: unwrap_project(project.as_deref()),
            testbed,
            adapter_id: QueryAdapter::get_id(conn, adapter.to_string()),
            start_time: start_time.naive_utc(),
            end_time: end_time.naive_utc(),
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
