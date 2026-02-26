use bencher_json::{
    AlertUuid, BoundaryUuid, DateTime, ReportUuid,
    project::{
        alert::{AlertStatus, JsonAlert, JsonPerfAlert, JsonUpdateAlert},
        boundary::BoundaryLimit,
        report::Iteration,
    },
};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, SelectableHelper as _};
use dropshot::HttpError;

use super::{
    QueryThreshold,
    boundary::{BoundaryId, QueryBoundary},
};
use crate::{
    auth_conn,
    context::{ApiContext, DbConnection},
    error::{resource_conflict_err, resource_not_found_err},
    macros::fn_get::{fn_get, fn_get_id, fn_get_uuid},
    model::project::{
        ProjectId, QueryProject,
        benchmark::QueryBenchmark,
        branch::{head::HeadId, version::VersionId},
        metric::QueryMetric,
    },
    schema::{self, alert as alert_table},
    write_conn,
};

crate::macros::typed_id::typed_id!(AlertId);

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

    pub async fn silence_all(context: &ApiContext, head_id: HeadId) -> Result<usize, HttpError> {
        let alerts =
            schema::alert::table
                .inner_join(schema::boundary::table.inner_join(
                    schema::metric::table.inner_join(
                        schema::report_benchmark::table.inner_join(schema::report::table),
                    ),
                ))
                .filter(schema::report::head_id.eq(head_id))
                .select(schema::alert::id)
                .load::<AlertId>(auth_conn!(context))
                .map_err(resource_not_found_err!(Alert, head_id))?;

        if alerts.is_empty() {
            return Ok(0);
        }

        let silenced_alert = UpdateAlert::silence();
        diesel::update(schema::alert::table.filter(schema::alert::id.eq_any(&alerts)))
            .set(&silenced_alert)
            .execute(write_conn!(context))
            .map_err(resource_conflict_err!(Alert, (&alerts, &silenced_alert)))?;

        Ok(alerts.len())
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonAlert, HttpError> {
        let (
            report_uuid,
            created,
            head_id,
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
                schema::report::head_id,
                schema::report::version_id,
                schema::report_benchmark::iteration,
                QueryBenchmark::as_select(),
                QueryMetric::as_select(),
                QueryBoundary::as_select(),
            ))
            .first::<(
                ReportUuid,
                DateTime,
                HeadId,
                VersionId,
                Iteration,
                QueryBenchmark,
                QueryMetric,
                QueryBoundary,
            )>(conn)
            .map_err(resource_not_found_err!(Alert, self))?;
        let project = QueryProject::get(conn, query_benchmark.project_id)?;
        self.into_json_for_report(
            conn,
            &project,
            report_uuid,
            created,
            head_id,
            version_id,
            iteration,
            query_benchmark,
            query_metric,
            query_boundary,
        )
    }

    #[expect(clippy::too_many_arguments)]
    pub fn into_json_for_report(
        self,
        conn: &mut DbConnection,
        project: &QueryProject,
        report_uuid: ReportUuid,
        created: DateTime,
        head_id: HeadId,
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
            conn,
            query_boundary.threshold_id,
            query_boundary.model_id,
            head_id,
            version_id,
        )?;
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
            status: status.map(Into::into),
            modified: DateTime::now(),
        }
    }
}

impl UpdateAlert {
    pub fn silence() -> Self {
        Self {
            status: Some(AlertStatus::Silenced),
            modified: DateTime::now(),
        }
    }
}

#[cfg(test)]
mod tests {
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    use crate::{
        schema,
        test_util::{
            create_alert, create_base_entities, create_benchmark, create_boundary,
            create_branch_with_head, create_head_version, create_measure, create_metric,
            create_model, create_report, create_report_benchmark, create_testbed, create_threshold,
            create_version, get_alert_status, setup_test_db,
        },
    };

    use super::UpdateAlert;

    // AlertStatus::Active = 0, AlertStatus::Silenced = 10
    const ACTIVE: i32 = 0;
    const SILENCED: i32 = 10;

    /// Helper to create the full entity chain needed for an alert.
    /// Returns `(head_id, alert_id)`.
    #[expect(clippy::too_many_arguments)]
    fn create_alert_chain(
        conn: &mut diesel::SqliteConnection,
        base_project_id: i32,
        head_id: i32,
        version_id: i32,
        testbed_id: i32,
        branch_id: i32,
        measure_id: i32,
        uuids: &AlertChainUuids<'_>,
    ) -> i32 {
        let report_id = create_report(
            conn,
            uuids.report_uuid,
            base_project_id,
            head_id,
            version_id,
            testbed_id,
        );
        let benchmark_id = create_benchmark(
            conn,
            base_project_id,
            uuids.benchmark_uuid,
            uuids.benchmark_name,
            uuids.benchmark_slug,
        );
        let report_benchmark_id = create_report_benchmark(
            conn,
            uuids.report_benchmark_uuid,
            report_id,
            0,
            benchmark_id,
        );
        let metric_id = create_metric(
            conn,
            uuids.metric_uuid,
            report_benchmark_id,
            measure_id,
            1.0,
        );
        let threshold_id = create_threshold(
            conn,
            base_project_id,
            branch_id,
            testbed_id,
            measure_id,
            uuids.threshold_uuid,
        );
        let model_id = create_model(conn, threshold_id, uuids.model_uuid, 0);
        let boundary_id =
            create_boundary(conn, uuids.boundary_uuid, metric_id, threshold_id, model_id);
        create_alert(conn, uuids.alert_uuid, boundary_id, true, ACTIVE)
    }

    struct AlertChainUuids<'a> {
        report_uuid: &'a str,
        benchmark_uuid: &'a str,
        benchmark_name: &'a str,
        benchmark_slug: &'a str,
        report_benchmark_uuid: &'a str,
        metric_uuid: &'a str,
        threshold_uuid: &'a str,
        model_uuid: &'a str,
        boundary_uuid: &'a str,
        alert_uuid: &'a str,
    }

    #[test]
    fn silence_all_updates_all_alerts_for_head() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let branch = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );
        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );
        let version_id = create_version(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            1,
            None,
        );
        create_head_version(&mut conn, branch.head_id, version_id);
        let measure = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000040",
            "latency",
            "latency",
        );

        // Create 3 alerts on the same head
        let alert1 = create_alert_chain(
            &mut conn,
            base.project_id,
            branch.head_id,
            version_id,
            testbed,
            branch.branch_id,
            measure,
            &AlertChainUuids {
                report_uuid: "00000000-0000-0000-0000-000000000100",
                benchmark_uuid: "00000000-0000-0000-0000-000000000101",
                benchmark_name: "bench1",
                benchmark_slug: "bench1",
                report_benchmark_uuid: "00000000-0000-0000-0000-000000000102",
                metric_uuid: "00000000-0000-0000-0000-000000000103",
                threshold_uuid: "00000000-0000-0000-0000-000000000104",
                model_uuid: "00000000-0000-0000-0000-000000000105",
                boundary_uuid: "00000000-0000-0000-0000-000000000106",
                alert_uuid: "00000000-0000-0000-0000-000000000107",
            },
        );
        let measure2 = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000041",
            "throughput",
            "throughput",
        );
        let alert2 = create_alert_chain(
            &mut conn,
            base.project_id,
            branch.head_id,
            version_id,
            testbed,
            branch.branch_id,
            measure2,
            &AlertChainUuids {
                report_uuid: "00000000-0000-0000-0000-000000000200",
                benchmark_uuid: "00000000-0000-0000-0000-000000000201",
                benchmark_name: "bench2",
                benchmark_slug: "bench2",
                report_benchmark_uuid: "00000000-0000-0000-0000-000000000202",
                metric_uuid: "00000000-0000-0000-0000-000000000203",
                threshold_uuid: "00000000-0000-0000-0000-000000000204",
                model_uuid: "00000000-0000-0000-0000-000000000205",
                boundary_uuid: "00000000-0000-0000-0000-000000000206",
                alert_uuid: "00000000-0000-0000-0000-000000000207",
            },
        );
        let measure3 = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000042",
            "filesize",
            "filesize",
        );
        let alert3 = create_alert_chain(
            &mut conn,
            base.project_id,
            branch.head_id,
            version_id,
            testbed,
            branch.branch_id,
            measure3,
            &AlertChainUuids {
                report_uuid: "00000000-0000-0000-0000-000000000300",
                benchmark_uuid: "00000000-0000-0000-0000-000000000301",
                benchmark_name: "bench3",
                benchmark_slug: "bench3",
                report_benchmark_uuid: "00000000-0000-0000-0000-000000000302",
                metric_uuid: "00000000-0000-0000-0000-000000000303",
                threshold_uuid: "00000000-0000-0000-0000-000000000304",
                model_uuid: "00000000-0000-0000-0000-000000000305",
                boundary_uuid: "00000000-0000-0000-0000-000000000306",
                alert_uuid: "00000000-0000-0000-0000-000000000307",
            },
        );

        // All should be Active
        assert_eq!(get_alert_status(&mut conn, alert1), ACTIVE);
        assert_eq!(get_alert_status(&mut conn, alert2), ACTIVE);
        assert_eq!(get_alert_status(&mut conn, alert3), ACTIVE);

        // Query alert IDs for this head
        let alert_ids: Vec<i32> =
            schema::alert::table
                .inner_join(schema::boundary::table.inner_join(
                    schema::metric::table.inner_join(
                        schema::report_benchmark::table.inner_join(schema::report::table),
                    ),
                ))
                .filter(schema::report::head_id.eq(branch.head_id))
                .select(schema::alert::id)
                .load::<i32>(&mut conn)
                .expect("Failed to query alerts");
        assert_eq!(alert_ids.len(), 3);

        // Bulk silence using eq_any
        let silenced_alert = UpdateAlert::silence();
        diesel::update(schema::alert::table.filter(schema::alert::id.eq_any(&alert_ids)))
            .set(&silenced_alert)
            .execute(&mut conn)
            .expect("Failed to bulk silence alerts");

        // Verify all are now Silenced
        assert_eq!(get_alert_status(&mut conn, alert1), SILENCED);
        assert_eq!(get_alert_status(&mut conn, alert2), SILENCED);
        assert_eq!(get_alert_status(&mut conn, alert3), SILENCED);
    }

    #[test]
    fn silence_all_ignores_other_heads() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let branch1 = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );
        let branch2 = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000012",
            "feature",
            "feature",
            "00000000-0000-0000-0000-000000000013",
        );
        let testbed = create_testbed(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );
        let version_id = create_version(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            1,
            None,
        );
        create_head_version(&mut conn, branch1.head_id, version_id);
        create_head_version(&mut conn, branch2.head_id, version_id);
        let measure = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000040",
            "latency",
            "latency",
        );

        // Create alert on head1
        let alert_head1 = create_alert_chain(
            &mut conn,
            base.project_id,
            branch1.head_id,
            version_id,
            testbed,
            branch1.branch_id,
            measure,
            &AlertChainUuids {
                report_uuid: "00000000-0000-0000-0000-000000000100",
                benchmark_uuid: "00000000-0000-0000-0000-000000000101",
                benchmark_name: "bench1",
                benchmark_slug: "bench1",
                report_benchmark_uuid: "00000000-0000-0000-0000-000000000102",
                metric_uuid: "00000000-0000-0000-0000-000000000103",
                threshold_uuid: "00000000-0000-0000-0000-000000000104",
                model_uuid: "00000000-0000-0000-0000-000000000105",
                boundary_uuid: "00000000-0000-0000-0000-000000000106",
                alert_uuid: "00000000-0000-0000-0000-000000000107",
            },
        );
        let measure2 = create_measure(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000041",
            "throughput",
            "throughput",
        );

        // Create alert on head2
        let alert_head2 = create_alert_chain(
            &mut conn,
            base.project_id,
            branch2.head_id,
            version_id,
            testbed,
            branch2.branch_id,
            measure2,
            &AlertChainUuids {
                report_uuid: "00000000-0000-0000-0000-000000000200",
                benchmark_uuid: "00000000-0000-0000-0000-000000000201",
                benchmark_name: "bench2",
                benchmark_slug: "bench2",
                report_benchmark_uuid: "00000000-0000-0000-0000-000000000202",
                metric_uuid: "00000000-0000-0000-0000-000000000203",
                threshold_uuid: "00000000-0000-0000-0000-000000000204",
                model_uuid: "00000000-0000-0000-0000-000000000205",
                boundary_uuid: "00000000-0000-0000-0000-000000000206",
                alert_uuid: "00000000-0000-0000-0000-000000000207",
            },
        );

        // Silence only head1's alerts
        let head1_alert_ids: Vec<i32> =
            schema::alert::table
                .inner_join(schema::boundary::table.inner_join(
                    schema::metric::table.inner_join(
                        schema::report_benchmark::table.inner_join(schema::report::table),
                    ),
                ))
                .filter(schema::report::head_id.eq(branch1.head_id))
                .select(schema::alert::id)
                .load::<i32>(&mut conn)
                .expect("Failed to query alerts");

        let silenced_alert = UpdateAlert::silence();
        diesel::update(schema::alert::table.filter(schema::alert::id.eq_any(&head1_alert_ids)))
            .set(&silenced_alert)
            .execute(&mut conn)
            .expect("Failed to silence");

        // head1 alert silenced, head2 alert still active
        assert_eq!(get_alert_status(&mut conn, alert_head1), SILENCED);
        assert_eq!(get_alert_status(&mut conn, alert_head2), ACTIVE);
    }

    #[test]
    fn silence_all_empty_returns_zero() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let branch = create_branch_with_head(
            &mut conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );

        // Query alerts for a head with no alerts
        let alert_ids: Vec<i32> =
            schema::alert::table
                .inner_join(schema::boundary::table.inner_join(
                    schema::metric::table.inner_join(
                        schema::report_benchmark::table.inner_join(schema::report::table),
                    ),
                ))
                .filter(schema::report::head_id.eq(branch.head_id))
                .select(schema::alert::id)
                .load::<i32>(&mut conn)
                .expect("Failed to query alerts");

        assert!(alert_ids.is_empty());
    }
}
