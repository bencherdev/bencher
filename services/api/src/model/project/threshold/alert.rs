use std::str::FromStr;

use bencher_json::project::{
    alert::{JsonAlert, JsonAlertStatus, JsonPerfAlert, JsonUpdateAlert},
    boundary::JsonLimit,
};
use chrono::Utc;
use diesel::{ExpressionMethods, Insertable, JoinOnDsl, QueryDsl, Queryable, RunQueryDsl};
use uuid::Uuid;

use super::{
    boundary::{BoundaryId, QueryBoundary},
    QueryThreshold,
};
use crate::{
    context::DbConnection,
    error::api_error,
    model::project::{benchmark::QueryBenchmark, report::QueryReport, ProjectId},
    schema,
    schema::alert as alert_table,
    util::{
        query::{fn_get, fn_get_id},
        to_date_time,
    },
    ApiError,
};

crate::util::typed_id::typed_id!(AlertId);

#[derive(Queryable)]
pub struct QueryAlert {
    pub id: AlertId,
    pub uuid: String,
    pub boundary_id: BoundaryId,
    pub boundary_limit: bool,
    pub status: Status,
    pub modified: i64,
}

impl QueryAlert {
    fn_get!(alert);
    fn_get_id!(alert, AlertId);

    pub fn get_uuid(conn: &mut DbConnection, id: AlertId) -> Result<Uuid, ApiError> {
        let uuid: String = schema::alert::table
            .filter(schema::alert::id.eq(id))
            .select(schema::alert::uuid)
            .first(conn)
            .map_err(api_error!())?;
        Uuid::from_str(&uuid).map_err(api_error!())
    }

    pub fn get_perf_json(
        conn: &mut DbConnection,
        boundary_id: BoundaryId,
    ) -> Result<JsonPerfAlert, ApiError> {
        let query_alert = schema::alert::table
            .filter(schema::alert::boundary_id.eq(boundary_id))
            .first::<Self>(conn)
            .map_err(api_error!())?;

        let QueryAlert {
            uuid,
            boundary_limit,
            status,
            modified,
            ..
        } = query_alert;
        Ok(JsonPerfAlert {
            uuid: Uuid::from_str(&uuid).map_err(api_error!())?,
            limit: Limit::from(boundary_limit).into(),
            status: status.into(),
            modified: to_date_time(modified)?,
        })
    }

    pub fn from_uuid(
        conn: &mut DbConnection,
        project_id: ProjectId,
        uuid: Uuid,
    ) -> Result<Self, ApiError> {
        schema::alert::table
            .left_join(
                schema::boundary::table.on(schema::alert::boundary_id.eq(schema::boundary::id)),
            )
            .left_join(schema::metric::table.on(schema::metric::id.eq(schema::boundary::metric_id)))
            .left_join(schema::perf::table.on(schema::metric::perf_id.eq(schema::perf::id)))
            .left_join(
                schema::benchmark::table.on(schema::perf::benchmark_id.eq(schema::benchmark::id)),
            )
            .filter(schema::benchmark::project_id.eq(project_id))
            .filter(schema::alert::uuid.eq(uuid.to_string()))
            .select((
                schema::alert::id,
                schema::alert::uuid,
                schema::alert::boundary_id,
                schema::alert::boundary_limit,
                schema::alert::status,
                schema::alert::modified,
            ))
            .first::<QueryAlert>(conn)
            .map_err(api_error!())
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
            iteration: u32::try_from(iteration).map_err(api_error!())?,
            threshold: QueryThreshold::get_json(conn, threshold_id, statistic_id)?,
            benchmark: QueryBenchmark::get_benchmark_metric_json(conn, metric_id)?,
            limit: Limit::from(boundary_limit).into(),
            status: status.into(),
            modified: to_date_time(modified)?,
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

const ACTIVE_INT: i32 = 0;
const DISMISSED_INT: i32 = 1;

#[derive(Debug, Clone, Copy, Default, FromSqlRow, AsExpression)]
#[diesel(sql_type = diesel::sql_types::Integer)]
#[repr(i32)]
pub enum Status {
    #[default]
    Active = ACTIVE_INT,
    Dismissed = DISMISSED_INT,
}

impl TryFrom<i32> for Status {
    type Error = ApiError;

    fn try_from(status: i32) -> Result<Self, Self::Error> {
        match status {
            ACTIVE_INT => Ok(Self::Active),
            DISMISSED_INT => Ok(Self::Dismissed),
            _ => Err(ApiError::BadAlertStatus(status)),
        }
    }
}

impl From<Status> for JsonAlertStatus {
    fn from(status: Status) -> Self {
        match status {
            Status::Active => Self::Active,
            Status::Dismissed => Self::Dismissed,
        }
    }
}

impl From<JsonAlertStatus> for Status {
    fn from(status: JsonAlertStatus) -> Self {
        match status {
            JsonAlertStatus::Active => Self::Active,
            JsonAlertStatus::Dismissed => Self::Dismissed,
        }
    }
}

impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for Status
where
    DB: diesel::backend::Backend,
    i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, DB>,
    ) -> diesel::serialize::Result {
        match self {
            Self::Active => ACTIVE_INT.to_sql(out),
            Self::Dismissed => DISMISSED_INT.to_sql(out),
        }
    }
}

impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for Status
where
    DB: diesel::backend::Backend,
    i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        Ok(Self::try_from(i32::from_sql(bytes)?)?)
    }
}

#[derive(Insertable)]
#[diesel(table_name = alert_table)]
pub struct InsertAlert {
    pub uuid: String,
    pub boundary_id: BoundaryId,
    pub boundary_limit: bool,
    pub status: Status,
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

#[derive(Debug, Clone, AsChangeset)]
#[diesel(table_name = alert_table)]
pub struct UpdateAlert {
    pub status: Option<Status>,
    pub modified: i64,
}

impl From<JsonUpdateAlert> for UpdateAlert {
    fn from(update: JsonUpdateAlert) -> Self {
        let JsonUpdateAlert { status } = update;
        Self {
            status: status.map(Into::into),
            modified: Utc::now().timestamp(),
        }
    }
}
