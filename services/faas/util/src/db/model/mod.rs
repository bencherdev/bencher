use chrono::DateTime;
use chrono::Utc;
use diesel::Insertable;
use diesel::Queryable;
use reports::MetaMetrics;
use serde::{Deserialize, Serialize};

use crate::db::schema::report;

#[derive(Queryable, Debug, Deserialize, Serialize)]
pub struct Report {
    pub id: i32,
    pub date_time: DateTime<Utc>,
    pub metrics: serde_json::Value,
    pub hash: i64,
    pub length: i32,
}

#[derive(Insertable)]
#[table_name = "report"]
pub struct NewReport {
    pub date_time: DateTime<Utc>,
    pub metrics: serde_json::Value,
    pub hash: i64,
    pub length: i32,
}

// This is just for testing purposes
impl Into<MetaMetrics> for Report {
    fn into(self) -> MetaMetrics {
        MetaMetrics {
            id: self.id as usize,
            date_time: self.date_time,
            metrics: serde_json::from_str(&serde_json::to_string(&self.metrics).unwrap()).unwrap(),
            hash: self.hash as u64,
            length: self.length as usize,
        }
    }
}
