use std::str::FromStr;

use bencher_json::project::{
    alert::{JsonAlert, JsonAlertStatus},
    boundary::JsonLimit,
};
use chrono::{TimeZone, Utc};
use diesel::{ExpressionMethods, Insertable, JoinOnDsl, QueryDsl, Queryable, RunQueryDsl};
use uuid::Uuid;

use super::{boundary::QueryBoundary, QueryThreshold};
use crate::{
    context::DbConnection,
    error::api_error,
    model::project::{benchmark::QueryBenchmark, report::QueryReport},
    schema,
    schema::alert as alert_table,
    util::query::fn_get_id,
    ApiError,
};

#[derive(Queryable)]
pub struct QueryAlert {
    pub id: i32,
    pub uuid: String,
    pub boundary_id: i32,
    pub boundary_limit: bool,
    pub status: i32,
    pub modified: i64,
}

impl QueryAlert {
    fn_get_id!(alert);

    pub fn get_uuid(conn: &mut DbConnection, id: i32) -> Result<Uuid, ApiError> {
        let uuid: String = schema::alert::table
            .filter(schema::alert::id.eq(id))
            .select(schema::alert::uuid)
            .first(conn)
            .map_err(api_error!())?;
        Uuid::from_str(&uuid).map_err(api_error!())
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonAlert, ApiError> {
        let Self {
            uuid,
            boundary_id,
            boundary_limit,
            status,
            modified,
            ..
        } = self;
        let QueryBoundary {
            threshold_id,
            statistic_id,
            metric_id,
            ..
        } = QueryBoundary::get(conn, boundary_id)?;

        let (report_id, iteration): (_, i32) = schema::perf::table
            .left_join(schema::metric::table.on(schema::metric::perf_id.eq(schema::perf::id)))
            .left_join(
                schema::boundary::table.on(schema::boundary::metric_id.eq(schema::metric::id)),
            )
            .filter(schema::metric::id.eq(metric_id))
            .select((schema::perf::report_id, schema::perf::iteration))
            .first(conn)
            .map_err(api_error!())?;

        Ok(JsonAlert {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            report: QueryReport::get_uuid(conn, report_id)?,
            iteration: iteration as u32,
            threshold: QueryThreshold::get_json(conn, threshold_id, statistic_id)?,
            benchmark: QueryBenchmark::metric_json(conn, metric_id)?,
            limit: Limit::from(boundary_limit).into(),
            status: Status::try_from(status)?.into(),
            modified: Utc
                .timestamp_opt(modified, 0)
                .single()
                .ok_or(ApiError::Timestamp(modified))?,
        })
    }
}

#[derive(Debug, PartialEq)]
pub enum Limit {
    Lower = 0,
    Upper = 1,
}

impl From<bool> for Limit {
    fn from(limit: bool) -> Self {
        if limit {
            Self::Upper
        } else {
            Self::Lower
        }
    }
}

impl From<Limit> for bool {
    fn from(limit: Limit) -> Self {
        match limit {
            Limit::Lower => false,
            Limit::Upper => true,
        }
    }
}

impl From<Limit> for JsonLimit {
    fn from(limit: Limit) -> Self {
        match limit {
            Limit::Lower => Self::Lower,
            Limit::Upper => Self::Upper,
        }
    }
}

#[derive(Default)]
pub enum Status {
    #[default]
    Unread = 0,
    Read = 1,
}

impl TryFrom<i32> for Status {
    type Error = ApiError;

    fn try_from(status: i32) -> Result<Self, Self::Error> {
        match status {
            0 => Ok(Self::Unread),
            1 => Ok(Self::Read),
            _ => Err(ApiError::BadAlertStatus(status)),
        }
    }
}

impl From<Status> for i32 {
    fn from(status: Status) -> Self {
        match status {
            Status::Unread => 0,
            Status::Read => 1,
        }
    }
}

impl From<Status> for JsonAlertStatus {
    fn from(status: Status) -> Self {
        match status {
            Status::Unread => Self::Unread,
            Status::Read => Self::Read,
        }
    }
}

#[derive(Insertable)]
#[diesel(table_name = alert_table)]
pub struct InsertAlert {
    pub uuid: String,
    pub boundary_id: i32,
    pub boundary_limit: bool,
    pub status: i32,
    pub modified: i64,
}

impl InsertAlert {
    pub fn from_boundary(
        conn: &mut DbConnection,
        boundary: Uuid,
        limit: Limit,
    ) -> Result<(), ApiError> {
        let insert_alert = InsertAlert {
            uuid: Uuid::new_v4().to_string(),
            boundary_id: QueryBoundary::get_id(conn, &boundary)?,
            boundary_limit: limit.into(),
            status: Status::default().into(),
            modified: Utc::now().timestamp(),
        };

        diesel::insert_into(schema::alert::table)
            .values(&insert_alert)
            .execute(conn)
            .map_err(api_error!())?;

        Ok(())
    }
}
