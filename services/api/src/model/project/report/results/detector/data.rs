use bencher_boundary::MetricsData;
use chrono::offset::Utc;
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl};
use dropshot::HttpError;
use slog::{warn, Logger};

use crate::{
    context::DbConnection,
    error::not_found_error,
    model::project::{
        benchmark::BenchmarkId, branch::BranchId, measure::MeasureId, testbed::TestbedId,
    },
    schema,
};

use super::threshold::ThresholdModel;

pub fn metrics_data(
    log: &Logger,
    conn: &mut DbConnection,
    branch_id: BranchId,
    testbed_id: TestbedId,
    benchmark_id: BenchmarkId,
    measure_id: MeasureId,
    model: &ThresholdModel,
) -> Result<MetricsData, HttpError> {
    let mut query =
        schema::metric::table
            .inner_join(
                schema::report_benchmark::table
                    .inner_join(
                        schema::report::table
                            .inner_join(schema::version::table.inner_join(
                                schema::branch_version::table.inner_join(
                                    schema::branch::table.on(
                                        schema::branch_version::branch_id.eq(schema::branch::id),
                                    ),
                                ),
                            ))
                            .inner_join(schema::testbed::table),
                    )
                    .inner_join(schema::benchmark::table),
            )
            .filter(schema::branch::id.eq(branch_id))
            .filter(schema::testbed::id.eq(testbed_id))
            .filter(schema::benchmark::id.eq(benchmark_id))
            .filter(schema::metric::measure_id.eq(measure_id))
            .into_boxed();

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
