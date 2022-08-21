use bencher_json::threshold::JsonNewThreshold;
use diesel::{
    Insertable,
    SqliteConnection,
};
use dropshot::HttpError;
use uuid::Uuid;

use super::{
    branch::QueryBranch,
    testbed::QueryTestbed,
};
use crate::db::schema::threshold as threshold_table;

mod t_test;
mod z_score;

#[derive(Insertable)]
#[table_name = "threshold_table"]
pub struct InsertThreshold {
    pub uuid:       String,
    pub branch_id:  i32,
    pub testbed_id: i32,
}

impl InsertThreshold {
    pub fn from_json(
        conn: &SqliteConnection,
        json_threshold: JsonNewThreshold,
    ) -> Result<Self, HttpError> {
        let JsonNewThreshold { branch, testbed } = json_threshold;
        Ok(Self {
            uuid:       Uuid::new_v4().to_string(),
            branch_id:  QueryBranch::get_id(conn, &branch)?,
            testbed_id: QueryTestbed::get_id(conn, &testbed)?,
        })
    }
}
