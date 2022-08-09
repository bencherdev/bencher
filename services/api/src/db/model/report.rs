use std::str::FromStr;

use bencher_json::{
    report::{
        JsonBenchmarkPerf,
        JsonBenchmarks,
        JsonNewAdapter,
    },
    JsonReport,
};
use chrono::{
    DateTime,
    TimeZone,
    Utc,
};
use diesel::{
    Insertable,
    JoinOnDsl,
    Queryable,
    SqliteConnection,
};
use dropshot::HttpError;
use uuid::Uuid;

use super::{
    adapter::QueryAdapter,
    testbed::QueryTestbed,
    user::QueryUser,
    version::QueryVersion,
};
use crate::{
    db::{
        schema,
        schema::report as report_table,
    },
    diesel::{
        ExpressionMethods,
        QueryDsl,
        RunQueryDsl,
    },
    util::http_error,
};

pub const DEFAULT_PROJECT: &str = "default";
const REPORT_ERROR: &str = "Failed to get report.";

#[derive(Queryable)]
pub struct QueryReport {
    pub id:         i32,
    pub uuid:       String,
    pub user_id:    i32,
    pub version_id: i32,
    pub testbed_id: i32,
    pub adapter_id: i32,
    pub start_time: i64,
    pub end_time:   i64,
}

impl QueryReport {
    pub fn to_json(self, conn: &SqliteConnection) -> Result<JsonReport, HttpError> {
        let id = self.id;
        self.to_json_with_benchmarks(conn, get_benchmarks(conn, id)?)
    }

    pub fn to_json_with_benchmarks(
        self,
        conn: &SqliteConnection,
        benchmarks: JsonBenchmarks,
    ) -> Result<JsonReport, HttpError> {
        let Self {
            id: _,
            uuid,
            user_id,
            version_id,
            testbed_id,
            adapter_id,
            start_time,
            end_time,
        } = self;
        Ok(JsonReport {
            uuid: Uuid::from_str(&uuid).map_err(|_| http_error!(REPORT_ERROR))?,
            user_uuid: QueryUser::get_uuid(conn, user_id)?,
            version_uuid: QueryVersion::get_uuid(conn, version_id)?,
            testbed_uuid: QueryTestbed::get_uuid(conn, testbed_id)?,
            adapter_uuid: QueryAdapter::get_uuid(conn, adapter_id)?,
            start_time: to_date_time(start_time)?,
            end_time: to_date_time(end_time)?,
            benchmarks,
        })
    }
}

// https://docs.rs/chrono/latest/chrono/serde/ts_nanoseconds/index.html
pub fn to_date_time(timestamp: i64) -> Result<DateTime<Utc>, HttpError> {
    Utc.timestamp_opt(
        timestamp / 1_000_000_000,
        (timestamp % 1_000_000_000) as u32,
    )
    .single()
    .ok_or(http_error!(REPORT_ERROR))
}

fn get_benchmarks(conn: &SqliteConnection, report_id: i32) -> Result<JsonBenchmarks, HttpError> {
    let uuids: Vec<(String, String)> = schema::perf::table
        .inner_join(
            schema::benchmark::table.on(schema::perf::benchmark_id.eq(schema::benchmark::id)),
        )
        .filter(schema::perf::report_id.eq(report_id))
        .select((schema::benchmark::uuid, schema::perf::uuid))
        .order(schema::benchmark::name)
        .load::<(String, String)>(conn)
        .map_err(|_| http_error!(REPORT_ERROR))?;

    let mut benchmarks = JsonBenchmarks::new();
    for (benchmark_uuid, perf_uuid) in uuids {
        benchmarks.push(JsonBenchmarkPerf {
            benchmark_uuid: Uuid::from_str(&benchmark_uuid)
                .map_err(|_| http_error!(REPORT_ERROR))?,
            perf_uuid:      Uuid::from_str(&perf_uuid).map_err(|_| http_error!(REPORT_ERROR))?,
        });
    }

    Ok(benchmarks)
}

#[derive(Insertable)]
#[table_name = "report_table"]
pub struct InsertReport {
    pub uuid:       String,
    pub user_id:    i32,
    pub version_id: i32,
    pub testbed_id: i32,
    pub adapter_id: i32,
    pub start_time: i64,
    pub end_time:   i64,
}

impl InsertReport {
    pub fn new(
        conn: &SqliteConnection,
        user_id: i32,
        version_id: i32,
        testbed_id: i32,
        adapter: &JsonNewAdapter,
        start_time: &DateTime<Utc>,
        end_time: &DateTime<Utc>,
    ) -> Result<Self, HttpError> {
        Ok(Self {
            uuid: Uuid::new_v4().to_string(),
            user_id,
            version_id,
            testbed_id,
            adapter_id: QueryAdapter::get_id_from_name(conn, &adapter.to_string())?,
            start_time: start_time.timestamp_nanos(),
            end_time: end_time.timestamp_nanos(),
        })
    }
}
