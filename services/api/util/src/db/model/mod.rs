use chrono::DateTime;
use chrono::Utc;
use diesel::Insertable;
use diesel::Queryable;

use crate::db::schema::report;

#[derive(Queryable, Debug)]
pub struct Report {
    pub id: i32,
    pub date_time: DateTime<Utc>,
    pub hash: i32,
    pub length: i32,
}

#[derive(Insertable)]
#[table_name = "report"]
pub struct NewReport {
    pub date_time: DateTime<Utc>,
    pub hash: i32,
    pub length: i32,
}
