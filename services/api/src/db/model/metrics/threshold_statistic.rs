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


use super::alerts::Alerts;

const PERF_ERROR: &str = "Failed to create perf statistic.";

pub struct ThresholdStatistic {
    pub threshold_id: i32,
    pub statistic:    Statistic,
}

pub struct Statistic {
    pub id:          i32,
    pub uuid:        String,
    pub test:        StatisticKind,
    pub sample_size: i64,
    pub window:      i64,
    pub left_side:   Option<f32>,
    pub right_side:  Option<f32>,
}

impl ThresholdStatistic {
    pub fn new(
        conn: &SqliteConnection,
        branch_id: i32,
        testbed_id: i32,
        kind: PerfKind,
    ) -> Result<Self, HttpError> {
        schema::statistic::table
            .inner_join(
                schema::threshold::table
                    .on(schema::statistic::id.eq(schema::threshold::statistic_id)),
            )
            .filter(
                schema::threshold::branch_id
                    .eq(branch_id)
                    .and(schema::threshold::testbed_id.eq(testbed_id))
                    .and(schema::threshold::kind.eq(kind as i32)),
            )
            .select((
                schema::threshold::id,
                schema::statistic::id,
                schema::statistic::uuid,
                schema::statistic::test,
                schema::statistic::sample_size,
                schema::statistic::window,
                schema::statistic::left_side,
                schema::statistic::right_side,
            ))
            .first::<(
                i32,
                i32,
                String,
                i32,
                Option<i64>,
                Option<i64>,
                Option<f32>,
                Option<f32>,
            )>(conn)
            .map(
                |(threshold_id, id, uuid, test, sample_size, window, left_side,
        right_side)| -> Result<Self, HttpError> {             let
        statistic = Statistic {                 id,
                        uuid,
                        test: test.try_into()?,
                        sample_size: unwrap_sample_size(sample_size),
                        window: unwrap_window(window),
                        left_side,
                        right_side,
                    };
                    Ok(Self {
                        threshold_id,
                        statistic,
                    })
                },
            )
            .map_err(|_| http_error!(PERF_ERROR))?
    }

    pub fn latency_alerts(
        &self,
        conn: &SqliteConnection,
        branch_id: i32,
        testbed_id: i32,
        benchmark_id: i32,
        json_latency: &JsonLatency,
    ) -> Result<Alerts, HttpError> {
        let alerts = Alerts::new();

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
            .filter(schema::report::start_time.ge(self.statistic.window))
            .left_join(
                schema::testbed::table.on(schema::report::testbed_id.eq(schema::testbed::id)),
            )
            .filter(schema::testbed::id.eq(testbed_id))
            .left_join(
                schema::version::table.on(schema::report::version_id.eq(schema::version::id)),
            )
            .left_join(schema::branch::table.on(schema::version::branch_id.eq(schema::branch::id)))
            .filter(schema::branch::id.eq(branch_id));

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
            .limit(self.statistic.sample_size)
            .load::<QueryLatency>(conn)
            .map_err(|_| http_error!(PERF_ERROR))?
            .into_iter()
            .filter_map(|query| query.to_json().ok())
            .collect();

        if json_latency_data.is_empty() {
            return Ok(alerts);
        }
        // TODO calculate the standard deviation and apply the proper test
        // generate alerts for the json_latency given as applicable
        let length = json_latency_data.len();
        let json_latency_sum: JsonLatency = json_latency_data.into_iter().sum();
        let mean = json_latency_sum / length;

        Ok(alerts)
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