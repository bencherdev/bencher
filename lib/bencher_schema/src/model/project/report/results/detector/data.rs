use bencher_boundary::MetricsData;
use chrono::offset::Utc;
use diesel::{ExpressionMethods as _, JoinOnDsl as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;
use slog::{Logger, warn};

use crate::{
    context::DbConnection,
    error::not_found_error,
    model::project::{benchmark::BenchmarkId, parameter::ParameterId},
    schema,
};

pub fn metrics_data(
    log: &Logger,
    conn: &mut DbConnection,
    detector: &super::Detector,
    benchmark_id: BenchmarkId,
    parameter_id: ParameterId,
) -> Result<MetricsData, HttpError> {
    let mut query = schema::metric::table
        .inner_join(
            schema::report_benchmark::table
                .inner_join(
                    schema::report::table
                        .inner_join(
                            schema::version::table.inner_join(
                                schema::head_version::table.inner_join(
                                    schema::head::table
                                        .on(schema::head_version::head_id.eq(schema::head::id)),
                                ),
                            ),
                        )
                        .inner_join(schema::testbed::table),
                )
                .inner_join(schema::benchmark::table),
        )
        .filter(schema::head::id.eq(detector.head_id))
        .filter(schema::testbed::id.eq(detector.testbed_id))
        .filter(schema::benchmark::id.eq(benchmark_id))
        // A parameter set is a grid point with its own series history, so the
        // sample is the one grid point's, never the benchmark's grid pooled.
        .filter(schema::report_benchmark::parameter_id.eq(parameter_id))
        .filter(schema::metric::measure_id.eq(detector.measure_id))
        // A threshold gates one name, and the sample it gates against is that name's
        // history and nothing else: a threshold on `p99` is tested against `p99`
        // rows, never against the `value` rows beside them. A bare threshold names
        // `value`, which is the sample detection has always drawn.
        .filter(schema::metric::name.eq(detector.threshold.metric.clone()))
        .into_boxed();

    if let Some(spec_id) = detector.spec_id {
        query = query.filter(schema::report::spec_id.eq(spec_id));
    }

    let model = &detector.threshold.model;
    if let Some(window) = model.window {
        let now = Utc::now().timestamp();
        if let Some(start_time) = now.checked_sub(window.into()) {
            query = query.filter(schema::report::start_time.ge(start_time));
        } else {
            debug_assert!(false, "window > i64::MIN");
            warn!(
                log,
                "Window is too large, ignoring. But this should never happen: window {window} > i64::MIN for now {now}"
            );
        }
    }

    let mut query = query.order((
        schema::version::number.desc(),
        schema::report::start_time.desc(),
        schema::report_benchmark::iteration.desc(),
    ));

    if let Some(max_sample_size) = model.max_sample_size {
        query = query.limit(max_sample_size.into());
    }

    let data = query
        .select(schema::metric::value)
        .load::<f64>(conn)
        .map_err(not_found_error)?
        .into_iter()
        .collect();

    Ok(MetricsData { data })
}
