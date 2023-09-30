use chrono::offset::Utc;
use diesel::{ExpressionMethods, QueryDsl, RunQueryDsl};

use crate::{
    context::DbConnection,
    model::project::{
        benchmark::BenchmarkId, branch::BranchId, metric_kind::MetricKindId, testbed::TestbedId,
    },
    schema, ApiError,
};

use super::threshold::MetricsStatistic;

pub struct MetricsData {
    pub data: Vec<f64>,
}

impl MetricsData {
    pub fn new(
        conn: &mut DbConnection,
        metric_kind_id: MetricKindId,
        branch_id: BranchId,
        testbed_id: TestbedId,
        benchmark_id: BenchmarkId,
        statistic: &MetricsStatistic,
    ) -> Result<Self, ApiError> {
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
            query = query.filter(
                schema::report::start_time
                    .ge(now.checked_sub(window.into()).ok_or(ApiError::BadMath)?),
            );
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
            .map_err(ApiError::from)?
            .into_iter()
            .collect();

        Ok(Self { data })
    }
}
