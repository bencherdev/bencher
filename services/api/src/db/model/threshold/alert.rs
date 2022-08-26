use std::str::FromStr;

use bencher_json::alert::{
    JsonAlert,
    JsonSide,
};
use diesel::{
    Insertable,
    QueryDsl,
    Queryable,
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
        model::{
            perf::QueryPerf,
            report::QueryReport,
        },
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
    pub report_id:    i32,
    pub perf_id:      Option<i32>,
    pub threshold_id: i32,
    pub statistic_id: i32,
    pub side:         bool,
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
            report_id,
            perf_id,
            threshold_id,
            statistic_id,
            side,
            boundary,
            outlier,
        } = self;
        Ok(JsonAlert {
            uuid:      Uuid::from_str(&uuid).map_err(|_| http_error!(ALERT_ERROR))?,
            report:    QueryReport::get_uuid(conn, report_id)?,
            perf:      map_id(conn, perf_id)?,
            threshold: QueryThreshold::get_uuid(conn, threshold_id)?,
            statistic: QueryStatistic::get_uuid(conn, statistic_id)?,
            side:      Side::from(side).into(),
            boundary:  boundary.into(),
            outlier:   outlier.into(),
        })
    }
}

fn map_id(conn: &SqliteConnection, id: Option<i32>) -> Result<Option<Uuid>, HttpError> {
    Ok(if let Some(id) = id {
        Some(QueryPerf::get_uuid(conn, id)?)
    } else {
        None
    })
}

enum Side {
    Left  = 0,
    Right = 1,
}

impl From<bool> for Side {
    fn from(side: bool) -> Self {
        match side {
            false => Self::Left,
            true => Self::Right,
        }
    }
}

impl Into<bool> for Side {
    fn into(self) -> bool {
        match self {
            Self::Left => false,
            Self::Right => true,
        }
    }
}

impl Into<JsonSide> for Side {
    fn into(self) -> JsonSide {
        match self {
            Self::Left => JsonSide::Left,
            Self::Right => JsonSide::Right,
        }
    }
}

#[derive(Insertable)]
#[table_name = "alert_table"]
pub struct InsertAlert {
    pub uuid:         String,
    pub report_id:    i32,
    pub perf_id:      Option<i32>,
    pub threshold_id: i32,
    pub statistic_id: i32,
    pub side:         bool,
    pub boundary:     f64,
    pub outlier:      f64,
}
