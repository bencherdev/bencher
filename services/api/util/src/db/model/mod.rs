use chrono::DateTime;
use chrono::Utc;
use diesel::Queryable;

#[derive(Queryable)]
pub struct Report {
    pub id: u64,
    pub date_time: DateTime<Utc>,
    pub hash: u64,
    pub len: u64,
}
