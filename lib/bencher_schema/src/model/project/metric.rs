use bencher_json::{MetricName, MetricUuid};
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
    pub name: MetricName,
    pub value: f64,
}

impl QueryMetric {
    fn_from_uuid!(metric, MetricUuid, Metric);

    /// Count metric usage for an organization over a time window, across all project
    /// visibilities. This is the billable figure for legacy Team (and metered
    /// Enterprise) plans and the licensed entitlements check; Pro bills on active
    /// series instead (see `series::count_active`).
    #[cfg(feature = "plus")]
    pub fn usage(
        conn: &mut DbConnection,
        organization_id: OrganizationId,
        start_time: bencher_json::DateTime,
        end_time: bencher_json::DateTime,
    ) -> Result<u32, HttpError> {
        schema::metric::table
            .inner_join(
                schema::report_benchmark::table
                    .inner_join(schema::benchmark::table.inner_join(schema::project::table))
                    .inner_join(schema::report::table),
            )
            .filter(schema::report::project_id.eq(schema::project::id))
            .filter(schema::project::organization_id.eq(organization_id))
            .filter(schema::report::end_time.ge(start_time))
            .filter(schema::report::end_time.le(end_time))
            // Named values collapse into their measure's series, so only the point
            // estimate is counted: a bounded metric is one measurement, not three.
            // In this shape every measure has exactly one `value` row, which makes
            // this exactly the count the row-per-measure table produced. The billing
            // layer revisits it when a payload can name `p99` and never name `value`.
            .filter(schema::metric::name.eq(MetricName::value()))
            .select(diesel::dsl::count_star())
            .get_result::<i64>(conn)
            .map_err(|e| {
                crate::error::issue_error(
                    "Failed to count metric usage",
                    &format!("Failed to count metric usage for organization ({organization_id}) between {start_time} and {end_time}."),
                    e,
                )
            })?
            .try_into()
            .map_err(|e| {
                crate::error::issue_error(
                    "Failed to count metric usage",
                    &format!("Failed to count metric usage for organization ({organization_id}) between {start_time} and {end_time}."),
                    e,
                )
            })
    }
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = metric_table)]
pub struct InsertMetric {
    pub uuid: MetricUuid,
    pub report_benchmark_id: ReportBenchmarkId,
    pub measure_id: MeasureId,
    pub name: MetricName,
    pub value: f64,
}

impl InsertMetric {
    /// One named scalar.
    ///
    /// `value`, `lower_value`, and `upper_value` are ordinary named rows: the
    /// metric triple is a convention over three names, not a shape the table
    /// knows about.
    pub fn named(
        report_benchmark_id: ReportBenchmarkId,
        measure_id: MeasureId,
        name: MetricName,
        value: f64,
    ) -> Self {
        Self {
            uuid: MetricUuid::new(),
            report_benchmark_id,
            measure_id,
            name,
            value,
        }
    }
}

// `usage` and `Visibility::Private` are `plus`-only, so this module compiles
// with the `plus` feature (as the rest of the test target already does).
#[cfg(test)]
mod tests {
    use bencher_json::{DateTime, MetricName, project::Visibility};
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    use super::{MeasureId, QueryMetric, ReportBenchmarkId};
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
    fn seed_metric(
        conn: &mut DbConnection,
        project_id: ProjectId,
        base: u8,
    ) -> (ReportBenchmarkId, MeasureId) {
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
        (report_benchmark, measure)
    }

    // Give a seeded metric its bound rows, the way an ingested metric triple has them.
    fn seed_bounds(
        conn: &mut DbConnection,
        report_benchmark: ReportBenchmarkId,
        measure: MeasureId,
        base: u8,
    ) {
        for (offset, name) in [
            (0x09, MetricName::lower_value()),
            (0x0a, MetricName::upper_value()),
        ] {
            diesel::insert_into(schema::metric::table)
                .values((
                    schema::metric::uuid.eq(format!(
                        "00000000-0000-0000-0000-0000000000{:02x}",
                        base + offset
                    )),
                    schema::metric::report_benchmark_id.eq(report_benchmark),
                    schema::metric::measure_id.eq(measure),
                    schema::metric::name.eq(name),
                    schema::metric::value.eq(1.0),
                ))
                .execute(conn)
                .expect("Failed to insert metric bound");
        }
    }

    #[test]
    fn usage_counts_public_and_private_projects() {
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

        assert_eq!(all, 2, "usage counts Public and Private Project metrics");
    }

    // A bounded metric is one measurement, not three. Named values collapse into
    // their measure's series, so the bound rows must not reach the billable count.
    #[test]
    fn usage_does_not_count_the_bound_rows() {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let (report_benchmark, measure) = seed_metric(&mut conn, base.project_id, 0x20);
        seed_bounds(&mut conn, report_benchmark, measure, 0x20);

        let rows: i64 = schema::metric::table
            .count()
            .get_result(&mut conn)
            .expect("Failed to count the metric rows");
        assert_eq!(rows, 3, "the metric triple is three rows");

        let usage = QueryMetric::usage(
            &mut conn,
            base.organization_id,
            DateTime::TEST,
            DateTime::TEST,
        )
        .unwrap();

        assert_eq!(usage, 1, "the bound rows do not bill");
    }
}
