use std::str::FromStr;

use bencher_json::{
    project::report::{
        data::{JsonReportAlert, JsonReportAlerts, JsonReportBenchmark, JsonReportBenchmarks},
        JsonAdapter,
    },
    JsonNewReport, JsonReport,
};
use chrono::{DateTime, TimeZone, Utc};
use diesel::{
    ExpressionMethods, Insertable, JoinOnDsl, QueryDsl, Queryable, RunQueryDsl, SqliteConnection,
};
use uuid::Uuid;

use super::{testbed::QueryTestbed, version::QueryVersion};
use crate::{
    error::api_error, model::user::QueryUser, schema, schema::report as report_table,
    util::error::database_map, ApiError,
};

pub mod metrics;

#[derive(Queryable)]
pub struct QueryReport {
    pub id: i32,
    pub uuid: String,
    pub user_id: i32,
    pub version_id: i32,
    pub testbed_id: i32,
    pub adapter: i32,
    pub start_time: i64,
    pub end_time: i64,
}

impl QueryReport {
    pub fn get_uuid(conn: &mut SqliteConnection, id: i32) -> Result<Uuid, ApiError> {
        let uuid: String = schema::report::table
            .filter(schema::report::id.eq(id))
            .select(schema::report::uuid)
            .first(conn)
            .map_err(api_error!())?;
        Uuid::from_str(&uuid).map_err(api_error!())
    }

    pub fn into_json(self, conn: &mut SqliteConnection) -> Result<JsonReport, ApiError> {
        let benchmarks = self.get_benchmarks(conn)?;
        let alerts = self.get_alerts(conn)?;
        let Self {
            id: _,
            uuid,
            user_id,
            version_id,
            testbed_id,
            adapter,
            start_time,
            end_time,
        } = self;
        Ok(JsonReport {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            user: QueryUser::get_uuid(conn, user_id)?,
            version: QueryVersion::get_uuid(conn, version_id)?,
            testbed: QueryTestbed::get_uuid(conn, testbed_id)?,
            adapter: Adapter::try_from(adapter)?.into(),
            start_time: to_date_time(start_time)?,
            end_time: to_date_time(end_time)?,
            benchmarks,
            alerts,
        })
    }

    fn get_benchmarks(
        &self,
        conn: &mut SqliteConnection,
    ) -> Result<JsonReportBenchmarks, ApiError> {
        Ok(schema::perf::table
            .inner_join(
                schema::benchmark::table.on(schema::perf::benchmark_id.eq(schema::benchmark::id)),
            )
            .filter(schema::perf::report_id.eq(self.id))
            .select(schema::perf::uuid)
            .order(schema::benchmark::name)
            .load::<String>(conn)
            .map_err(api_error!())?
            .iter()
            .filter_map(|uuid| {
                database_map("QueryReport::get_benchmarks", Uuid::from_str(uuid))
                    .map(JsonReportBenchmark)
            })
            .collect())
    }

    fn get_alerts(&self, conn: &mut SqliteConnection) -> Result<JsonReportAlerts, ApiError> {
        Ok(schema::alert::table
            .left_join(schema::perf::table.on(schema::perf::id.eq(schema::alert::perf_id)))
            .filter(schema::perf::report_id.eq(self.id))
            .select(schema::alert::uuid)
            .order(schema::alert::id)
            .load::<String>(conn)
            .map_err(api_error!())?
            .iter()
            .filter_map(|uuid| {
                database_map("QueryReport::get_alerts", Uuid::from_str(uuid)).map(JsonReportAlert)
            })
            .collect())
    }
}

// https://docs.rs/chrono/latest/chrono/serde/ts_nanoseconds/index.html
pub fn to_date_time(timestamp: i64) -> Result<DateTime<Utc>, ApiError> {
    Utc.timestamp_opt(
        timestamp / 1_000_000_000,
        (timestamp % 1_000_000_000) as u32,
    )
    .single()
    .ok_or(ApiError::Timestamp(timestamp))
}

const JSON: isize = 0;
const RUST_TEST: isize = 100;
const RUST_BENCH: isize = 150;

pub enum Adapter {
    Json = JSON,
    RustTest = RUST_TEST,
    RustBench = RUST_BENCH,
}

impl TryFrom<i32> for Adapter {
    type Error = ApiError;

    fn try_from(adapter: i32) -> Result<Self, Self::Error> {
        match adapter as isize {
            JSON => Ok(Self::Json),
            RUST_TEST => Ok(Self::RustTest),
            RUST_BENCH => Ok(Self::RustBench),
            _ => Err(ApiError::Adapter(adapter)),
        }
    }
}

impl From<&JsonAdapter> for Adapter {
    fn from(adapter: &JsonAdapter) -> Self {
        match adapter {
            JsonAdapter::Json => Self::Json,
            JsonAdapter::RustTest => Self::RustTest,
            JsonAdapter::RustBench => Self::RustBench,
        }
    }
}

impl From<Adapter> for JsonAdapter {
    fn from(adapter: Adapter) -> Self {
        match adapter {
            Adapter::Json => Self::Json,
            Adapter::RustTest => Self::RustTest,
            Adapter::RustBench => Self::RustBench,
        }
    }
}

#[derive(Insertable)]
#[diesel(table_name = report_table)]
pub struct InsertReport {
    pub uuid: String,
    pub user_id: i32,
    pub version_id: i32,
    pub testbed_id: i32,
    pub adapter: i32,
    pub start_time: i64,
    pub end_time: i64,
}

impl InsertReport {
    pub fn from_json(
        user_id: i32,
        version_id: i32,
        testbed_id: i32,
        report: &JsonNewReport,
    ) -> Self {
        Self {
            uuid: Uuid::new_v4().to_string(),
            user_id,
            version_id,
            testbed_id,
            adapter: Adapter::from(&report.adapter) as i32,
            start_time: report.start_time.timestamp_nanos(),
            end_time: report.end_time.timestamp_nanos(),
        }
    }
}
