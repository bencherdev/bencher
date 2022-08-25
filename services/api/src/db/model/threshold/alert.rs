use std::str::FromStr;

use bencher_json::{
    alert::{
        JsonAlert,
        JsonSide,
    },
    report::{
        JsonNewPerf,
        JsonReportAlert,
        JsonReportAlerts,
    },
};
use diesel::{
    expression_methods::BoolExpressionMethods,
    Insertable,
    JoinOnDsl,
    NullableExpressionMethods,
    QueryDsl,
    Queryable,
    RunQueryDsl,
    SqliteConnection,
};
use dropshot::HttpError;
use uuid::Uuid;

use super::{
    statistic::{
        QueryStatistic,
        ThresholdStatistic,
    },
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
            perf_id,
            threshold_id,
            statistic_id,
            side,
            boundary,
            outlier,
        } = self;
        Ok(JsonAlert {
            uuid:      Uuid::from_str(&uuid).map_err(|_| http_error!(ALERT_ERROR))?,
            perf:      QueryPerf::get_uuid(conn, perf_id)?,
            threshold: QueryThreshold::get_uuid(conn, threshold_id)?,
            statistic: QueryStatistic::get_uuid(conn, statistic_id)?,
            side:      Side::from(side).into(),
            boundary:  boundary.into(),
            outlier:   outlier.into(),
        })
    }
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
    pub perf_id:      i32,
    pub threshold_id: i32,
    pub statistic_id: i32,
    pub side:         bool,
    pub boundary:     f64,
    pub outlier:      f64,
}

struct Perf {
    pub id: i32,
    pub latency_id: Option<i32>,
    pub throughput_id: Option<i32>,
    pub compute_id: Option<i32>,
    pub memory_id: Option<i32>,
    pub storage_id: Option<i32>,
}

impl InsertAlert {
    pub fn alerts(
        conn: &SqliteConnection,
        threshold_statistic: Option<&ThresholdStatistic>,
        benchmark_id: i32,
    ) -> Result<JsonReportAlerts, HttpError> {
        let alerts = Vec::new();

        let threshold_statistic = if let Some(threshold_statistic) = threshold_statistic {
            threshold_statistic
        } else {
            return Ok(alerts);
        };

        let perfs: Vec<Perf> = schema::perf::table
            .left_join(
                schema::benchmark::table.on(schema::perf::benchmark_id.eq(schema::benchmark::id)),
            )
            .filter(schema::benchmark::id.eq(benchmark_id))
            .left_join(schema::report::table.on(schema::perf::report_id.eq(schema::report::id)))
            .filter(schema::report::start_time.ge(threshold_statistic.statistic.window))
            .left_join(
                schema::testbed::table.on(schema::report::testbed_id.eq(schema::testbed::id)),
            )
            .filter(schema::testbed::id.eq(threshold_statistic.testbed_id))
            .left_join(
                schema::version::table.on(schema::report::version_id.eq(schema::version::id)),
            )
            .left_join(schema::branch::table.on(schema::version::branch_id.eq(schema::branch::id)))
            .filter(schema::branch::id.eq(threshold_statistic.branch_id))
            .select((
                schema::perf::id,
                schema::perf::latency_id,
                schema::perf::throughput_id,
                schema::perf::compute_id,
                schema::perf::memory_id,
                schema::perf::storage_id,
            ))
            .order((
                schema::version::number.desc(),
                schema::report::start_time.desc(),
                schema::perf::iteration.desc(),
            ))
            .limit(threshold_statistic.statistic.sample_size)
            .load::<(
                i32,
                Option<i32>,
                Option<i32>,
                Option<i32>,
                Option<i32>,
                Option<i32>,
            )>(conn)
            .map_err(|_| http_error!(ALERT_ERROR))?
            .into_iter()
            .map(
                |(id, latency_id, throughput_id, compute_id, memory_id, storage_id)| Perf {
                    id,
                    latency_id,
                    throughput_id,
                    compute_id,
                    memory_id,
                    storage_id,
                },
            )
            .collect();

        for perf in &perfs {
            // let latency = if let Some(latency_id) = perf.latency_id {
            //     schema::latency::table
            //         .filter(schema::latency::id.eq(latency_id))
            //         .first::<QueryLatency>(conn)
            //         .ok()
            // } else {
            //     None
            // };
        }

        Ok(alerts)
    }
}
