use bencher_json::report::{
    data::{
        JsonReportAlert,
        JsonReportAlerts,
    },
    new::{
        mean::Mean,
        JsonBenchmarks,
        JsonMetrics,
    },
    JsonLatency,
    JsonMetricsMap,
    JsonMinMaxAvg,
    JsonThroughput,
};
use chrono::offset::Utc;
use diesel::{
    expression_methods::BoolExpressionMethods,
    JoinOnDsl,
    NullableExpressionMethods,
    QueryDsl,
    RunQueryDsl,
    SqliteConnection,
};
use dropshot::HttpError;
use ordered_float::OrderedFloat;
use uuid::Uuid;

use super::thresholds::Statistic;
use crate::{
    db::{
        model::{
            benchmark::{
                InsertBenchmark,
                QueryBenchmark,
            },
            metrics::thresholds::{
                Threshold,
                Thresholds,
            },
            perf::{
                latency::QueryLatency,
                min_max_avg::QueryMinMaxAvg,
                throughput::QueryThroughput,
                InsertPerf,
                QueryPerf,
            },
            threshold::{
                alert::InsertAlert,
                statistic::StatisticKind,
                PerfKind,
            },
        },
        schema,
        schema::statistic as statistic_table,
    },
    diesel::ExpressionMethods,
    util::http_error,
};

const PERF_ERROR: &str = "Failed to create perf statistic.";

#[derive(Debug)]
struct SampleMean {
    pub latency:    Option<JsonLatency>,
    pub throughput: Option<JsonThroughput>,
    pub compute:    Option<JsonMinMaxAvg>,
    pub memory:     Option<JsonMinMaxAvg>,
    pub storage:    Option<JsonMinMaxAvg>,
}

pub enum SampleMeanKind {
    Latency(Option<JsonLatency>),
    Throughput(Option<JsonThroughput>),
    MinMaxAvg(Option<JsonMinMaxAvg>),
}

pub enum SampleKind {
    Latency,
    Throughput,
    MinMaxAvg(MinMaxAvgKind),
}

pub enum MinMaxAvgKind {
    Compute,
    Memory,
    Storage,
}

impl SampleMean {
    pub fn new(
        conn: &SqliteConnection,
        branch_id: i32,
        testbed_id: i32,
        benchmark_id: i32,
        thresholds: &Thresholds,
    ) -> Result<Self, HttpError> {
        Ok(Self {
            latency:    map_latency(
                conn,
                branch_id,
                testbed_id,
                benchmark_id,
                thresholds.latency.as_ref(),
            )?,
            throughput: map_throughput(
                conn,
                branch_id,
                testbed_id,
                benchmark_id,
                thresholds.throughput.as_ref(),
            )?,
            compute:    map_min_max_avg(
                conn,
                branch_id,
                testbed_id,
                benchmark_id,
                thresholds.compute.as_ref(),
                MinMaxAvgKind::Compute,
            )?,
            memory:     map_min_max_avg(
                conn,
                branch_id,
                testbed_id,
                benchmark_id,
                thresholds.memory.as_ref(),
                MinMaxAvgKind::Memory,
            )?,
            storage:    map_min_max_avg(
                conn,
                branch_id,
                testbed_id,
                benchmark_id,
                thresholds.storage.as_ref(),
                MinMaxAvgKind::Storage,
            )?,
        })
    }
}

// TODO move over to generics instead
fn map_latency(
    conn: &SqliteConnection,
    branch_id: i32,
    testbed_id: i32,
    benchmark_id: i32,
    threshold: Option<&Threshold>,
) -> Result<Option<JsonLatency>, HttpError> {
    Ok(if let Some(threshold) = threshold {
        if let SampleMeanKind::Latency(json) = SampleMeanKind::new(
            conn,
            branch_id,
            testbed_id,
            benchmark_id,
            &threshold.statistic,
            SampleKind::Latency,
        )? {
            json
        } else {
            return Err(http_error!(PERF_ERROR));
        }
    } else {
        None
    })
}

fn map_throughput(
    conn: &SqliteConnection,
    branch_id: i32,
    testbed_id: i32,
    benchmark_id: i32,
    threshold: Option<&Threshold>,
) -> Result<Option<JsonThroughput>, HttpError> {
    Ok(if let Some(threshold) = threshold {
        if let SampleMeanKind::Throughput(json) = SampleMeanKind::new(
            conn,
            branch_id,
            testbed_id,
            benchmark_id,
            &threshold.statistic,
            SampleKind::Throughput,
        )? {
            json
        } else {
            return Err(http_error!(PERF_ERROR));
        }
    } else {
        None
    })
}

fn map_min_max_avg(
    conn: &SqliteConnection,
    branch_id: i32,
    testbed_id: i32,
    benchmark_id: i32,
    threshold: Option<&Threshold>,
    kind: MinMaxAvgKind,
) -> Result<Option<JsonMinMaxAvg>, HttpError> {
    Ok(if let Some(threshold) = threshold {
        if let SampleMeanKind::MinMaxAvg(json) = SampleMeanKind::new(
            conn,
            branch_id,
            testbed_id,
            benchmark_id,
            &threshold.statistic,
            SampleKind::MinMaxAvg(kind),
        )? {
            json
        } else {
            return Err(http_error!(PERF_ERROR));
        }
    } else {
        None
    })
}

impl SampleMeanKind {
    fn new(
        conn: &SqliteConnection,
        branch_id: i32,
        testbed_id: i32,
        benchmark_id: i32,
        statistic: &Statistic,
        kind: SampleKind,
    ) -> Result<Self, HttpError> {
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
            .filter(schema::report::start_time.ge(statistic.window))
            .left_join(
                schema::testbed::table.on(schema::report::testbed_id.eq(schema::testbed::id)),
            )
            .filter(schema::testbed::id.eq(testbed_id))
            .left_join(
                schema::version::table.on(schema::report::version_id.eq(schema::version::id)),
            )
            .left_join(schema::branch::table.on(schema::version::branch_id.eq(schema::branch::id)))
            .filter(schema::branch::id.eq(branch_id));

        match kind {
            SampleKind::Latency => {
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
                    .limit(statistic.sample_size)
                    .load::<QueryLatency>(conn)
                    .map_err(|_| http_error!(PERF_ERROR))?
                    .into_iter()
                    .filter_map(|query| query.to_json().ok())
                    .collect();

                Ok(SampleMeanKind::Latency(JsonLatency::mean(json_data)))
            },
            SampleKind::Throughput => {
                let json_data: Vec<JsonThroughput> = query
                    .inner_join(
                        schema::throughput::table
                            .on(schema::perf::throughput_id.eq(schema::throughput::id.nullable())),
                    )
                    .select((
                        schema::throughput::id,
                        schema::throughput::uuid,
                        schema::throughput::lower_variance,
                        schema::throughput::upper_variance,
                        schema::throughput::events,
                        schema::throughput::unit_time,
                    ))
                    .order(&order_by)
                    .limit(statistic.sample_size)
                    .load::<QueryThroughput>(conn)
                    .map_err(|_| http_error!(PERF_ERROR))?
                    .into_iter()
                    .filter_map(|query| query.to_json().ok())
                    .collect();

                Ok(SampleMeanKind::Throughput(JsonThroughput::mean(json_data)))
            },
            SampleKind::MinMaxAvg(mma) => {
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
                            .limit(statistic.sample_size)
                            .load::<QueryMinMaxAvg>(conn)
                            .map_err(|_| http_error!(PERF_ERROR))?
                            .into_iter()
                            .map(|query| query.to_json())
                            .collect(),
                        MinMaxAvgKind::Memory => {
                            query
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
                                .limit(statistic.sample_size)
                                .load::<QueryMinMaxAvg>(conn)
                                .map_err(|_| http_error!(PERF_ERROR))?
                                .into_iter()
                                .map(|query| query.to_json())
                                .collect()
                        },
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
                            .limit(statistic.sample_size)
                            .load::<QueryMinMaxAvg>(conn)
                            .map_err(|_| http_error!(PERF_ERROR))?
                            .into_iter()
                            .map(|query| query.to_json())
                            .collect(),
                    };

                Ok(SampleMeanKind::MinMaxAvg(JsonMinMaxAvg::mean(json_data)))
            },
        }
    }
}
