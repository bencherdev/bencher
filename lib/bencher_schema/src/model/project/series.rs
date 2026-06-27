#![cfg(feature = "plus")]

//! Cache of each monitored series' most recent activity.
//!
//! A *series* is a distinct `(testbed, benchmark, measure)` an organization reports
//! to. `series_last_seen` stores, per series, the greatest `report.created` (the
//! server-side ingestion time, not the user-supplied `end_time`) ever recorded for it
//! (`last_seen`). The cache is written on ingest in the same transaction as the metric
//! inserts and backfilled from existing metrics by the migration. `last_seen` only ever
//! rises (`MAX`): reprocessing an older report cannot lower it, and deleting a report
//! does NOT lower it either; a series that reported during a period was active that
//! period and stays billed for it. The one way a series leaves the count early is
//! hard-deleting its testbed, benchmark, or measure, which cascades its rows away and
//! can lower the current period's count. Accepted: entity deletion requires first
//! deleting all of that entity's reports (destroying the org's own history), and an
//! inactive series stops billing next period anyway.
//!
//! The cache exists so that billing on monthly-active series (and the matching telemetry
//! figure) is a single index range scan over `(organization_id, last_seen)` instead of a
//! `COUNT(DISTINCT ...)` over every metric. Absent report deletions it equals that
//! `COUNT(DISTINCT ...)` (the test oracle, [`oracle_count`]).

use bencher_json::{DateTime, project::Visibility};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    error::issue_error,
    model::{
        organization::OrganizationId,
        project::{ProjectId, benchmark::BenchmarkId, measure::MeasureId, testbed::TestbedId},
    },
    schema::{self, series_last_seen as series_table},
};

/// Record that a series produced a metric at `last_seen`, keeping the stored value
/// the greater of its current and new value.
///
/// Called once per distinct `(testbed, benchmark, measure)` in an ingested report,
/// inside the same write transaction as that report's metric inserts. `last_seen` is the
/// report's server-side creation time. `MAX` keeps it monotonic: reprocessing an older
/// report cannot lower a series' recorded activity, and a repeat within the same report
/// (across iterations) is an idempotent no-op.
///
/// Does not open its own transaction; callers run it inside the ingest write
/// transaction so the cache cannot drift from the metrics, and to avoid a nested
/// `SQLite` savepoint per series.
pub fn upsert_series_last_seen(
    conn: &mut DbConnection,
    organization_id: OrganizationId,
    project_id: ProjectId,
    testbed_id: TestbedId,
    benchmark_id: BenchmarkId,
    measure_id: MeasureId,
    last_seen: DateTime,
) -> diesel::QueryResult<()> {
    use crate::macros::sql::max;

    diesel::insert_into(series_table::table)
        .values((
            series_table::organization_id.eq(organization_id),
            series_table::project_id.eq(project_id),
            series_table::testbed_id.eq(testbed_id),
            series_table::benchmark_id.eq(benchmark_id),
            series_table::measure_id.eq(measure_id),
            series_table::last_seen.eq(last_seen),
        ))
        .on_conflict((
            series_table::testbed_id,
            series_table::benchmark_id,
            series_table::measure_id,
        ))
        .do_update()
        // Only `last_seen` is refreshed; `organization_id`/`project_id` are written once
        // at insert. Safe because a project cannot move organizations (no such
        // operation); if that ever changes, refresh them here too. Visibility is not
        // stored, so visibility changes are handled at read time by the project join.
        .set(series_table::last_seen.eq(max(series_table::last_seen, last_seen)))
        .execute(conn)?;
    Ok(())
}

/// Count an organization's monthly-active series in `[start_time, end_time]` (all
/// project visibilities).
///
/// This is the Pro billable figure: Pro is billed for all of its active series
/// regardless of project visibility. Mirrors `QueryMetric::usage`.
pub fn count_active(
    conn: &mut DbConnection,
    organization_id: OrganizationId,
    start_time: DateTime,
    end_time: DateTime,
) -> Result<u32, HttpError> {
    count_inner(conn, organization_id, start_time, end_time, None)
}

/// Count an organization's monthly-active series in `[start_time, end_time]`,
/// restricted to private projects.
///
/// Not currently used for billing (Pro bills all visibilities via [`count_active`]);
/// kept for a private-only view.
pub fn count_active_private(
    conn: &mut DbConnection,
    organization_id: OrganizationId,
    start_time: DateTime,
    end_time: DateTime,
) -> Result<u32, HttpError> {
    count_inner(
        conn,
        organization_id,
        start_time,
        end_time,
        Some(Visibility::Private),
    )
}

/// Count an organization's monthly-active series whose `last_seen` falls within
/// `[start_time, end_time]`, optionally restricted to a project `visibility`.
///
/// Soft-deleted projects are excluded (their series stop billing). The
/// `(organization_id, last_seen)` index makes the org-and-window filter a single
/// range scan; the join to `project` only applies the visibility and soft-delete
/// filters.
fn count_inner(
    conn: &mut DbConnection,
    organization_id: OrganizationId,
    start_time: DateTime,
    end_time: DateTime,
    visibility: Option<Visibility>,
) -> Result<u32, HttpError> {
    let mut query = series_table::table
        .inner_join(schema::project::table)
        .filter(series_table::organization_id.eq(organization_id))
        .filter(series_table::last_seen.ge(start_time))
        .filter(series_table::last_seen.le(end_time))
        .filter(schema::project::deleted.is_null())
        .select(diesel::dsl::count_star())
        .into_boxed();
    if let Some(visibility) = visibility {
        query = query.filter(schema::project::visibility.eq(visibility));
    }
    query
        .get_result::<i64>(conn)
        .map_err(|e| {
            issue_error(
                "Failed to count active series",
                &format!(
                    "Failed to count active series (visibility: {visibility:?}) for organization ({organization_id}) between {start_time} and {end_time}."
                ),
                e,
            )
        })?
        .try_into()
        .map_err(|e| {
            issue_error(
                "Failed to count active series",
                &format!(
                    "Failed to count active series (visibility: {visibility:?}) for organization ({organization_id}) between {start_time} and {end_time}."
                ),
                e,
            )
        })
}

/// Test-only oracle: the distinct-series count the cache must equal, computed from
/// scratch over the raw metric rows in plain Rust (independent of the cache's SQL).
///
/// Counts distinct `(testbed, benchmark, measure)` whose latest `report.created`
/// falls in `[start_time, end_time]`, excluding soft-deleted projects, optionally
/// restricted to a `visibility`. Never run at runtime; [`count_inner`] is the
/// runtime source and this pins it in tests (shared with the ingest tests).
#[cfg(test)]
pub(crate) fn oracle_count(
    conn: &mut DbConnection,
    organization_id: OrganizationId,
    start_time: DateTime,
    end_time: DateTime,
    visibility: Option<Visibility>,
) -> u32 {
    use std::collections::HashMap;

    let rows: Vec<(
        TestbedId,
        BenchmarkId,
        MeasureId,
        DateTime,
        Visibility,
        Option<DateTime>,
    )> = schema::metric::table
        .inner_join(
            schema::report_benchmark::table
                .inner_join(schema::benchmark::table.inner_join(schema::project::table))
                .inner_join(schema::report::table),
        )
        .filter(schema::report::project_id.eq(schema::project::id))
        .filter(schema::project::organization_id.eq(organization_id))
        .select((
            schema::report::testbed_id,
            schema::report_benchmark::benchmark_id,
            schema::metric::measure_id,
            schema::report::created,
            schema::project::visibility,
            schema::project::deleted,
        ))
        .load(conn)
        .expect("Failed to load metric rows for oracle");

    // Reduce to the latest creation time per series, applying the same project filters
    // the cache read applies.
    let mut last_by_series: HashMap<(TestbedId, BenchmarkId, MeasureId), DateTime> = HashMap::new();
    for (testbed_id, benchmark_id, measure_id, created_row, project_visibility, deleted) in rows {
        if deleted.is_some() {
            continue;
        }
        // `Visibility` has no `PartialEq`; compare discriminants (it is a unit enum).
        if let Some(want) = visibility
            && project_visibility as i32 != want as i32
        {
            continue;
        }
        // `DateTime` has no `Ord`; compare by timestamp (whole seconds, exactly what
        // SQL stores and compares).
        last_by_series
            .entry((testbed_id, benchmark_id, measure_id))
            .and_modify(|latest| {
                if created_row.timestamp() > latest.timestamp() {
                    *latest = created_row;
                }
            })
            .or_insert(created_row);
    }

    u32::try_from(
        last_by_series
            .values()
            .filter(|last_seen| {
                last_seen.timestamp() >= start_time.timestamp()
                    && last_seen.timestamp() <= end_time.timestamp()
            })
            .count(),
    )
    .expect("series count exceeds u32")
}

#[cfg(test)]
mod tests {
    use bencher_json::{DateTime, project::Visibility};
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    use super::{count_active, count_active_private, oracle_count, upsert_series_last_seen};
    use crate::{
        context::DbConnection,
        macros::sql::last_insert_rowid,
        model::{
            organization::OrganizationId,
            project::{
                ProjectId,
                benchmark::BenchmarkId,
                branch::{head::HeadId, version::VersionId},
                measure::MeasureId,
                report::ReportId,
                testbed::TestbedId,
            },
        },
        schema,
        test_util::{
            archive_benchmark, archive_measure, archive_testbed, create_benchmark,
            create_branch_with_head, create_measure, create_report_benchmark, create_testbed,
            create_version, setup_test_db,
        },
    };

    /// A fully set-up project: the ids threaded into every report this test ingests.
    #[derive(Clone, Copy)]
    struct Proj {
        org: OrganizationId,
        project: ProjectId,
        head: HeadId,
        version: VersionId,
    }

    /// A monotonic source of globally unique UUID strings, so every seeded row gets a
    /// distinct UUID without hand-tracking ranges per entity kind.
    struct Uuids(u32);

    impl Uuids {
        fn next(&mut self) -> String {
            let n = self.0;
            self.0 += 1;
            format!("00000000-0000-0000-0000-{n:012x}")
        }
    }

    /// Whole-second timestamp (storage truncates sub-second), strictly ordered by
    /// `secs` so SQL and Rust comparisons agree.
    fn at(secs: i64) -> DateTime {
        DateTime::try_from(1_700_000_000i64 + secs).expect("valid timestamp")
    }

    fn make_org(conn: &mut DbConnection, uuids: &mut Uuids) -> OrganizationId {
        let uuid = uuids.next();
        diesel::insert_into(schema::organization::table)
            .values((
                schema::organization::uuid.eq(&uuid),
                schema::organization::name.eq(format!("Org {uuid}")),
                schema::organization::slug.eq(format!("org-{uuid}")),
                schema::organization::created.eq(DateTime::TEST),
                schema::organization::modified.eq(DateTime::TEST),
            ))
            .execute(conn)
            .expect("insert organization");
        diesel::select(last_insert_rowid())
            .get_result(conn)
            .expect("organization id")
    }

    fn make_project(
        conn: &mut DbConnection,
        uuids: &mut Uuids,
        org: OrganizationId,
        visibility: Visibility,
    ) -> ProjectId {
        let uuid = uuids.next();
        diesel::insert_into(schema::project::table)
            .values((
                schema::project::uuid.eq(&uuid),
                schema::project::organization_id.eq(org),
                schema::project::name.eq(format!("Project {uuid}")),
                schema::project::slug.eq(format!("project-{uuid}")),
                schema::project::visibility.eq(visibility),
                schema::project::created.eq(DateTime::TEST),
                schema::project::modified.eq(DateTime::TEST),
            ))
            .execute(conn)
            .expect("insert project");
        diesel::select(last_insert_rowid())
            .get_result(conn)
            .expect("project id")
    }

    fn make_proj(
        conn: &mut DbConnection,
        uuids: &mut Uuids,
        org: OrganizationId,
        visibility: Visibility,
    ) -> Proj {
        let project = make_project(conn, uuids, org, visibility);
        let branch_uuid = uuids.next();
        let head_uuid = uuids.next();
        let version_uuid = uuids.next();
        let branch =
            create_branch_with_head(conn, project, &branch_uuid, "main", "main", &head_uuid);
        let version = create_version(conn, project, &version_uuid, 1, None);
        Proj {
            org,
            project,
            head: branch.head_id,
            version,
        }
    }

    /// Add a second branch (a new head) to a project, e.g. a PR branch.
    fn make_head(conn: &mut DbConnection, uuids: &mut Uuids, project: ProjectId) -> HeadId {
        let branch_uuid = uuids.next();
        let head_uuid = uuids.next();
        let slug = uuids.next();
        create_branch_with_head(conn, project, &branch_uuid, &slug, &slug, &head_uuid).head_id
    }

    fn make_testbed(conn: &mut DbConnection, uuids: &mut Uuids, project: ProjectId) -> TestbedId {
        let uuid = uuids.next();
        create_testbed(conn, project, &uuid, &uuid, &uuid)
    }

    fn make_benchmark(
        conn: &mut DbConnection,
        uuids: &mut Uuids,
        project: ProjectId,
    ) -> BenchmarkId {
        let uuid = uuids.next();
        create_benchmark(conn, project, &uuid, &uuid, &uuid)
    }

    fn make_measure(conn: &mut DbConnection, uuids: &mut Uuids, project: ProjectId) -> MeasureId {
        let uuid = uuids.next();
        create_measure(conn, project, &uuid, &uuid, &uuid)
    }

    /// Ingest one report on `head` that produces a single metric for the series
    /// `(testbed, benchmark, measure)` at `end_time`: it writes the report, the
    /// `report_benchmark`, the metric (which the oracle reads), and the series upsert
    /// (the function under test), mirroring the real ingest path.
    #[expect(
        clippy::too_many_arguments,
        reason = "test helper threads the full series key"
    )]
    fn ingest_on(
        conn: &mut DbConnection,
        uuids: &mut Uuids,
        proj: Proj,
        head: HeadId,
        testbed: TestbedId,
        benchmark: BenchmarkId,
        measure: MeasureId,
        end_time: DateTime,
    ) {
        let report_uuid = uuids.next();
        diesel::insert_into(schema::report::table)
            .values((
                schema::report::uuid.eq(&report_uuid),
                schema::report::project_id.eq(proj.project),
                schema::report::head_id.eq(head),
                schema::report::version_id.eq(proj.version),
                schema::report::testbed_id.eq(testbed),
                schema::report::adapter.eq(0),
                schema::report::start_time.eq(end_time),
                schema::report::end_time.eq(end_time),
                schema::report::created.eq(end_time),
            ))
            .execute(conn)
            .expect("insert report");
        let report: ReportId = diesel::select(last_insert_rowid())
            .get_result(conn)
            .expect("report id");
        let rb_uuid = uuids.next();
        let report_benchmark = create_report_benchmark(conn, &rb_uuid, report, 0, benchmark);
        let metric_uuid = uuids.next();
        diesel::insert_into(schema::metric::table)
            .values((
                schema::metric::uuid.eq(&metric_uuid),
                schema::metric::report_benchmark_id.eq(report_benchmark),
                schema::metric::measure_id.eq(measure),
                schema::metric::value.eq(1.0f64),
            ))
            .execute(conn)
            .expect("insert metric");
        upsert_series_last_seen(
            conn,
            proj.org,
            proj.project,
            testbed,
            benchmark,
            measure,
            end_time,
        )
        .expect("upsert series");
    }

    /// Ingest on the project's primary head.
    fn ingest(
        conn: &mut DbConnection,
        uuids: &mut Uuids,
        proj: Proj,
        testbed: TestbedId,
        benchmark: BenchmarkId,
        measure: MeasureId,
        end_time: DateTime,
    ) {
        ingest_on(
            conn, uuids, proj, proj.head, testbed, benchmark, measure, end_time,
        );
    }

    /// The whole-of-time window: every seeded series is in range.
    fn always() -> (DateTime, DateTime) {
        (at(-1), at(1_000_000))
    }

    // ----- Identity and counting -----

    #[test]
    fn iterations_do_not_inflate() {
        let mut conn = setup_test_db();
        let mut uuids = Uuids(1);
        let org = make_org(&mut conn, &mut uuids);
        let proj = make_proj(&mut conn, &mut uuids, org, Visibility::Public);
        let (testbed, benchmark, measure) = (
            make_testbed(&mut conn, &mut uuids, proj.project),
            make_benchmark(&mut conn, &mut uuids, proj.project),
            make_measure(&mut conn, &mut uuids, proj.project),
        );
        // Three reports (as if three iterations) of the same series.
        for _ in 0..3 {
            ingest(
                &mut conn,
                &mut uuids,
                proj,
                testbed,
                benchmark,
                measure,
                at(0),
            );
        }
        let (start, end) = always();
        assert_eq!(count_active(&mut conn, org, start, end).unwrap(), 1);
        assert_eq!(oracle_count(&mut conn, org, start, end, None), 1);
    }

    #[test]
    fn multiplicity_is_correct() {
        let mut conn = setup_test_db();
        let mut uuids = Uuids(1);
        let org = make_org(&mut conn, &mut uuids);
        let proj = make_proj(&mut conn, &mut uuids, org, Visibility::Public);
        let mut testbeds = Vec::new();
        let mut benchmarks = Vec::new();
        let mut measures = Vec::new();
        for _ in 0..2 {
            testbeds.push(make_testbed(&mut conn, &mut uuids, proj.project));
            benchmarks.push(make_benchmark(&mut conn, &mut uuids, proj.project));
            measures.push(make_measure(&mut conn, &mut uuids, proj.project));
        }
        for &testbed in &testbeds {
            for &benchmark in &benchmarks {
                for &measure in &measures {
                    ingest(
                        &mut conn,
                        &mut uuids,
                        proj,
                        testbed,
                        benchmark,
                        measure,
                        at(0),
                    );
                }
            }
        }
        let (start, end) = always();
        // 2 testbeds x 2 benchmarks x 2 measures = 8 series.
        assert_eq!(count_active(&mut conn, org, start, end).unwrap(), 8);
        assert_eq!(oracle_count(&mut conn, org, start, end, None), 8);
    }

    #[test]
    fn branch_is_excluded_from_series() {
        let mut conn = setup_test_db();
        let mut uuids = Uuids(1);
        let org = make_org(&mut conn, &mut uuids);
        let proj = make_proj(&mut conn, &mut uuids, org, Visibility::Public);
        let (testbed, benchmark, measure) = (
            make_testbed(&mut conn, &mut uuids, proj.project),
            make_benchmark(&mut conn, &mut uuids, proj.project),
            make_measure(&mut conn, &mut uuids, proj.project),
        );
        // Same series on `main` plus three PR branches (distinct heads).
        ingest(
            &mut conn,
            &mut uuids,
            proj,
            testbed,
            benchmark,
            measure,
            at(0),
        );
        for _ in 0..3 {
            let head = make_head(&mut conn, &mut uuids, proj.project);
            ingest_on(
                &mut conn,
                &mut uuids,
                proj,
                head,
                testbed,
                benchmark,
                measure,
                at(0),
            );
        }
        let (start, end) = always();
        assert_eq!(count_active(&mut conn, org, start, end).unwrap(), 1);
        assert_eq!(oracle_count(&mut conn, org, start, end, None), 1);
    }

    #[test]
    fn cross_project_within_org_counts_separately() {
        let mut conn = setup_test_db();
        let mut uuids = Uuids(1);
        let org = make_org(&mut conn, &mut uuids);
        // Two projects in the same org, each with its own (id-distinct) entities.
        for _ in 0..2 {
            let proj = make_proj(&mut conn, &mut uuids, org, Visibility::Public);
            let testbed = make_testbed(&mut conn, &mut uuids, proj.project);
            let benchmark = make_benchmark(&mut conn, &mut uuids, proj.project);
            let measure = make_measure(&mut conn, &mut uuids, proj.project);
            ingest(
                &mut conn,
                &mut uuids,
                proj,
                testbed,
                benchmark,
                measure,
                at(0),
            );
        }
        let (start, end) = always();
        assert_eq!(count_active(&mut conn, org, start, end).unwrap(), 2);
        assert_eq!(oracle_count(&mut conn, org, start, end, None), 2);
    }

    #[test]
    fn archived_entities_that_still_report_count() {
        let mut conn = setup_test_db();
        let mut uuids = Uuids(1);
        let org = make_org(&mut conn, &mut uuids);
        let proj = make_proj(&mut conn, &mut uuids, org, Visibility::Public);
        let (testbed, benchmark, measure) = (
            make_testbed(&mut conn, &mut uuids, proj.project),
            make_benchmark(&mut conn, &mut uuids, proj.project),
            make_measure(&mut conn, &mut uuids, proj.project),
        );
        archive_testbed(&mut conn, testbed);
        archive_benchmark(&mut conn, benchmark);
        archive_measure(&mut conn, measure);
        // An archived series that still produces a metric is still active.
        ingest(
            &mut conn,
            &mut uuids,
            proj,
            testbed,
            benchmark,
            measure,
            at(0),
        );
        let (start, end) = always();
        assert_eq!(count_active(&mut conn, org, start, end).unwrap(), 1);
        assert_eq!(oracle_count(&mut conn, org, start, end, None), 1);
    }

    #[test]
    fn rename_is_a_new_series() {
        let mut conn = setup_test_db();
        let mut uuids = Uuids(1);
        let org = make_org(&mut conn, &mut uuids);
        let proj = make_proj(&mut conn, &mut uuids, org, Visibility::Public);
        let testbed = make_testbed(&mut conn, &mut uuids, proj.project);
        let measure = make_measure(&mut conn, &mut uuids, proj.project);
        // get_or_create is name-keyed, so a mid-period rename creates a new benchmark
        // id: the same testbed/measure under two benchmark ids is two series.
        let benchmark_before = make_benchmark(&mut conn, &mut uuids, proj.project);
        let benchmark_after = make_benchmark(&mut conn, &mut uuids, proj.project);
        ingest(
            &mut conn,
            &mut uuids,
            proj,
            testbed,
            benchmark_before,
            measure,
            at(0),
        );
        ingest(
            &mut conn,
            &mut uuids,
            proj,
            testbed,
            benchmark_after,
            measure,
            at(1),
        );
        let (start, end) = always();
        assert_eq!(count_active(&mut conn, org, start, end).unwrap(), 2);
        assert_eq!(oracle_count(&mut conn, org, start, end, None), 2);
    }

    #[test]
    fn soft_deleted_project_is_excluded() {
        let mut conn = setup_test_db();
        let mut uuids = Uuids(1);
        let org = make_org(&mut conn, &mut uuids);
        let proj = make_proj(&mut conn, &mut uuids, org, Visibility::Public);
        let (testbed, benchmark, measure) = (
            make_testbed(&mut conn, &mut uuids, proj.project),
            make_benchmark(&mut conn, &mut uuids, proj.project),
            make_measure(&mut conn, &mut uuids, proj.project),
        );
        ingest(
            &mut conn,
            &mut uuids,
            proj,
            testbed,
            benchmark,
            measure,
            at(0),
        );
        let (start, end) = always();
        assert_eq!(count_active(&mut conn, org, start, end).unwrap(), 1);
        // Soft-delete the project: its series stop billing.
        diesel::update(schema::project::table.filter(schema::project::id.eq(proj.project)))
            .set(schema::project::deleted.eq(Some(DateTime::TEST)))
            .execute(&mut conn)
            .expect("soft-delete project");
        assert_eq!(count_active(&mut conn, org, start, end).unwrap(), 0);
        assert_eq!(oracle_count(&mut conn, org, start, end, None), 0);
    }

    // ----- Time and period -----

    #[test]
    fn boundary_start_is_inclusive() {
        let mut conn = setup_test_db();
        let mut uuids = Uuids(1);
        let org = make_org(&mut conn, &mut uuids);
        let proj = make_proj(&mut conn, &mut uuids, org, Visibility::Public);
        let (testbed, benchmark, measure) = (
            make_testbed(&mut conn, &mut uuids, proj.project),
            make_benchmark(&mut conn, &mut uuids, proj.project),
            make_measure(&mut conn, &mut uuids, proj.project),
        );
        // last_seen exactly at the window start is counted (`>=`).
        ingest(
            &mut conn,
            &mut uuids,
            proj,
            testbed,
            benchmark,
            measure,
            at(10),
        );
        assert_eq!(count_active(&mut conn, org, at(10), at(20)).unwrap(), 1);
        assert_eq!(oracle_count(&mut conn, org, at(10), at(20), None), 1);
        // Just after the window end is not counted.
        assert_eq!(count_active(&mut conn, org, at(0), at(9)).unwrap(), 0);
    }

    #[test]
    fn period_rollover_resets() {
        let mut conn = setup_test_db();
        let mut uuids = Uuids(1);
        let org = make_org(&mut conn, &mut uuids);
        let proj = make_proj(&mut conn, &mut uuids, org, Visibility::Public);
        let (testbed, benchmark, measure) = (
            make_testbed(&mut conn, &mut uuids, proj.project),
            make_benchmark(&mut conn, &mut uuids, proj.project),
            make_measure(&mut conn, &mut uuids, proj.project),
        );
        // Seen in period 1, never again.
        ingest(
            &mut conn,
            &mut uuids,
            proj,
            testbed,
            benchmark,
            measure,
            at(5),
        );
        // Period 1 counts it; period 2 does not.
        assert_eq!(count_active(&mut conn, org, at(0), at(10)).unwrap(), 1);
        assert_eq!(count_active(&mut conn, org, at(11), at(20)).unwrap(), 0);
        assert_eq!(oracle_count(&mut conn, org, at(11), at(20), None), 0);
    }

    #[test]
    fn backdated_report_does_not_lower_last_seen() {
        let mut conn = setup_test_db();
        let mut uuids = Uuids(1);
        let org = make_org(&mut conn, &mut uuids);
        let proj = make_proj(&mut conn, &mut uuids, org, Visibility::Public);
        let (testbed, benchmark, measure) = (
            make_testbed(&mut conn, &mut uuids, proj.project),
            make_benchmark(&mut conn, &mut uuids, proj.project),
            make_measure(&mut conn, &mut uuids, proj.project),
        );
        ingest(
            &mut conn,
            &mut uuids,
            proj,
            testbed,
            benchmark,
            measure,
            at(50),
        );
        // A late, backdated report must not lower the recorded activity (MAX).
        ingest(
            &mut conn,
            &mut uuids,
            proj,
            testbed,
            benchmark,
            measure,
            at(10),
        );
        // The late window still counts it; the early window does not (it was not
        // resurrected back to at(10)).
        assert_eq!(count_active(&mut conn, org, at(40), at(60)).unwrap(), 1);
        assert_eq!(count_active(&mut conn, org, at(0), at(20)).unwrap(), 0);
        assert_eq!(oracle_count(&mut conn, org, at(40), at(60), None), 1);
        assert_eq!(oracle_count(&mut conn, org, at(0), at(20), None), 0);
    }

    // ----- Visibility split -----

    #[test]
    fn count_active_and_private_split_by_visibility() {
        let mut conn = setup_test_db();
        let mut uuids = Uuids(1);
        let org = make_org(&mut conn, &mut uuids);
        for visibility in [Visibility::Public, Visibility::Private] {
            let proj = make_proj(&mut conn, &mut uuids, org, visibility);
            let testbed = make_testbed(&mut conn, &mut uuids, proj.project);
            let benchmark = make_benchmark(&mut conn, &mut uuids, proj.project);
            let measure = make_measure(&mut conn, &mut uuids, proj.project);
            ingest(
                &mut conn,
                &mut uuids,
                proj,
                testbed,
                benchmark,
                measure,
                at(0),
            );
        }
        let (start, end) = always();
        assert_eq!(count_active(&mut conn, org, start, end).unwrap(), 2);
        assert_eq!(count_active_private(&mut conn, org, start, end).unwrap(), 1);
        assert_eq!(oracle_count(&mut conn, org, start, end, None), 2);
        assert_eq!(
            oracle_count(&mut conn, org, start, end, Some(Visibility::Private)),
            1
        );
    }

    // ----- Edge cases -----

    #[test]
    fn zero_when_no_series() {
        let mut conn = setup_test_db();
        let mut uuids = Uuids(1);
        let org = make_org(&mut conn, &mut uuids);
        // An active org with no metrics this period bills zero series, without panic.
        let (start, end) = always();
        assert_eq!(count_active(&mut conn, org, start, end).unwrap(), 0);
        assert_eq!(count_active_private(&mut conn, org, start, end).unwrap(), 0);
    }

    #[test]
    fn org_scoped_count_excludes_other_orgs() {
        let mut conn = setup_test_db();
        let mut uuids = Uuids(1);
        let org_a = make_org(&mut conn, &mut uuids);
        let org_b = make_org(&mut conn, &mut uuids);
        for org in [org_a, org_b] {
            let proj = make_proj(&mut conn, &mut uuids, org, Visibility::Public);
            let testbed = make_testbed(&mut conn, &mut uuids, proj.project);
            let benchmark = make_benchmark(&mut conn, &mut uuids, proj.project);
            let measure = make_measure(&mut conn, &mut uuids, proj.project);
            ingest(
                &mut conn,
                &mut uuids,
                proj,
                testbed,
                benchmark,
                measure,
                at(0),
            );
        }
        let (start, end) = always();
        assert_eq!(count_active(&mut conn, org_a, start, end).unwrap(), 1);
        assert_eq!(count_active(&mut conn, org_b, start, end).unwrap(), 1);
    }

    #[test]
    fn duplicate_upsert_converges_without_error() {
        let mut conn = setup_test_db();
        let mut uuids = Uuids(1);
        let org = make_org(&mut conn, &mut uuids);
        let proj = make_proj(&mut conn, &mut uuids, org, Visibility::Public);
        let (testbed, benchmark, measure) = (
            make_testbed(&mut conn, &mut uuids, proj.project),
            make_benchmark(&mut conn, &mut uuids, proj.project),
            make_measure(&mut conn, &mut uuids, proj.project),
        );
        // Two bare upserts of the same series key converge to one row, last_seen = MAX,
        // with no duplicate-key error.
        upsert_series_last_seen(
            &mut conn,
            org,
            proj.project,
            testbed,
            benchmark,
            measure,
            at(5),
        )
        .unwrap();
        upsert_series_last_seen(
            &mut conn,
            org,
            proj.project,
            testbed,
            benchmark,
            measure,
            at(3),
        )
        .unwrap();
        let (start, end) = always();
        assert_eq!(count_active(&mut conn, org, start, end).unwrap(), 1);
        // last_seen stayed at the max (5), so the [4, 10] window still counts it.
        assert_eq!(count_active(&mut conn, org, at(4), at(10)).unwrap(), 1);
    }

    #[test]
    fn series_upsert_rolls_back_with_its_transaction() {
        use diesel::Connection as _;

        let mut conn = setup_test_db();
        let mut uuids = Uuids(1);
        let org = make_org(&mut conn, &mut uuids);
        let proj = make_proj(&mut conn, &mut uuids, org, Visibility::Public);
        let (testbed, benchmark, measure) = (
            make_testbed(&mut conn, &mut uuids, proj.project),
            make_benchmark(&mut conn, &mut uuids, proj.project),
            make_measure(&mut conn, &mut uuids, proj.project),
        );
        let (start, end) = always();
        // The cache upsert runs inside the caller's transaction (it opens none of its
        // own), so a failure anywhere in the ingest transaction rolls back the metric
        // and its series row together: the cache cannot drift from the metrics.
        let result = conn.transaction::<(), diesel::result::Error, _>(|conn| {
            ingest_on(
                conn,
                &mut uuids,
                proj,
                proj.head,
                testbed,
                benchmark,
                measure,
                at(0),
            );
            Err(diesel::result::Error::RollbackTransaction)
        });
        assert!(result.is_err());
        assert_eq!(count_active(&mut conn, org, start, end).unwrap(), 0);
        assert_eq!(oracle_count(&mut conn, org, start, end, None), 0);
    }

    #[test]
    fn entity_delete_removes_series() {
        let mut conn = setup_test_db();
        let mut uuids = Uuids(1);
        let org = make_org(&mut conn, &mut uuids);
        let proj = make_proj(&mut conn, &mut uuids, org, Visibility::Public);
        let (testbed, benchmark, measure) = (
            make_testbed(&mut conn, &mut uuids, proj.project),
            make_benchmark(&mut conn, &mut uuids, proj.project),
            make_measure(&mut conn, &mut uuids, proj.project),
        );
        ingest(
            &mut conn,
            &mut uuids,
            proj,
            testbed,
            benchmark,
            measure,
            at(0),
        );
        let (start, end) = always();
        assert_eq!(count_active(&mut conn, org, start, end).unwrap(), 1);
        // Hard-deleting an entity cascades its series rows away and lowers the current
        // period's count: the documented, accepted exception to monotonic billing (see
        // the module docs). Mirror the delete endpoint's reports-first requirement:
        // remove the benchmark's metric and report_benchmark rows, then the benchmark.
        diesel::delete(schema::metric::table)
            .execute(&mut conn)
            .expect("delete metrics");
        diesel::delete(schema::report_benchmark::table)
            .execute(&mut conn)
            .expect("delete report benchmarks");
        diesel::delete(schema::benchmark::table.filter(schema::benchmark::id.eq(benchmark)))
            .execute(&mut conn)
            .expect("delete benchmark");
        assert_eq!(count_active(&mut conn, org, start, end).unwrap(), 0);
        // The cascade removed the cache row itself, not just filtered it from the count.
        let rows: i64 = super::series_table::table
            .count()
            .get_result(&mut conn)
            .expect("count series rows");
        assert_eq!(rows, 0);
    }

    #[test]
    fn backfill_equals_oracle() {
        use diesel::sql_query;

        // Mirrors the backfill in migration 2026-06-24-120000_series_last_seen/up.sql.
        const BACKFILL: &str = "\
INSERT INTO series_last_seen (organization_id, project_id, testbed_id, benchmark_id, measure_id, last_seen)
SELECT p.organization_id, p.id, r.testbed_id, rb.benchmark_id, m.measure_id, MAX(r.created)
FROM metric m
INNER JOIN report_benchmark rb ON m.report_benchmark_id = rb.id
INNER JOIN report r ON rb.report_id = r.id
INNER JOIN benchmark b ON rb.benchmark_id = b.id
INNER JOIN project p ON b.project_id = p.id
GROUP BY r.testbed_id, rb.benchmark_id, m.measure_id";

        let mut conn = setup_test_db();
        let mut uuids = Uuids(1);
        let org = make_org(&mut conn, &mut uuids);
        let proj = make_proj(&mut conn, &mut uuids, org, Visibility::Public);
        let mut testbeds = Vec::new();
        let mut benchmarks = Vec::new();
        for _ in 0..2 {
            testbeds.push(make_testbed(&mut conn, &mut uuids, proj.project));
            benchmarks.push(make_benchmark(&mut conn, &mut uuids, proj.project));
        }
        let measure = make_measure(&mut conn, &mut uuids, proj.project);
        // Seed metrics WITHOUT the cache, then clear the cache and run the migration's
        // backfill SQL to prove it reconstructs the same counts as the oracle, with the
        // latest report.created per series.
        ingest(
            &mut conn,
            &mut uuids,
            proj,
            testbeds[0],
            benchmarks[0],
            measure,
            at(10),
        );
        ingest(
            &mut conn,
            &mut uuids,
            proj,
            testbeds[0],
            benchmarks[0],
            measure,
            at(30),
        );
        ingest(
            &mut conn,
            &mut uuids,
            proj,
            testbeds[1],
            benchmarks[1],
            measure,
            at(12),
        );
        diesel::delete(super::series_table::table)
            .execute(&mut conn)
            .expect("clear cache");

        sql_query(BACKFILL)
            .execute(&mut conn)
            .expect("run backfill");

        let (start, end) = always();
        assert_eq!(count_active(&mut conn, org, start, end).unwrap(), 2);
        assert_eq!(oracle_count(&mut conn, org, start, end, None), 2);
        // Backfilled last_seen is the latest (30), so the early-only window excludes it.
        assert_eq!(count_active(&mut conn, org, at(0), at(15)).unwrap(), 1);
        assert_eq!(oracle_count(&mut conn, org, at(0), at(15), None), 1);
    }

    // ----- Property test: the backbone -----

    /// Deterministic `SplitMix64`, seeded so the property test is reproducible (no
    /// wall-clock, no external RNG).
    struct Rng(u64);

    impl Rng {
        fn next_u64(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }

        fn below(&mut self, n: u64) -> usize {
            usize::try_from(self.next_u64() % n).expect("fits usize")
        }
    }

    #[test]
    fn cache_equals_oracle_property() {
        // One public and one private project, each with a small pool of entities, plus
        // an extra head per project so branch variation is exercised.
        struct Pool {
            proj: Proj,
            heads: Vec<HeadId>,
            testbeds: Vec<TestbedId>,
            benchmarks: Vec<BenchmarkId>,
            measures: Vec<MeasureId>,
        }

        let mut conn = setup_test_db();
        let mut uuids = Uuids(1);
        let mut rng = Rng(0x5EED);
        let org = make_org(&mut conn, &mut uuids);

        let mut pools: Vec<Pool> = Vec::new();
        for visibility in [Visibility::Public, Visibility::Private] {
            let proj = make_proj(&mut conn, &mut uuids, org, visibility);
            let heads = vec![proj.head, make_head(&mut conn, &mut uuids, proj.project)];
            let mut testbeds = Vec::new();
            let mut benchmarks = Vec::new();
            let mut measures = Vec::new();
            for _ in 0..3 {
                testbeds.push(make_testbed(&mut conn, &mut uuids, proj.project));
                benchmarks.push(make_benchmark(&mut conn, &mut uuids, proj.project));
                measures.push(make_measure(&mut conn, &mut uuids, proj.project));
            }
            pools.push(Pool {
                proj,
                heads,
                testbeds,
                benchmarks,
                measures,
            });
        }

        for round in 0..300 {
            // Pick a random series in a random project and ingest at a random time.
            let (proj, head, testbed, benchmark, measure, end_time) = {
                let pool = &pools[rng.below(pools.len() as u64)];
                (
                    pool.proj,
                    pool.heads[rng.below(pool.heads.len() as u64)],
                    pool.testbeds[rng.below(pool.testbeds.len() as u64)],
                    pool.benchmarks[rng.below(pool.benchmarks.len() as u64)],
                    pool.measures[rng.below(pool.measures.len() as u64)],
                    at(i64::try_from(rng.below(100)).expect("fits i64")),
                )
            };
            ingest_on(
                &mut conn, &mut uuids, proj, head, testbed, benchmark, measure, end_time,
            );

            // Periodically assert the invariant over several random windows.
            if round % 7 == 0 {
                for _ in 0..4 {
                    let a = i64::try_from(rng.below(100)).expect("fits i64");
                    let b = i64::try_from(rng.below(100)).expect("fits i64");
                    let (start, end) = (at(a.min(b)), at(a.max(b)));
                    assert_eq!(
                        count_active(&mut conn, org, start, end).unwrap(),
                        oracle_count(&mut conn, org, start, end, None),
                        "count_active != oracle at round {round} window [{a}, {b}]",
                    );
                    assert_eq!(
                        count_active_private(&mut conn, org, start, end).unwrap(),
                        oracle_count(&mut conn, org, start, end, Some(Visibility::Private)),
                        "count_active_private != oracle at round {round} window [{a}, {b}]",
                    );
                }
            }
        }
    }
}
