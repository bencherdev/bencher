use bencher_boundary::MetricsData;
use chrono::offset::Utc;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};
use dropshot::HttpError;
use slog::{warn, Logger};

use crate::{
    context::DbConnection,
    error::not_found_error,
    model::project::{
        benchmark::BenchmarkId, branch::BranchId, metric_kind::MetricKindId, testbed::TestbedId,
    },
    schema,
};

use super::threshold::MetricsStatistic;

pub fn metrics_data(
    log: &Logger,
    conn: &mut DbConnection,
    metric_kind_id: MetricKindId,
    branch_id: BranchId,
    testbed_id: TestbedId,
    benchmark_id: BenchmarkId,
    statistic: &MetricsStatistic,
) -> Result<MetricsData, HttpError> {
    let mut query = schema::metric::table
        .filter(schema::metric::metric_kind_id.eq(metric_kind_id))
        .inner_join(
            schema::perf::table
                .inner_join(
                    schema::report::table
                        .inner_join(schema::version::table.inner_join(
                            schema::branch_version::table.inner_join(schema::branch::table),
                        ))
                        .inner_join(schema::testbed::table),
                )
                .inner_join(schema::benchmark::table),
        )
        .filter(schema::branch::id.eq(branch_id))
        .filter(schema::testbed::id.eq(testbed_id))
        .filter(schema::benchmark::id.eq(benchmark_id))
        .into_boxed();

    if let Some(window) = statistic.window {
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
        schema::perf::iteration.desc(),
    ));

    if let Some(max_sample_size) = statistic.max_sample_size {
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
