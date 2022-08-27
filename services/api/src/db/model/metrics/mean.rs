use bencher_json::report::{
    data::{
        JsonReportAlert,
        JsonReportAlerts,
    },
    new::{
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
use uuid::Uuid;

use super::alerts::Alerts;
use crate::{
    db::{
        model::{
            benchmark::{
                InsertBenchmark,
                QueryBenchmark,
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

#[derive(Debug, Default)]
struct Mean {
    pub latency:    Option<i32>,
    pub throughput: Option<i32>,
    pub compute:    Option<i32>,
    pub memory:     Option<i32>,
    pub storage:    Option<i32>,
}

impl Mean {
    pub fn new(
        conn: &SqliteConnection,
        branch_id: i32,
        testbed_id: i32,
        benchmark_id: i32,
        sample_size: i64,
        window:      i64,
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

        let mean = Self::default();

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
            .limit(sample_size)
            .load::<QueryLatency>(conn)
            .map_err(|_| http_error!(PERF_ERROR))?
            .into_iter()
            .filter_map(|query| query.to_json().ok())
            .collect();

        // if json_latency_data.is_empty() {
        //     return Ok();
        // }
        // // TODO calculate the standard deviation and apply the proper test
        // // generate alerts for the json_latency given as applicable
        // let length = json_latency_data.len();
        // let json_latency_sum: JsonLatency = json_latency_data.into_iter().sum();
        // let mean = json_latency_sum / length;

        Ok(mean)
    }

    fn mean(
        conn: &SqliteConnection,
        branch_id: i32,
        testbed_id: i32,
        benchmark_id: i32,
        sample_size: i64,
        window:      i64,
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

        let mean = Self::default();

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
            .limit(sample_size)
            .load::<QueryLatency>(conn)
            .map_err(|_| http_error!(PERF_ERROR))?
            .into_iter()
            .filter_map(|query| query.to_json().ok())
            .collect();

        // if json_latency_data.is_empty() {
        //     return Ok();
        // }
        // // TODO calculate the standard deviation and apply the proper test
        // // generate alerts for the json_latency given as applicable
        // let length = json_latency_data.len();
        // let json_latency_sum: JsonLatency = json_latency_data.into_iter().sum();
        // let mean = json_latency_sum / length;

        Ok(mean)
    }
}
