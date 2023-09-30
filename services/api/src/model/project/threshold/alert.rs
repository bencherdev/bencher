use std::str::FromStr;

use bencher_json::project::{
    alert::{JsonAlert, JsonAlertStatus, JsonPerfAlert, JsonUpdateAlert},
    boundary::JsonLimit,
};
use chrono::Utc;
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl};
use uuid::Uuid;

use super::{
    boundary::{BoundaryId, QueryBoundary},
    statistic::StatisticId,
    QueryThreshold, ThresholdId,
};
use crate::{
    context::DbConnection,
    model::project::{benchmark::QueryBenchmark, metric::QueryMetric, ProjectId, QueryProject},
    schema::alert as alert_table,
    schema::{self},
    util::{
        query::{fn_get, fn_get_id},
        to_date_time,
    },
    ApiError,
};

crate::util::typed_id::typed_id!(AlertId);

#[derive(diesel::Queryable)]
pub struct QueryAlert {
    pub id: AlertId,
    pub uuid: String,
    pub boundary_id: BoundaryId,
    pub boundary_limit: Limit,
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
            .map_err(ApiError::from)?;
        Uuid::from_str(&uuid).map_err(ApiError::from)
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
            .map_err(ApiError::from)
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonAlert, ApiError> {
        let (
            report,
            iteration,
            threshold_id,
            statistic_id,
            query_benchmark,
            query_metric,
            query_boundary,
        ) = schema::alert::table
            .filter(schema::alert::id.eq(self.id))
            .inner_join(
                schema::boundary::table.on(schema::alert::boundary_id.eq(schema::boundary::id)),
            )
            .inner_join(
                schema::metric::table.on(schema::metric::id.eq(schema::boundary::metric_id)),
            )
            .inner_join(schema::perf::table.on(schema::metric::perf_id.eq(schema::perf::id)))
            .inner_join(schema::report::table.on(schema::perf::report_id.eq(schema::report::id)))
            .inner_join(
                schema::benchmark::table.on(schema::perf::benchmark_id.eq(schema::benchmark::id)),
            )
            .select((
                schema::report::uuid,
                schema::perf::iteration,
                schema::boundary::threshold_id,
                schema::boundary::statistic_id,
                (
                    schema::benchmark::id,
                    schema::benchmark::uuid,
                    schema::benchmark::project_id,
                    schema::benchmark::name,
                    schema::benchmark::slug,
                    schema::benchmark::created,
                    schema::benchmark::modified,
                ),
                (
                    schema::metric::id,
                    schema::metric::uuid,
                    schema::metric::perf_id,
                    schema::metric::metric_kind_id,
                    schema::metric::value,
                    schema::metric::lower_value,
                    schema::metric::upper_value,
                ),
                (
                    schema::boundary::id,
                    schema::boundary::uuid,
                    schema::boundary::threshold_id,
                    schema::boundary::statistic_id,
                    schema::boundary::metric_id,
                    schema::boundary::lower_limit,
                    schema::boundary::upper_limit,
                ),
            ))
            .first::<(
                String,
                i32,
                ThresholdId,
                StatisticId,
                QueryBenchmark,
                QueryMetric,
                QueryBoundary,
            )>(conn)
            .map_err(ApiError::from)?;
        let project = QueryProject::get_uuid(conn, query_benchmark.project_id)?;
        self.into_json_for_report(
            conn,
            project,
            report,
            iteration,
            threshold_id,
            statistic_id,
            query_benchmark,
            query_metric,
            query_boundary,
        )
    }

    #[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
    pub fn into_json_for_report(
        self,
        conn: &mut DbConnection,
        project: Uuid,
        report: String,
        iteration: i32,
        threshold_id: ThresholdId,
        statistic_id: StatisticId,
        query_benchmark: QueryBenchmark,
        query_metric: QueryMetric,
        query_boundary: QueryBoundary,
    ) -> Result<JsonAlert, ApiError> {
        let Self {
            uuid,
            boundary_limit,
            status,
            modified,
            ..
        } = self;
        let report = Uuid::from_str(&report).map_err(ApiError::from)?;
        let benchmark = query_benchmark.into_benchmark_metric_json_for_project(
            project,
            query_metric,
            Some(query_boundary),
        )?;
        Ok(JsonAlert {
            uuid: Uuid::from_str(&uuid).map_err(ApiError::from)?,
            report,
            iteration: u32::try_from(iteration).map_err(ApiError::from)?,
            threshold: QueryThreshold::get_json(conn, threshold_id, statistic_id)?,
            benchmark,
            limit: boundary_limit.into(),
            status: status.into(),
            modified: to_date_time(modified)?,
        })
    }

    pub fn into_perf_json(self) -> Result<JsonPerfAlert, ApiError> {
        let QueryAlert {
            uuid,
            boundary_limit,
            status,
            modified,
            ..
        } = self;
        Ok(JsonPerfAlert {
            uuid: Uuid::from_str(&uuid).map_err(ApiError::from)?,
            limit: boundary_limit.into(),
            status: status.into(),
            modified: to_date_time(modified)?,
        })
    }
}

const LOWER_BOOL: bool = false;
const UPPER_BOOL: bool = true;

#[derive(Debug, Clone, Copy, PartialEq, Eq, diesel::FromSqlRow, diesel::AsExpression)]
#[diesel(sql_type = diesel::sql_types::Bool)]
pub enum Limit {
    Lower,
    Upper,
}

impl From<bool> for Limit {
    fn from(limit: bool) -> Self {
        #[allow(clippy::match_bool)]
        match limit {
            LOWER_BOOL => Self::Lower,
            UPPER_BOOL => Self::Upper,
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

impl<DB> diesel::serialize::ToSql<diesel::sql_types::Bool, DB> for Limit
where
    DB: diesel::backend::Backend,
    bool: diesel::serialize::ToSql<diesel::sql_types::Bool, DB>,
{
    fn to_sql<'b>(
        &'b self,
        out: &mut diesel::serialize::Output<'b, '_, DB>,
    ) -> diesel::serialize::Result {
        match self {
            Self::Lower => LOWER_BOOL.to_sql(out),
            Self::Upper => UPPER_BOOL.to_sql(out),
        }
    }
}

impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Bool, DB> for Limit
where
    DB: diesel::backend::Backend,
    bool: diesel::deserialize::FromSql<diesel::sql_types::Bool, DB>,
{
    fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
        Ok(Self::from(bool::from_sql(bytes)?))
    }
}

const ACTIVE_INT: i32 = 0;
const DISMISSED_INT: i32 = 1;

#[derive(Debug, Clone, Copy, Default, diesel::FromSqlRow, diesel::AsExpression)]
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

#[derive(diesel::Insertable)]
#[diesel(table_name = alert_table)]
pub struct InsertAlert {
    pub uuid: String,
    pub boundary_id: BoundaryId,
    pub boundary_limit: Limit,
    pub status: Status,
    pub modified: i64,
}

impl InsertAlert {
    pub fn from_boundary(
        conn: &mut DbConnection,
        boundary: Uuid,
        boundary_limit: Limit,
    ) -> Result<(), ApiError> {
        let insert_alert = InsertAlert {
            uuid: Uuid::new_v4().to_string(),
            boundary_id: QueryBoundary::get_id(conn, &boundary)?,
            boundary_limit,
            status: Status::default(),
            modified: Utc::now().timestamp(),
        };

        diesel::insert_into(schema::alert::table)
            .values(&insert_alert)
            .execute(conn)
            .map_err(ApiError::from)?;

        Ok(())
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
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
