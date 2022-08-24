use std::str::FromStr;

use bencher_json::alert::JsonAlert;
use diesel::{
    expression_methods::BoolExpressionMethods,
    Insertable,
    JoinOnDsl,
    QueryDsl,
    RunQueryDsl,
    SqliteConnection,
};
use dropshot::HttpError;
use uuid::Uuid;

use super::{
    statistic::QueryStatistic,
    QueryThreshold,
};
use crate::{
    db::{
        model::perf::QueryPerf,
        schema,
        schema::alert as alert_table,
    },
    diesel::ExpressionMethods,
    util::http_error,
};

const ALERT_ERROR: &str = "Failed to get alert.";

#[derive(Queryable)]
pub struct QueryAlert {
    pub id:           i32,
    pub uuid:         String,
    pub perf_id:      i32,
    pub threshold_id: i32,
    pub statistic_id: i32,
    pub boundary:     f64,
    pub outlier:      f64,
}

impl QueryAlert {
    pub fn get_id(conn: &SqliteConnection, uuid: impl ToString) -> Result<i32, HttpError> {
        schema::alert::table
            .filter(schema::alert::uuid.eq(uuid.to_string()))
            .select(schema::alert::id)
            .first(conn)
            .map_err(|_| http_error!(ALERT_ERROR))
    }

    pub fn get_uuid(conn: &SqliteConnection, id: i32) -> Result<Uuid, HttpError> {
        let uuid: String = schema::alert::table
            .filter(schema::alert::id.eq(id))
            .select(schema::alert::uuid)
            .first(conn)
            .map_err(|_| http_error!(ALERT_ERROR))?;
        Uuid::from_str(&uuid).map_err(|_| http_error!(ALERT_ERROR))
    }

    pub fn to_json(self, conn: &SqliteConnection) -> Result<JsonAlert, HttpError> {
        let Self {
            id: _,
            uuid,
            perf_id,
            threshold_id,
            statistic_id,
            boundary,
            outlier,
        } = self;
        Ok(JsonAlert {
            uuid:      Uuid::from_str(&uuid).map_err(|_| http_error!(ALERT_ERROR))?,
            perf:      QueryPerf::get_uuid(conn, perf_id)?,
            threshold: QueryThreshold::get_uuid(conn, threshold_id)?,
            statistic: QueryStatistic::get_uuid(conn, statistic_id)?,
            boundary:  boundary.into(),
            outlier:   outlier.into(),
        })
    }
}

#[derive(Insertable)]
#[table_name = "alert_table"]
pub struct InsertAlert {
    pub uuid:         String,
    pub perf_id:      i32,
    pub threshold_id: i32,
    pub statistic_id: i32,
    pub boundary:     f64,
    pub outlier:      f64,
}

impl InsertAlert {
    pub fn get_alerts(conn: &SqliteConnection) -> Result<Vec<Uuid>, HttpError> {
        Ok(Vec::new())
    }
}
