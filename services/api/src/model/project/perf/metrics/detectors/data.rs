use chrono::offset::Utc;
use diesel::{ExpressionMethods, JoinOnDsl, QueryDsl, RunQueryDsl, SqliteConnection};
use dropshot::HttpError;

use crate::{error::api_error, model::project::threshold::statistic::QueryStatistic, schema};

pub struct MetricsData {
    pub data: Vec<f64>,
}

impl MetricsData {
    pub fn new(
        conn: &mut SqliteConnection,
        branch_id: i32,
        testbed_id: i32,
        metric_kind_id: i32,
        benchmark_id: i32,
        statistic: &QueryStatistic,
    ) -> Result<Self, HttpError> {
        let mut query = schema::perf::table
            .left_join(
                schema::benchmark::table.on(schema::perf::benchmark_id.eq(schema::benchmark::id)),
            )
            .filter(schema::benchmark::id.eq(benchmark_id))
            .left_join(schema::report::table.on(schema::perf::report_id.eq(schema::report::id)))
            .into_boxed();

        if let Some(window) = statistic.window {
            let now = Utc::now().timestamp_nanos();
            query = query.filter(schema::report::start_time.ge(now - window));
        }

        let mut query = query
            .left_join(
                schema::testbed::table.on(schema::report::testbed_id.eq(schema::testbed::id)),
            )
            .filter(schema::testbed::id.eq(testbed_id))
            .left_join(
                schema::version::table.on(schema::report::version_id.eq(schema::version::id)),
            )
            .left_join(schema::branch::table.on(schema::version::branch_id.eq(schema::branch::id)))
            .filter(schema::branch::id.eq(branch_id))
            .inner_join(schema::metric::table.on(schema::perf::id.eq(schema::metric::perf_id)))
            .filter(schema::metric::metric_kind_id.eq(metric_kind_id))
            .order((
                schema::version::number.desc(),
                schema::report::start_time.desc(),
                schema::perf::iteration.desc(),
            ));

        if let Some(max_sample_size) = statistic.max_sample_size {
            query = query.limit(max_sample_size);
        }

        let data = query
            .select(schema::metric::value)
            .load::<f64>(conn)
            .map_err(api_error!())?
            .into_iter()
            .collect();

        Ok(Self { data })
    }
}
