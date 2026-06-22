#[cfg(feature = "plus")]
use bencher_json::project::Visibility;
use bencher_json::{JsonMetric, JsonNewMetric, MetricUuid};
#[cfg(feature = "plus")]
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

#[cfg(feature = "plus")]
use crate::model::organization::OrganizationId;
#[cfg(feature = "plus")]
use crate::schema;
use crate::{context::DbConnection, macros::fn_get::fn_from_uuid, schema::metric as metric_table};

use super::{
    measure::{MeasureId, QueryMeasure},
    report::report_benchmark::{QueryReportBenchmark, ReportBenchmarkId},
};

crate::macros::typed_id::typed_id!(MetricId);

#[derive(
    Debug, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = metric_table)]
#[diesel(belongs_to(QueryReportBenchmark, foreign_key = report_benchmark_id))]
#[diesel(belongs_to(QueryMeasure, foreign_key = measure_id))]
pub struct QueryMetric {
    pub id: MetricId,
    pub uuid: MetricUuid,
    pub report_benchmark_id: ReportBenchmarkId,
    pub measure_id: MeasureId,
    pub value: f64,
    pub lower_value: Option<f64>,
    pub upper_value: Option<f64>,
}

impl QueryMetric {
    fn_from_uuid!(metric, MetricUuid, Metric);

    #[cfg(feature = "plus")]
    pub fn usage(
        conn: &mut DbConnection,
        organization_id: OrganizationId,
        start_time: bencher_json::DateTime,
        end_time: bencher_json::DateTime,
    ) -> Result<u32, HttpError> {
        Self::usage_inner(conn, organization_id, start_time, end_time, None)
    }

    /// Count private-project metric usage for an organization over a time window.
    ///
    /// This is the billable figure only for plan levels where Public Project Metrics
    /// are free (see `metered_bills_public_metrics`); levels that bill public metrics
    /// use `usage` (all visibilities) instead. The selection lives in the metered
    /// usage estimate and mirrors the skip in `PlanKind::check_usage`.
    #[cfg(feature = "plus")]
    pub fn private_usage(
        conn: &mut DbConnection,
        organization_id: OrganizationId,
        start_time: bencher_json::DateTime,
        end_time: bencher_json::DateTime,
    ) -> Result<u32, HttpError> {
        Self::usage_inner(
            conn,
            organization_id,
            start_time,
            end_time,
            Some(Visibility::Private),
        )
    }

    /// Count metric usage for an organization over a time window, optionally
    /// restricted to projects of a given `visibility`.
    #[cfg(feature = "plus")]
    fn usage_inner(
        conn: &mut DbConnection,
        organization_id: OrganizationId,
        start_time: bencher_json::DateTime,
        end_time: bencher_json::DateTime,
        visibility: Option<Visibility>,
    ) -> Result<u32, HttpError> {
        let mut query = schema::metric::table
            .inner_join(
                schema::report_benchmark::table
                    .inner_join(schema::benchmark::table.inner_join(schema::project::table))
                    .inner_join(schema::report::table),
            )
            .filter(schema::report::project_id.eq(schema::project::id))
            .filter(schema::project::organization_id.eq(organization_id))
            .filter(schema::report::end_time.ge(start_time))
            .filter(schema::report::end_time.le(end_time))
            .select(diesel::dsl::count_star())
            .into_boxed();
        if let Some(visibility) = visibility {
            query = query.filter(schema::project::visibility.eq(visibility));
        }
        query
            .get_result::<i64>(conn)
            .map_err(|e| {
                crate::error::issue_error(
                    "Failed to count metric usage",
                    &format!("Failed to count metric usage (visibility: {visibility:?}) for organization ({organization_id}) between {start_time} and {end_time}."),
                    e,
                )
            })?
            .try_into()
            .map_err(|e| {
                crate::error::issue_error(
                    "Failed to count metric usage",
                    &format!("Failed to count metric usage (visibility: {visibility:?}) for organization ({organization_id}) between {start_time} and {end_time}."),
                    e,
                )
            })
    }

    pub fn into_json(self) -> JsonMetric {
        let Self {
            uuid,
            value,
            lower_value,
            upper_value,
            ..
        } = self;
        JsonMetric {
            uuid,
            value: value.into(),
            lower_value: lower_value.map(Into::into),
            upper_value: upper_value.map(Into::into),
        }
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = metric_table)]
pub struct InsertMetric {
    pub uuid: MetricUuid,
    pub report_benchmark_id: ReportBenchmarkId,
    pub measure_id: MeasureId,
    pub value: f64,
    pub lower_value: Option<f64>,
    pub upper_value: Option<f64>,
}

impl InsertMetric {
    pub fn from_json(
        report_benchmark_id: ReportBenchmarkId,
        measure_id: MeasureId,
        metric: JsonNewMetric,
    ) -> Self {
        let JsonNewMetric {
            value,
            lower_value,
            upper_value,
        } = metric;
        Self {
            uuid: MetricUuid::new(),
            report_benchmark_id,
            measure_id,
            value: value.into(),
            lower_value: lower_value.map(Into::into),
            upper_value: upper_value.map(Into::into),
        }
    }
}

// `private_usage` and `Visibility::Private` are `plus`-only, so this module
// compiles with the `plus` feature (as the rest of the test target already does).
#[cfg(test)]
mod tests {
    use bencher_json::{DateTime, project::Visibility};
    use diesel::{ExpressionMethods as _, RunQueryDsl as _};

    use super::QueryMetric;
    use crate::{
        context::DbConnection,
        macros::sql::last_insert_rowid,
        model::{organization::OrganizationId, project::ProjectId},
        schema,
        test_util::{
            create_base_entities, create_benchmark, create_branch_with_head, create_measure,
            create_metric, create_report, create_report_benchmark, create_testbed, create_version,
            setup_test_db,
        },
    };

    fn create_private_project(
        conn: &mut DbConnection,
        organization_id: OrganizationId,
    ) -> ProjectId {
        diesel::insert_into(schema::project::table)
            .values((
                schema::project::uuid.eq("00000000-0000-0000-0000-000000000003"),
                schema::project::organization_id.eq(organization_id),
                schema::project::name.eq("Private Project"),
                schema::project::slug.eq("private-project"),
                schema::project::visibility.eq(Visibility::Private),
                schema::project::created.eq(DateTime::TEST),
                schema::project::modified.eq(DateTime::TEST),
            ))
            .execute(conn)
            .expect("Failed to insert private project");
        diesel::select(last_insert_rowid())
            .get_result(conn)
            .expect("Failed to get private project id")
    }

    // Seed one metric under `project_id`. `base` namespaces the entity UUIDs and
    // slugs so multiple projects can be seeded into the same database.
    fn seed_metric(conn: &mut DbConnection, project_id: ProjectId, base: u8) {
        let uuid = |n: u8| format!("00000000-0000-0000-0000-0000000000{:02x}", base + n);
        let branch = create_branch_with_head(
            conn,
            project_id,
            &uuid(0),
            "Main",
            &format!("main-{base}"),
            &uuid(1),
        );
        let version = create_version(conn, project_id, &uuid(2), 1, None);
        let testbed = create_testbed(
            conn,
            project_id,
            &uuid(3),
            "Testbed",
            &format!("testbed-{base}"),
        );
        let measure = create_measure(
            conn,
            project_id,
            &uuid(4),
            "Latency",
            &format!("latency-{base}"),
        );
        let report = create_report(conn, &uuid(5), project_id, branch.head_id, version, testbed);
        let benchmark = create_benchmark(
            conn,
            project_id,
            &uuid(6),
            "Bench",
            &format!("bench-{base}"),
        );
        let report_benchmark = create_report_benchmark(conn, &uuid(7), report, 0, benchmark);
        create_metric(conn, &uuid(8), report_benchmark, measure, 1.0);
    }

    #[test]
    fn private_usage_counts_only_private_projects() {
        let mut conn = setup_test_db();
        // `create_base_entities` makes a Public project (visibility 0).
        let base = create_base_entities(&mut conn);
        seed_metric(&mut conn, base.project_id, 0x20);
        let private_project = create_private_project(&mut conn, base.organization_id);
        seed_metric(&mut conn, private_project, 0x40);

        let all = QueryMetric::usage(
            &mut conn,
            base.organization_id,
            DateTime::TEST,
            DateTime::TEST,
        )
        .unwrap();
        let private = QueryMetric::private_usage(
            &mut conn,
            base.organization_id,
            DateTime::TEST,
            DateTime::TEST,
        )
        .unwrap();

        assert_eq!(all, 2, "usage counts Public and Private Project metrics");
        assert_eq!(
            private, 1,
            "private_usage counts only Private Project metrics"
        );
    }
}
