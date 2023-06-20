use std::str::FromStr;

use bencher_json::{
    project::report::{JsonAdapter, JsonReportAlerts, JsonReportResults},
    JsonNewReport, JsonReport,
};
use chrono::{DateTime, TimeZone, Utc};
use diesel::{ExpressionMethods, Insertable, JoinOnDsl, QueryDsl, Queryable, RunQueryDsl};
use uuid::Uuid;

use self::adapter::Adapter;

use super::{branch::QueryBranch, testbed::QueryTestbed, version::QueryVersion};
use crate::{
    context::DbConnection,
    error::api_error,
    model::user::QueryUser,
    schema,
    schema::report as report_table,
    util::{error::database_map, query::fn_get_id},
    ApiError,
};

mod adapter;
pub mod results;

#[derive(Queryable)]
pub struct QueryReport {
    pub id: i32,
    pub uuid: String,
    pub user_id: i32,
    pub branch_id: i32,
    pub version_id: i32,
    pub testbed_id: i32,
    pub adapter: i32,
    pub start_time: i64,
    pub end_time: i64,
}

impl QueryReport {
    fn_get_id!(report);

    pub fn get_uuid(conn: &mut DbConnection, id: i32) -> Result<Uuid, ApiError> {
        let uuid: String = schema::report::table
            .filter(schema::report::id.eq(id))
            .select(schema::report::uuid)
            .first(conn)
            .map_err(api_error!())?;
        Uuid::from_str(&uuid).map_err(api_error!())
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonReport, ApiError> {
        let results = self.get_results(conn)?;
        let alerts = self.get_alerts(conn)?;
        let Self {
            uuid,
            user_id,
            branch_id,
            version_id,
            testbed_id,
            adapter,
            start_time,
            end_time,
            ..
        } = self;

        let url = "http://localhost".parse().unwrap();

        Ok(JsonReport {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            user: QueryUser::get_uuid(conn, user_id)?,
            branch: QueryBranch::get_uuid(conn, branch_id)?,
            version: QueryVersion::get_uuid(conn, version_id)?,
            testbed: QueryTestbed::get_uuid(conn, testbed_id)?,
            adapter: Adapter::try_from(adapter)?.into(),
            start_time: to_date_time(start_time)?,
            end_time: to_date_time(end_time)?,
            results,
            alerts,
            url,
        })
    }

    fn get_results(&self, conn: &mut DbConnection) -> Result<JsonReportResults, ApiError> {
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
                database_map("QueryReport::get_benchmarks", Uuid::from_str(uuid)).map(Into::into)
            })
            .collect())
    }

    fn get_alerts(&self, conn: &mut DbConnection) -> Result<JsonReportAlerts, ApiError> {
        Ok(schema::alert::table
            .left_join(schema::perf::table.on(schema::perf::id.eq(schema::alert::perf_id)))
            .filter(schema::perf::report_id.eq(self.id))
            .select(schema::alert::uuid)
            .order(schema::alert::id)
            .load::<String>(conn)
            .map_err(api_error!())?
            .iter()
            .filter_map(|uuid| {
                database_map("QueryReport::get_alerts", Uuid::from_str(uuid)).map(Into::into)
            })
            .collect())
    }
}

// https://docs.rs/chrono/latest/chrono/serde/ts_nanoseconds/index.html
#[allow(
    clippy::cast_sign_loss,
    clippy::integer_division,
    clippy::modulo_arithmetic
)]
pub fn to_date_time(timestamp: i64) -> Result<DateTime<Utc>, ApiError> {
    Utc.timestamp_opt(
        timestamp / 1_000_000_000,
        (timestamp % 1_000_000_000) as u32,
    )
    .single()
    .ok_or(ApiError::Timestamp(timestamp))
}

#[derive(Insertable)]
#[diesel(table_name = report_table)]
pub struct InsertReport {
    pub uuid: String,
    pub user_id: i32,
    pub branch_id: i32,
    pub version_id: i32,
    pub testbed_id: i32,
    pub adapter: i32,
    pub start_time: i64,
    pub end_time: i64,
}

impl InsertReport {
    pub fn from_json(
        user_id: i32,
        branch_id: i32,
        version_id: i32,
        testbed_id: i32,
        report: &JsonNewReport,
        adapter: JsonAdapter,
    ) -> Self {
        Self {
            uuid: Uuid::new_v4().to_string(),
            user_id,
            branch_id,
            version_id,
            testbed_id,
            adapter: Adapter::from(adapter) as i32,
            start_time: report.start_time.timestamp_nanos(),
            end_time: report.end_time.timestamp_nanos(),
        }
    }
}
