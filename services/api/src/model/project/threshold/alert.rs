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

use super::{
    boundary::{BoundaryId, QueryBoundary},
    QueryThreshold,
};
use crate::{
    conn_lock,
    context::{ApiContext, DbConnection},
    error::{resource_conflict_err, resource_not_found_err},
    model::project::{
        benchmark::QueryBenchmark,
        branch::{reference::ReferenceId, version::VersionId},
        metric::QueryMetric,
        ProjectId, QueryProject,
    },
    schema::{self, alert as alert_table},
    util::fn_get::{fn_get, fn_get_id, fn_get_uuid},
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
            .inner_join(schema::boundary::table.inner_join(
                schema::metric::table.inner_join(
                    schema::report_benchmark::table.inner_join(schema::benchmark::table),
                ),
            ))
            .filter(schema::benchmark::project_id.eq(project_id))
            .select(QueryAlert::as_select())
            .first(conn)
            .map_err(resource_not_found_err!(Alert, (project_id, uuid)))
    }

    pub async fn silence_all(
        context: &ApiContext,
        reference_id: ReferenceId,
    ) -> Result<usize, HttpError> {
        let alerts =
            schema::alert::table
                .inner_join(schema::boundary::table.inner_join(
                    schema::metric::table.inner_join(
                        schema::report_benchmark::table.inner_join(schema::report::table),
                    ),
                ))
                .filter(schema::report::reference_id.eq(reference_id))
                .select(schema::alert::id)
                .load::<AlertId>(conn_lock!(context))
                .map_err(resource_not_found_err!(Alert, reference_id))?;

        let silenced_alert = UpdateAlert::silence();
        for alert_id in &alerts {
            diesel::update(schema::alert::table.filter(schema::alert::id.eq(alert_id)))
                .set(&silenced_alert)
                .execute(conn_lock!(context))
                .map_err(resource_conflict_err!(Alert, (alert_id, &silenced_alert)))?;
        }

        Ok(alerts.len())
    }

    pub async fn into_json(self, context: &ApiContext) -> Result<JsonAlert, HttpError> {
        let (
            report_uuid,
            created,
            reference_id,
            version_id,
            iteration,
            query_benchmark,
            query_metric,
            query_boundary,
        ) = schema::alert::table
            .filter(schema::alert::id.eq(self.id))
            .inner_join(
                schema::boundary::table.inner_join(
                    schema::metric::table.inner_join(
                        schema::report_benchmark::table
                            .inner_join(schema::report::table)
                            .inner_join(schema::benchmark::table),
                    ),
                ),
            )
            .select((
                schema::report::uuid,
                schema::report::created,
                schema::report::reference_id,
                schema::report::version_id,
                schema::report_benchmark::iteration,
                QueryBenchmark::as_select(),
                QueryMetric::as_select(),
                QueryBoundary::as_select(),
            ))
            .first::<(
                ReportUuid,
                DateTime,
                ReferenceId,
                VersionId,
                Iteration,
                QueryBenchmark,
                QueryMetric,
                QueryBoundary,
            )>(conn_lock!(context))
            .map_err(resource_not_found_err!(Alert, self))?;
        let project = QueryProject::get(conn_lock!(context), query_benchmark.project_id)?;
        self.into_json_for_report(
            context,
            &project,
            report_uuid,
            created,
            reference_id,
            version_id,
            iteration,
            query_benchmark,
            query_metric,
            query_boundary,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn into_json_for_report(
        self,
        context: &ApiContext,
        project: &QueryProject,
        report_uuid: ReportUuid,
        created: DateTime,
        reference_id: ReferenceId,
        version_id: VersionId,
        iteration: Iteration,
        query_benchmark: QueryBenchmark,
        query_metric: QueryMetric,
        query_boundary: QueryBoundary,
    ) -> Result<JsonAlert, HttpError> {
        let Self {
            uuid,
            boundary_limit,
            status,
            modified,
            ..
        } = self;
        let threshold = QueryThreshold::get_alert_json(
            context,
            query_boundary.threshold_id,
            query_boundary.model_id,
            reference_id,
            version_id,
        )
        .await?;
        Ok(JsonAlert {
            uuid,
            report: report_uuid,
            iteration,
            benchmark: query_benchmark.into_json_for_project(project),
            metric: query_metric.into_json(),
            threshold,
            boundary: query_boundary.into_json(),
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

impl UpdateAlert {
    pub fn silence() -> Self {
        JsonUpdateAlert {
            status: Some(AlertStatus::Silenced),
        }
        .into()
    }
}
