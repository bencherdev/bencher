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

use crate::{
    db::schema::{
        adapter as adapter_table,
        report as report_table,
    },
    diesel::ExpressionMethods,
};

pub const DEFAULT_PROJECT: &str = "default";

#[derive(Queryable, Debug, Deserialize, Serialize, JsonSchema)]
pub struct Report {
    pub id:         i32,
    pub project:    Option<String>,
    pub testbed:    Option<String>,
    pub adapter_id: i32,
    pub start_time: NaiveDateTime,
    pub end_time:   NaiveDateTime,
}

#[derive(Queryable, Debug, Deserialize, Serialize, JsonSchema)]
pub struct Adapter {
    pub id:   i32,
    pub name: String,
}

#[derive(Insertable)]
#[table_name = "report_table"]
pub struct NewReport {
    pub project:    String,
    pub testbed:    Option<String>,
    pub adapter_id: i32,
    pub start_time: NaiveDateTime,
    pub end_time:   NaiveDateTime,
}
