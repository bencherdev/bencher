use bencher_json::{JsonMetricTriple, MetricName, MetricUuid};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

#[cfg(feature = "plus")]
use crate::model::organization::OrganizationId;
use crate::{
    context::DbConnection,
    macros::fn_get::fn_from_uuid,
    schema::{self, metric as metric_table},
};

use super::{
    measure::{MeasureId, QueryMeasure},
    report::report_benchmark::{QueryReportBenchmark, ReportBenchmarkId},
};

crate::macros::typed_id::typed_id!(MetricId);

// The bound rows, reached as two self joins on `metric` so a reader can select a
// triple in its own query. Declared together because only one `alias!` call
// teaches the two aliases that they may appear in the same query.
diesel::alias!(
    schema::metric as metric_lower_value: MetricLowerValue,
    schema::metric as metric_upper_value: MetricUpperValue,
);

/// The `LEFT JOIN` hanging one named bound row off the `metric` row already in the
/// query, which rides `index_metric_report_benchmark_measure_name`.
macro_rules! bound_join {
    ($alias:expr, $name:expr) => {
        $alias.on($alias
            .field(schema::metric::report_benchmark_id)
            .eq(schema::metric::report_benchmark_id)
            .and(
                $alias
                    .field(schema::metric::measure_id)
                    .eq(schema::metric::measure_id),
            )
            .and($alias.field(schema::metric::name).eq($name)))
    };
}

pub(crate) use bound_join;

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

    /// The metric triple built around this row.
    ///
    /// The triple is a convention over three names, so it is only meaningful for a
    /// `value` row: the caller is what decides that this row is one. The bounds are
    /// this row's siblings under the same report benchmark and the same measure, so
    /// the lookup rides `index_metric_report_benchmark_measure_name`.
    pub fn triple(&self, conn: &mut DbConnection) -> Result<JsonMetricTriple, HttpError> {
        let bounds = schema::metric::table
            .filter(schema::metric::report_benchmark_id.eq(self.report_benchmark_id))
            .filter(schema::metric::measure_id.eq(self.measure_id))
            .filter(
                schema::metric::name.eq_any([MetricName::lower_value(), MetricName::upper_value()]),
            )
            .select((schema::metric::name, schema::metric::value))
            .load::<(MetricName, f64)>(conn)
            .map_err(|e| {
                let message = format!(
                    "Failed to query the bounds for metric ({metric_uuid})",
                    metric_uuid = self.uuid
                );
                crate::error::issue_error("Failed to query metric bounds", &message, e)
            })?;

        let bound = |bound: &MetricName| {
            bounds
                .iter()
                .find_map(|(name, value)| (name == bound).then_some(*value))
        };
        Ok(self.triple_with(
            bound(&MetricName::lower_value()),
            bound(&MetricName::upper_value()),
        ))
    }

    /// The metric triple built around this row and the bounds already in hand.
    ///
    /// A reader that selected the bound rows alongside this one assembles the same
    /// triple [`Self::triple`] does, without going back to the database.
    pub fn triple_with(
        &self,
        lower_value: Option<f64>,
        upper_value: Option<f64>,
    ) -> JsonMetricTriple {
        JsonMetricTriple {
            uuid: self.uuid,
            value: self.value.into(),
            lower_value: lower_value.map(Into::into),
            upper_value: upper_value.map(Into::into),
        }
    }

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
            // Metrics collapse into their measure's series, so only the point
            // estimate is counted: a bounded metric triple is one measurement, not three.
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
    /// One metric.
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

    // A bounded metric triple is one measurement, not three. Metrics collapse into
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
