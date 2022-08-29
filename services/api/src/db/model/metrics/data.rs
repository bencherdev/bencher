use std::collections::VecDeque;

use bencher_json::report::{
    JsonLatency,
    JsonMinMaxAvg,
    JsonThroughput,
};
use chrono::offset::Utc;
use diesel::{
    JoinOnDsl,
    NullableExpressionMethods,
    QueryDsl,
    RunQueryDsl,
    SqliteConnection,
};
use dropshot::HttpError;

use crate::{
    db::{
        model::{
            perf::{
                latency::QueryLatency,
                min_max_avg::QueryMinMaxAvg,
                throughput::QueryThroughput,
            },
            threshold::{
                statistic::QueryStatistic,
                PerfKind,
            },
        },
        schema,
    },
    diesel::ExpressionMethods,
    util::http_error,
};

const PERF_ERROR: &str = "Failed to get perf data.";

pub struct MetricsData {
    pub data: VecDeque<f64>,
}

enum MetricsKind {
    Latency,
    Throughput,
    MinMaxAvg(MinMaxAvgKind),
}

enum MinMaxAvgKind {
    Compute,
    Memory,
    Storage,
}

impl MetricsData {
    pub fn new(
        conn: &SqliteConnection,
        branch_id: i32,
        testbed_id: i32,
        benchmark_id: i32,
        statistic: &QueryStatistic,
        kind: PerfKind,
    ) -> Result<Self, HttpError> {
        let sample_size = unwrap_sample_size(statistic.sample_size);
        let window = unwrap_window(statistic.window);

        let order_by = (
            schema::version::number.desc(),
            schema::report::start_time.desc(),
            schema::perf::iteration.desc(),
        );

        let query = schema::perf::table
            .left_join(
                schema::benchmark::table.on(schema::perf::benchmark_id.eq(schema::benchmark::id)),
            )
            .filter(schema::benchmark::id.eq(benchmark_id))
            .left_join(schema::report::table.on(schema::perf::report_id.eq(schema::report::id)))
            .filter(schema::report::start_time.ge(window))
            .left_join(
                schema::testbed::table.on(schema::report::testbed_id.eq(schema::testbed::id)),
            )
            .filter(schema::testbed::id.eq(testbed_id))
            .left_join(
                schema::version::table.on(schema::report::version_id.eq(schema::version::id)),
            )
            .left_join(schema::branch::table.on(schema::version::branch_id.eq(schema::branch::id)))
            .filter(schema::branch::id.eq(branch_id));

        let data: VecDeque<f64> =
            match kind.into() {
                MetricsKind::Latency => {
                    let json_data: Vec<JsonLatency> = query
                        .inner_join(
                            schema::latency::table
                                .on(schema::perf::latency_id.eq(schema::latency::id.nullable())),
                        )
                        .select((
                            schema::latency::id,
                            schema::latency::uuid,
                            schema::latency::lower_variance,
                            schema::latency::upper_variance,
                            schema::latency::duration,
                        ))
                        .order(&order_by)
                        .limit(sample_size)
                        .load::<QueryLatency>(conn)
                        .map_err(|_| http_error!(PERF_ERROR))?
                        .into_iter()
                        .filter_map(|query| query.to_json().ok())
                        .collect();

                    json_data.into_iter().map(|d| d.duration as f64).collect()
                },
                MetricsKind::Throughput => {
                    let json_data: Vec<JsonThroughput> =
                        query
                            .inner_join(schema::throughput::table.on(
                                schema::perf::throughput_id.eq(schema::throughput::id.nullable()),
                            ))
                            .select((
                                schema::throughput::id,
                                schema::throughput::uuid,
                                schema::throughput::lower_variance,
                                schema::throughput::upper_variance,
                                schema::throughput::events,
                                schema::throughput::unit_time,
                            ))
                            .order(&order_by)
                            .limit(sample_size)
                            .load::<QueryThroughput>(conn)
                            .map_err(|_| http_error!(PERF_ERROR))?
                            .into_iter()
                            .filter_map(|query| query.to_json().ok())
                            .collect();

                    json_data
                        .iter()
                        .map(|d| d.per_unit_time(&d.events).into())
                        .collect()
                },
                MetricsKind::MinMaxAvg(mma) => {
                    let json_data: Vec<JsonMinMaxAvg> =
                        match mma {
                            MinMaxAvgKind::Compute => query
                                .inner_join(schema::min_max_avg::table.on(
                                    schema::perf::compute_id.eq(schema::min_max_avg::id.nullable()),
                                ))
                                .select((
                                    schema::min_max_avg::id,
                                    schema::min_max_avg::uuid,
                                    schema::min_max_avg::min,
                                    schema::min_max_avg::max,
                                    schema::min_max_avg::avg,
                                ))
                                .order(&order_by)
                                .limit(sample_size)
                                .load::<QueryMinMaxAvg>(conn)
                                .map_err(|_| http_error!(PERF_ERROR))?
                                .into_iter()
                                .map(|query| query.to_json())
                                .collect(),
                            MinMaxAvgKind::Memory => query
                                .inner_join(schema::min_max_avg::table.on(
                                    schema::perf::memory_id.eq(schema::min_max_avg::id.nullable()),
                                ))
                                .select((
                                    schema::min_max_avg::id,
                                    schema::min_max_avg::uuid,
                                    schema::min_max_avg::min,
                                    schema::min_max_avg::max,
                                    schema::min_max_avg::avg,
                                ))
                                .order(&order_by)
                                .limit(sample_size)
                                .load::<QueryMinMaxAvg>(conn)
                                .map_err(|_| http_error!(PERF_ERROR))?
                                .into_iter()
                                .map(|query| query.to_json())
                                .collect(),
                            MinMaxAvgKind::Storage => query
                                .inner_join(schema::min_max_avg::table.on(
                                    schema::perf::storage_id.eq(schema::min_max_avg::id.nullable()),
                                ))
                                .select((
                                    schema::min_max_avg::id,
                                    schema::min_max_avg::uuid,
                                    schema::min_max_avg::min,
                                    schema::min_max_avg::max,
                                    schema::min_max_avg::avg,
                                ))
                                .order(&order_by)
                                .limit(sample_size)
                                .load::<QueryMinMaxAvg>(conn)
                                .map_err(|_| http_error!(PERF_ERROR))?
                                .into_iter()
                                .map(|query| query.to_json())
                                .collect(),
                        };

                    json_data.iter().map(|d| d.avg.into()).collect()
                },
            };

        Ok(Self { data })
    }
}

fn unwrap_sample_size(sample_size: Option<i64>) -> i64 {
    sample_size.unwrap_or(i64::MAX)
}

fn unwrap_window(window: Option<i64>) -> i64 {
    window
        .map(|window| {
            let now = Utc::now().timestamp_nanos();
            now - window
        })
        .unwrap_or_default()
}

impl From<PerfKind> for MetricsKind {
    fn from(kind: PerfKind) -> Self {
        match kind {
            PerfKind::Latency => Self::Latency,
            PerfKind::Throughput => Self::Throughput,
            PerfKind::Compute => Self::MinMaxAvg(MinMaxAvgKind::Compute),
            PerfKind::Memory => Self::MinMaxAvg(MinMaxAvgKind::Memory),
            PerfKind::Storage => Self::MinMaxAvg(MinMaxAvgKind::Storage),
        }
    }
}
