use diesel::{
    RunQueryDsl,
    SqliteConnection,
};
use dropshot::HttpError;

use crate::{
    db::{
        model::threshold::alert::InsertAlert,
        schema,
    },
    util::http_error,
};

const ALERT_ERROR: &str = "Failed to create perf alert.";

pub type Alerts = Vec<Alert>;

pub struct Alert {
    pub threshold_id: i32,
    pub statistic_id: i32,
    pub side:         bool,
    pub boundary:     f64,
    pub outlier:      f64,
}

impl Alert {
    pub fn insert(
        self,
        conn: &SqliteConnection,
        report_id: i32,
        perf_id: Option<i32>,
    ) -> Result<(), HttpError> {
        let insert_alert = InsertAlert::from_alert(report_id, perf_id, self);

        diesel::insert_into(schema::alert::table)
            .values(&insert_alert)
            .execute(conn)
            .map_err(|_| http_error!(ALERT_ERROR))?;

        Ok(())
    }
}
