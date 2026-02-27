use bencher_json::{BoundaryUuid, project::boundary::BoundaryLimit};
use diesel::RunQueryDsl as _;

use crate::macros::sql::last_insert_rowid;
use crate::{
    context::DbConnection,
    model::project::{
        metric::MetricId,
        threshold::{
            ThresholdId,
            alert::InsertAlert,
            boundary::{BoundaryId, InsertBoundary},
            model::ModelId,
        },
    },
    schema,
};

/// Pre-computed detection result from Phase 1 (reads + compute).
/// Contains all data needed to write boundary and optional alert in Phase 2.
pub struct PreparedDetection {
    pub threshold_id: ThresholdId,
    pub model_id: ModelId,
    pub boundary_uuid: BoundaryUuid,
    pub baseline: Option<f64>,
    pub lower_limit: Option<f64>,
    pub upper_limit: Option<f64>,
    pub outlier: Option<BoundaryLimit>,
    pub ignore_benchmark: bool,
}

impl PreparedDetection {
    /// Write this prepared detection (boundary + optional alert) into the database
    /// using the provided connection (expected to be within a transaction).
    pub fn write(self, conn: &mut DbConnection, metric_id: MetricId) -> diesel::QueryResult<()> {
        let Self {
            threshold_id,
            model_id,
            boundary_uuid,
            baseline,
            lower_limit,
            upper_limit,
            outlier,
            ignore_benchmark,
        } = self;

        let insert_boundary = InsertBoundary {
            uuid: boundary_uuid,
            threshold_id,
            model_id,
            metric_id,
            baseline,
            lower_limit,
            upper_limit,
        };

        diesel::insert_into(schema::boundary::table)
            .values(&insert_boundary)
            .execute(conn)?;

        let boundary_id = diesel::select(last_insert_rowid()).get_result::<BoundaryId>(conn)?;

        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::MetricCreate);

        if !ignore_benchmark && let Some(boundary_limit) = outlier {
            InsertAlert::insert(conn, boundary_id, boundary_limit)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::{BoundaryUuid, project::boundary::BoundaryLimit};
    use diesel::{Connection as _, ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};

    use crate::{
        context::DbConnection,
        model::project::{
            metric::MetricId,
            threshold::{ThresholdId, model::ModelId},
        },
        schema,
        test_util::{
            create_base_entities, create_benchmark, create_branch_with_head, create_head_version,
            create_measure, create_metric, create_model, create_report, create_report_benchmark,
            create_testbed, create_threshold, create_version, setup_test_db,
        },
    };

    use super::PreparedDetection;

    /// Set up the full entity chain needed for `PreparedDetection::write` tests.
    /// Returns `(threshold_id, model_id, metric_id)`.
    fn setup_prepared_detection_entities(
        conn: &mut DbConnection,
    ) -> (ThresholdId, ModelId, MetricId) {
        let base = create_base_entities(conn);
        let branch = create_branch_with_head(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000010",
            "main",
            "main",
            "00000000-0000-0000-0000-000000000011",
        );
        let testbed = create_testbed(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000020",
            "localhost",
            "localhost",
        );
        let measure = create_measure(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000030",
            "latency",
            "latency",
        );
        let threshold_id = create_threshold(
            conn,
            base.project_id,
            branch.branch_id,
            testbed,
            measure,
            "00000000-0000-0000-0000-000000000040",
        );
        let model_id = create_model(
            conn,
            threshold_id,
            "00000000-0000-0000-0000-000000000050",
            0,
        );
        let version_id = create_version(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000060",
            0,
            None,
        );
        create_head_version(conn, branch.head_id, version_id);
        let report_id = create_report(
            conn,
            "00000000-0000-0000-0000-000000000070",
            base.project_id,
            branch.head_id,
            version_id,
            testbed,
        );
        let benchmark_id = create_benchmark(
            conn,
            base.project_id,
            "00000000-0000-0000-0000-000000000080",
            "bench1",
            "bench1",
        );
        let report_benchmark_id = create_report_benchmark(
            conn,
            "00000000-0000-0000-0000-000000000090",
            report_id,
            0,
            benchmark_id,
        );
        let metric_id = create_metric(
            conn,
            "00000000-0000-0000-0000-0000000000a0",
            report_benchmark_id,
            measure,
            100.0,
        );

        (threshold_id, model_id, metric_id)
    }

    #[test]
    fn prepared_detection_write_inserts_boundary() {
        let mut conn = setup_test_db();
        let (threshold_id, model_id, metric_id) = setup_prepared_detection_entities(&mut conn);

        let detection = PreparedDetection {
            threshold_id,
            model_id,
            boundary_uuid: BoundaryUuid::new(),
            baseline: Some(50.0),
            lower_limit: Some(10.0),
            upper_limit: Some(90.0),
            outlier: None,
            ignore_benchmark: false,
        };

        conn.transaction(|conn| detection.write(conn, metric_id))
            .expect("Failed to write detection");

        // Assert 1 boundary row exists with correct fields
        let boundary_count: i64 = schema::boundary::table
            .filter(schema::boundary::threshold_id.eq(threshold_id))
            .filter(schema::boundary::model_id.eq(model_id))
            .filter(schema::boundary::metric_id.eq(metric_id))
            .count()
            .get_result(&mut conn)
            .expect("Failed to count boundaries");
        assert_eq!(boundary_count, 1);

        // Assert 0 alert rows exist
        let alert_count: i64 = schema::alert::table
            .count()
            .get_result(&mut conn)
            .expect("Failed to count alerts");
        assert_eq!(alert_count, 0);
    }

    #[test]
    fn prepared_detection_write_creates_alert_on_outlier() {
        let mut conn = setup_test_db();
        let (threshold_id, model_id, metric_id) = setup_prepared_detection_entities(&mut conn);

        let detection = PreparedDetection {
            threshold_id,
            model_id,
            boundary_uuid: BoundaryUuid::new(),
            baseline: Some(50.0),
            lower_limit: Some(10.0),
            upper_limit: Some(90.0),
            outlier: Some(BoundaryLimit::Upper),
            ignore_benchmark: false,
        };

        conn.transaction(|conn| detection.write(conn, metric_id))
            .expect("Failed to write detection");

        // Assert 1 boundary exists
        let boundary_count: i64 = schema::boundary::table
            .filter(schema::boundary::threshold_id.eq(threshold_id))
            .count()
            .get_result(&mut conn)
            .expect("Failed to count boundaries");
        assert_eq!(boundary_count, 1);

        // Assert 1 alert exists
        let alert_count: i64 = schema::alert::table
            .count()
            .get_result(&mut conn)
            .expect("Failed to count alerts");
        assert_eq!(alert_count, 1);
    }

    #[test]
    fn prepared_detection_write_skips_alert_when_ignore_benchmark() {
        let mut conn = setup_test_db();
        let (threshold_id, model_id, metric_id) = setup_prepared_detection_entities(&mut conn);

        let detection = PreparedDetection {
            threshold_id,
            model_id,
            boundary_uuid: BoundaryUuid::new(),
            baseline: Some(50.0),
            lower_limit: Some(10.0),
            upper_limit: Some(90.0),
            outlier: Some(BoundaryLimit::Upper),
            ignore_benchmark: true,
        };

        conn.transaction(|conn| detection.write(conn, metric_id))
            .expect("Failed to write detection");

        // Assert 1 boundary exists
        let boundary_count: i64 = schema::boundary::table
            .filter(schema::boundary::threshold_id.eq(threshold_id))
            .count()
            .get_result(&mut conn)
            .expect("Failed to count boundaries");
        assert_eq!(boundary_count, 1);

        // Assert 0 alerts exist (outlier ignored due to ignore_benchmark)
        let alert_count: i64 = schema::alert::table
            .count()
            .get_result(&mut conn)
            .expect("Failed to count alerts");
        assert_eq!(alert_count, 0);
    }
}
