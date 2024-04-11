use bencher_json::{
    project::{
        alert::{AlertStatus, JsonAlert, JsonPerfAlert, JsonUpdateAlert},
        boundary::BoundaryLimit,
        report::Iteration,
    },
    AlertUuid, BoundaryUuid, DateTime, ReportUuid,
};
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl, SelectableHelper};
use dropshot::HttpError;
use http::StatusCode;

use super::{
    boundary::{BoundaryId, QueryBoundary},
    QueryThreshold,
};
use crate::{
    context::DbConnection,
    error::{issue_error, resource_conflict_err, resource_not_found_err},
    model::project::{
        benchmark::QueryBenchmark, metric_boundary::QueryMetricBoundary, ProjectId, QueryProject,
    },
    schema::{self, alert as alert_table},
    util::fn_get::{fn_get, fn_get_id, fn_get_uuid},
    view,
};

crate::util::typed_id::typed_id!(AlertId);

#[derive(Debug, Clone, diesel::Queryable, diesel::Selectable)]
#[diesel(table_name = alert_table)]
pub struct QueryAlert {
    pub id: AlertId,
    pub uuid: AlertUuid,
    pub boundary_id: BoundaryId,
    pub boundary_limit: BoundaryLimit,
    pub status: AlertStatus,
    pub modified: DateTime,
}

impl QueryAlert {
    fn_get!(alert, AlertId);
    fn_get_id!(alert, AlertId, AlertUuid);
    fn_get_uuid!(alert, AlertId, AlertUuid);

    pub fn from_uuid(
        conn: &mut DbConnection,
        project_id: ProjectId,
        uuid: AlertUuid,
    ) -> Result<Self, HttpError> {
        schema::alert::table
            .filter(schema::alert::uuid.eq(uuid.to_string()))
            .inner_join(
                view::metric_boundary::table.inner_join(
                    schema::report_benchmark::table.inner_join(schema::benchmark::table),
                ),
            )
            .filter(schema::benchmark::project_id.eq(project_id))
            .select(QueryAlert::as_select())
            .first(conn)
            .map_err(resource_not_found_err!(Alert, (project_id, uuid)))
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonAlert, HttpError> {
        let (report_uuid, created, iteration, query_benchmark, query_metric_boundary) =
            schema::alert::table
                .filter(schema::alert::id.eq(self.id))
                .inner_join(
                    view::metric_boundary::table.inner_join(
                        schema::report_benchmark::table
                            .inner_join(schema::report::table)
                            .inner_join(schema::benchmark::table),
                    ),
                )
                .select((
                    schema::report::uuid,
                    schema::report::created,
                    schema::report_benchmark::iteration,
                    QueryBenchmark::as_select(),
                    QueryMetricBoundary::as_select(),
                ))
                .first::<(
                    ReportUuid,
                    DateTime,
                    Iteration,
                    QueryBenchmark,
                    QueryMetricBoundary,
                )>(conn)
                .map_err(resource_not_found_err!(Alert, self))?;
        let project = QueryProject::get(conn, query_benchmark.project_id)?;
        self.into_json_for_report(
            conn,
            &project,
            report_uuid,
            created,
            iteration,
            query_benchmark,
            query_metric_boundary,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn into_json_for_report(
        self,
        conn: &mut DbConnection,
        project: &QueryProject,
        report_uuid: ReportUuid,
        created: DateTime,
        iteration: Iteration,
        query_benchmark: QueryBenchmark,
        query_metric_boundary: QueryMetricBoundary,
    ) -> Result<JsonAlert, HttpError> {
        let Self {
            uuid,
            boundary_limit,
            status,
            modified,
            ..
        } = self;
        let (Some(threshold_id), Some(model_id)) = (
            query_metric_boundary.threshold_id,
            query_metric_boundary.model_id,
        ) else {
            return Err(issue_error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Alert is missing threshold and/or model",
                "Alert is missing threshold and/or model",
                format!("{query_metric_boundary:?}"),
            ));
        };
        let threshold = QueryThreshold::get_json(conn, threshold_id, model_id)?;
        let benchmark = query_benchmark.into_benchmark_metric_json(project, query_metric_boundary);
        Ok(JsonAlert {
            uuid,
            report: report_uuid,
            iteration,
            threshold,
            benchmark,
            limit: boundary_limit,
            status,
            created,
            modified,
        })
    }

    pub fn into_perf_json(self) -> JsonPerfAlert {
        let QueryAlert {
            uuid,
            boundary_limit,
            status,
            modified,
            ..
        } = self;
        JsonPerfAlert {
            uuid,
            limit: boundary_limit,
            status,
            modified,
        }
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = alert_table)]
pub struct InsertAlert {
    pub uuid: AlertUuid,
    pub boundary_id: BoundaryId,
    pub boundary_limit: BoundaryLimit,
    pub status: AlertStatus,
    pub modified: DateTime,
}

impl InsertAlert {
    pub fn from_boundary(
        conn: &mut DbConnection,
        boundary_uuid: BoundaryUuid,
        boundary_limit: BoundaryLimit,
    ) -> Result<(), HttpError> {
        let insert_alert = InsertAlert {
            uuid: AlertUuid::new(),
            boundary_id: QueryBoundary::get_id(conn, boundary_uuid)?,
            boundary_limit,
            status: AlertStatus::default(),
            modified: DateTime::now(),
        };

        diesel::insert_into(schema::alert::table)
            .values(&insert_alert)
            .execute(conn)
            .map_err(resource_conflict_err!(Alert, insert_alert))?;

        Ok(())
    }
}

#[derive(Debug, Clone, diesel::AsChangeset)]
#[diesel(table_name = alert_table)]
pub struct UpdateAlert {
    pub status: Option<AlertStatus>,
    pub modified: DateTime,
}

impl From<JsonUpdateAlert> for UpdateAlert {
    fn from(update: JsonUpdateAlert) -> Self {
        let JsonUpdateAlert { status } = update;
        Self {
            status,
            modified: DateTime::now(),
        }
    }
}
