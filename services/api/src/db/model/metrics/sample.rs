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
struct Sample {
    pub latency:    Option<JsonLatency>,
    pub throughput: Option<JsonThroughput>,
    pub compute:    Option<JsonMinMaxAvg>,
    pub memory:     Option<JsonMinMaxAvg>,
    pub storage:    Option<JsonMinMaxAvg>,
}

enum SampleMean {
    Latency(Option<JsonLatency>),
    Throughput(Option<JsonThroughput>),
    MinMaxAvg(Option<JsonMinMaxAvg>),
}

enum Kind {
    Latency,
    Throughput,
    MinMaxAvg,
}

impl Sample {
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
            )?,
            memory:     map_min_max_avg(
                conn,
                branch_id,
                testbed_id,
                benchmark_id,
                thresholds.memory.as_ref(),
            )?,
            storage:    map_min_max_avg(
                conn,
                branch_id,
                testbed_id,
                benchmark_id,
                thresholds.storage.as_ref(),
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
        if let SampleMean::Latency(json) = SampleMean::new(
            conn,
            branch_id,
            testbed_id,
            benchmark_id,
            threshold,
            Kind::Latency,
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
        if let SampleMean::Throughput(json) = SampleMean::new(
            conn,
            branch_id,
            testbed_id,
            benchmark_id,
            threshold,
            Kind::Throughput,
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
) -> Result<Option<JsonMinMaxAvg>, HttpError> {
    Ok(if let Some(threshold) = threshold {
        if let SampleMean::MinMaxAvg(json) = SampleMean::new(
            conn,
            branch_id,
            testbed_id,
            benchmark_id,
            threshold,
            Kind::MinMaxAvg,
        )? {
            json
        } else {
            return Err(http_error!(PERF_ERROR));
        }
    } else {
        None
    })
}

impl SampleMean {
    fn new(
        conn: &SqliteConnection,
        branch_id: i32,
        testbed_id: i32,
        benchmark_id: i32,
        threshold: &Threshold,
        kind: Kind,
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
            .filter(schema::report::start_time.ge(threshold.statistic.window))
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
            Kind::Latency => {
                let json_latency_data: Vec<JsonLatency> = query
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
                    .limit(threshold.statistic.sample_size)
                    .load::<QueryLatency>(conn)
                    .map_err(|_| http_error!(PERF_ERROR))?
                    .into_iter()
                    .filter_map(|query| query.to_json().ok())
                    .collect();

                Ok(SampleMean::Latency(JsonLatency::mean(json_latency_data)))
            },
            _ => todo!(),
        }
    }
}
