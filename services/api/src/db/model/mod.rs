use chrono::NaiveDateTime;
use diesel::{
    Insertable,
    Queryable,
};
use report::Report as JsonReport;
use schemars::JsonSchema;
use serde::{
    Deserialize,
    Serialize,
};

use crate::db::schema::report as report_table;

pub const DEFAULT_PROJECT: &str = "default";

#[derive(Queryable, Debug, Deserialize, Serialize, JsonSchema)]
pub struct Report {
    pub id:         i32,
    pub project:    Option<String>,
    pub testbed:    Option<String>,
    pub start_time: Option<NaiveDateTime>,
    pub end_time:   NaiveDateTime,
}

#[derive(Insertable)]
#[table_name = "report_table"]
pub struct NewReport {
    pub project:    String,
    pub testbed:    Option<String>,
    pub start_time: Option<NaiveDateTime>,
    pub end_time:   NaiveDateTime,
}

// todo -> add validation here and switch to `try_from`.
impl From<JsonReport> for NewReport {
    fn from(report: JsonReport) -> Self {
        let JsonReport {
            project,
            testbed,
            start_time,
            end_time,
            metrics: _,
        } = report;
        Self {
            project: unwrap_project(project.as_deref()),
            testbed,
            start_time: start_time.map(|dt| dt.naive_utc()),
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
