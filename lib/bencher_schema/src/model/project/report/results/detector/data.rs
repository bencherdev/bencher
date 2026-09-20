use bencher_boundary::MetricsData;
use bencher_json::MetricName;
use chrono::offset::Utc;
use diesel::{
    QueryResult, RunQueryDsl,
    query_builder::{AstPass, Query, QueryFragment, QueryId},
    sql_types::{BigInt, Double, Integer, Text},
    sqlite::Sqlite,
};
use dropshot::HttpError;
use slog::{Logger, warn};

use crate::{
    context::DbConnection,
    error::not_found_error,
    model::{
        project::{
            benchmark::BenchmarkId, branch::head::HeadId, measure::MeasureId, testbed::TestbedId,
            variant::VariantId,
        },
        spec::SpecId,
    },
};

pub fn metrics_data(
    log: &Logger,
    conn: &mut DbConnection,
    detector: &super::Detector,
    benchmark_id: BenchmarkId,
    variant_id: VariantId,
) -> Result<MetricsData, HttpError> {
    let model = &detector.threshold.model;
    let start_time = model.window.and_then(|window| {
        let now = Utc::now().timestamp();
        let start_time = now.checked_sub(window.into());
        if start_time.is_none() {
            debug_assert!(false, "window > i64::MIN");
            warn!(
                log,
                "Window is too large, ignoring. But this should never happen: window {window} > i64::MIN for now {now}"
            );
        }
        start_time
    });

    let data = HistoryQuery {
        head_id: detector.head_id,
        testbed_id: detector.testbed_id,
        benchmark_id,
        variant_id,
        measure_id: detector.measure_id,
        // A threshold checks one name, and the sample it checks against is that
        // name's history and nothing else. A bare threshold names `value`.
        metric_name: detector.threshold.metric.clone(),
        spec_id: detector.spec_id,
        start_time,
        max_sample_size: model.max_sample_size.map(i64::from),
    }
    .load::<f64>(conn)
    .map_err(not_found_error)?;

    Ok(MetricsData { data })
}

// The sample is one variant's history of the one metric name a threshold checks,
// never pooled variants or the other names stored beside it. A head's version
// numbers rise with its version ids, so walking the head's index newest first keeps
// version order and stops once the limit is met, and pinning the benchmark index
// keeps each report from scanning all of its results.
struct HistoryQuery {
    head_id: HeadId,
    testbed_id: TestbedId,
    benchmark_id: BenchmarkId,
    variant_id: VariantId,
    measure_id: MeasureId,
    metric_name: MetricName,
    spec_id: Option<SpecId>,
    start_time: Option<i64>,
    max_sample_size: Option<i64>,
}

impl Query for HistoryQuery {
    type SqlType = Double;
}

impl QueryId for HistoryQuery {
    type QueryId = ();
    // The optional filters change the SQL, so the prepared statement cache has to key
    // on the text it builds rather than on this type.
    const HAS_STATIC_QUERY_ID: bool = false;
}

impl<Conn> RunQueryDsl<Conn> for HistoryQuery {}

impl QueryFragment<Sqlite> for HistoryQuery {
    fn walk_ast<'b>(&'b self, mut pass: AstPass<'_, 'b, Sqlite>) -> QueryResult<()> {
        let Self {
            head_id,
            testbed_id,
            benchmark_id,
            variant_id,
            measure_id,
            metric_name,
            spec_id,
            start_time,
            max_sample_size,
        } = self;
        pass.push_sql(
            "SELECT metric.value FROM head_version \
             CROSS JOIN report \
             CROSS JOIN report_benchmark INDEXED BY index_report_benchmark_benchmark_report \
             CROSS JOIN metric \
             WHERE head_version.head_id = ",
        );
        pass.push_bind_param::<Integer, _>(head_id)?;
        pass.push_sql(" AND report.testbed_id = ");
        pass.push_bind_param::<Integer, _>(testbed_id)?;
        pass.push_sql(" AND report_benchmark.benchmark_id = ");
        pass.push_bind_param::<Integer, _>(benchmark_id)?;
        pass.push_sql(" AND report_benchmark.variant_id = ");
        pass.push_bind_param::<Integer, _>(variant_id)?;
        pass.push_sql(" AND metric.measure_id = ");
        pass.push_bind_param::<Integer, _>(measure_id)?;
        pass.push_sql(" AND metric.name = ");
        pass.push_bind_param::<Text, _>(metric_name)?;
        pass.push_sql(
            " AND report.version_id = head_version.version_id \
             AND report_benchmark.report_id = report.id \
             AND metric.report_benchmark_id = report_benchmark.id",
        );
        if let Some(spec_id) = spec_id {
            pass.push_sql(" AND report.spec_id = ");
            pass.push_bind_param::<Integer, _>(spec_id)?;
        }
        if let Some(start_time) = start_time {
            pass.push_sql(" AND report.start_time >= ");
            pass.push_bind_param::<BigInt, _>(start_time)?;
        }
        pass.push_sql(
            " ORDER BY head_version.version_id DESC, report.start_time DESC, report_benchmark.iteration DESC",
        );
        if let Some(max_sample_size) = max_sample_size {
            pass.push_sql(" LIMIT ");
            pass.push_bind_param::<BigInt, _>(max_sample_size)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::{
        DateTime, MetricName, ModelTest, ParameterSet, SampleSize, ThresholdUuid, Window,
    };
    use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _, SqliteConnection};
    use pretty_assertions::assert_eq;

    use super::metrics_data;
    use crate::{
        model::{
            project::{
                ProjectId,
                benchmark::BenchmarkId,
                branch::{head::HeadId, version::VersionId},
                measure::MeasureId,
                report::{
                    ReportId,
                    results::detector::{
                        Detector,
                        threshold::{Threshold, ThresholdModel},
                    },
                },
                testbed::TestbedId,
                threshold::{ThresholdId, model::ModelId},
                variant::VariantId,
            },
            spec::SpecId,
        },
        schema,
        test_util::{
            CreateSpecArgs, create_base_entities, create_benchmark, create_branch_with_head,
            create_head_version, create_measure, create_metric, create_named_metric, create_report,
            create_report_benchmark, create_report_benchmark_for_variant, create_spec,
            create_testbed, create_variant, create_version, get_empty_variant, set_report_spec,
            setup_test_db,
        },
    };

    const FUTURE: i64 = 4_000_000_000;

    struct Fixture {
        conn: SqliteConnection,
        main_head: HeadId,
        feature_head: HeadId,
        testbed: TestbedId,
        benchmark: BenchmarkId,
        variant: VariantId,
        measure: MeasureId,
        spec: SpecId,
    }

    fn uuid(n: u32) -> String {
        format!("00000000-0000-0000-0000-{n:012}")
    }

    struct Seed<'a> {
        conn: &'a mut SqliteConnection,
        project_id: ProjectId,
        next: u32,
    }

    impl Seed<'_> {
        fn uuid(&mut self) -> String {
            self.next += 1;
            uuid(self.next)
        }

        fn report(
            &mut self,
            head_id: HeadId,
            version_id: VersionId,
            testbed_id: TestbedId,
            start_time: i64,
        ) -> ReportId {
            let report_uuid = self.uuid();
            let report_id = create_report(
                self.conn,
                &report_uuid,
                self.project_id,
                head_id,
                version_id,
                testbed_id,
            );
            diesel::update(schema::report::table.filter(schema::report::id.eq(report_id)))
                .set(schema::report::start_time.eq(start_time))
                .execute(self.conn)
                .expect("Failed to set report start time");
            report_id
        }
    }

    /// Main holds versions 0, 1, and its own 2; the feature branch is cloned from
    /// main at version 1 and adds its own 2.
    #[expect(
        clippy::too_many_lines,
        reason = "one fixture seeds every history case"
    )]
    fn fixture() -> Fixture {
        let mut conn = setup_test_db();
        let base = create_base_entities(&mut conn);
        let mut seed = Seed {
            conn: &mut conn,
            project_id: base.project_id,
            next: 100,
        };
        let project_id = base.project_id;

        let main =
            create_branch_with_head(seed.conn, project_id, &uuid(1), "main", "main", &uuid(2));
        let feature = create_branch_with_head(
            seed.conn,
            project_id,
            &uuid(3),
            "feature",
            "feature",
            &uuid(4),
        );
        let testbed = create_testbed(seed.conn, project_id, &uuid(5), "one", "one");
        let other_testbed = create_testbed(seed.conn, project_id, &uuid(6), "two", "two");
        let benchmark = create_benchmark(seed.conn, project_id, &uuid(7), "bench", "bench");
        let other_benchmark = create_benchmark(seed.conn, project_id, &uuid(8), "other", "other");
        let variant = get_empty_variant(seed.conn, benchmark);
        let other_variant = create_variant(
            seed.conn,
            benchmark,
            &"{\"n\":1}".parse::<ParameterSet>().expect("parameters"),
        );
        let measure = create_measure(seed.conn, project_id, &uuid(9), "latency", "latency");
        let other_measure = create_measure(seed.conn, project_id, &uuid(10), "memory", "memory");
        let spec = create_spec(
            seed.conn,
            CreateSpecArgs {
                uuid: &uuid(11),
                name: "spec",
                slug: "spec",
                os: "linux",
                architecture: "x86_64",
                cpu: 1,
                memory: 1,
                disk: 1,
                network: false,
            },
        );

        let v0 = create_version(seed.conn, project_id, &uuid(12), 0, None);
        let v1 = create_version(seed.conn, project_id, &uuid(13), 1, None);
        let v2_feature = create_version(seed.conn, project_id, &uuid(14), 2, None);
        let v2_main = create_version(seed.conn, project_id, &uuid(15), 2, None);
        for version_id in [v0, v1, v2_main] {
            create_head_version(seed.conn, main.head_id, version_id);
        }
        for version_id in [v0, v1, v2_feature] {
            create_head_version(seed.conn, feature.head_id, version_id);
        }

        let test = DateTime::TEST.timestamp();
        // The oldest version's report starts last, so version order must win.
        let r0 = seed.report(main.head_id, v0, testbed, FUTURE + 1);
        let r1a = seed.report(main.head_id, v1, testbed, test);
        let r1b = seed.report(main.head_id, v1, testbed, test);
        let r1c = seed.report(main.head_id, v1, testbed, FUTURE);
        let r2f = seed.report(feature.head_id, v2_feature, testbed, FUTURE);
        let r2m = seed.report(main.head_id, v2_main, testbed, test);
        let r2f_other_testbed = seed.report(feature.head_id, v2_feature, other_testbed, FUTURE);
        for report_id in [r0, r1a, r2f] {
            set_report_spec(seed.conn, report_id, spec);
        }

        for (report_id, iteration, value) in [
            (r0, 0, 1.0),
            (r1a, 0, 2.0),
            (r1b, 1, 2.1),
            (r1a, 2, 2.2),
            (r1c, 0, 2.5),
            (r2f, 0, 3.0),
            (r2m, 0, 4.0),
            (r2f_other_testbed, 0, 30.0),
        ] {
            let rb_uuid = seed.uuid();
            let rb = create_report_benchmark(seed.conn, &rb_uuid, report_id, iteration, benchmark);
            let metric_uuid = seed.uuid();
            create_metric(seed.conn, &metric_uuid, rb, measure, value);
        }

        let rb_uuid = seed.uuid();
        let rb = create_report_benchmark(seed.conn, &rb_uuid, r2f, 1, benchmark);
        let metric_uuid = seed.uuid();
        create_metric(seed.conn, &metric_uuid, rb, other_measure, 90.0);
        let metric_uuid = seed.uuid();
        create_named_metric(
            seed.conn,
            &metric_uuid,
            rb,
            measure,
            &MetricName::lower_value(),
            91.0,
        );
        let rb_uuid = seed.uuid();
        let rb = create_report_benchmark_for_variant(
            seed.conn,
            &rb_uuid,
            r2f,
            2,
            benchmark,
            other_variant,
        );
        let metric_uuid = seed.uuid();
        create_metric(seed.conn, &metric_uuid, rb, measure, 92.0);
        let rb_uuid = seed.uuid();
        let rb = create_report_benchmark(seed.conn, &rb_uuid, r2f, 3, other_benchmark);
        let metric_uuid = seed.uuid();
        create_metric(seed.conn, &metric_uuid, rb, measure, 93.0);

        Fixture {
            conn,
            main_head: main.head_id,
            feature_head: feature.head_id,
            testbed,
            benchmark,
            variant,
            measure,
            spec,
        }
    }

    fn history(
        fixture: &mut Fixture,
        head_id: HeadId,
        spec: bool,
        window: Option<u32>,
        max_sample_size: Option<u32>,
    ) -> Vec<f64> {
        let detector = Detector {
            head_id,
            testbed_id: fixture.testbed,
            spec_id: spec.then_some(fixture.spec),
            measure_id: fixture.measure,
            threshold: Threshold {
                id: ThresholdId::default(),
                uuid: ThresholdUuid::default(),
                parameters: None,
                metric: MetricName::value(),
                model: ThresholdModel {
                    id: ModelId::default(),
                    test: ModelTest::TTest,
                    min_sample_size: None,
                    max_sample_size: max_sample_size
                        .map(|size| SampleSize::try_from(size).expect("sample size")),
                    window: window.map(|window| Window::try_from(window).expect("window")),
                    lower_boundary: None,
                    upper_boundary: None,
                },
            },
        };
        let log = slog::Logger::root(slog::Discard, slog::o!());
        metrics_data(
            &log,
            &mut fixture.conn,
            &detector,
            fixture.benchmark,
            fixture.variant,
        )
        .expect("Failed to query metrics data")
        .data
    }

    #[test]
    fn metrics_data_newest_version_first() {
        let mut fixture = fixture();
        let (main, feature) = (fixture.main_head, fixture.feature_head);
        let day = Some(86_400);

        assert_eq!(
            history(&mut fixture, feature, false, None, None),
            [3.0, 2.5, 2.2, 2.1, 2.0, 1.0]
        );
        assert_eq!(
            history(&mut fixture, main, false, None, None),
            [4.0, 2.5, 2.2, 2.1, 2.0, 1.0]
        );
        assert_eq!(
            history(&mut fixture, feature, false, None, Some(4)),
            [3.0, 2.5, 2.2, 2.1]
        );
        assert_eq!(
            history(&mut fixture, feature, true, None, None),
            [3.0, 2.2, 2.0, 1.0]
        );
        assert_eq!(
            history(&mut fixture, feature, true, None, Some(3)),
            [3.0, 2.2, 2.0]
        );
        assert_eq!(
            history(&mut fixture, feature, false, day, None),
            [3.0, 2.5, 1.0]
        );
        assert_eq!(
            history(&mut fixture, feature, false, day, Some(2)),
            [3.0, 2.5]
        );
        assert_eq!(history(&mut fixture, feature, true, day, None), [3.0, 1.0]);
        assert_eq!(history(&mut fixture, feature, true, day, Some(1)), [3.0]);
        assert_eq!(history(&mut fixture, main, true, day, Some(8)), [1.0]);
    }
}
