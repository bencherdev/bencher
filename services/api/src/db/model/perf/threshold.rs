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

use super::{
    latency::QueryLatency,
    min_max_avg::QueryMinMaxAvg,
    throughput::QueryThroughput,
};
use crate::{
    db::{
        model::threshold::{
            alert::InsertAlert,
            statistic::StatisticKind,
            PerfKind,
        },
        schema,
        schema::statistic as statistic_table,
    },
    diesel::ExpressionMethods,
    util::http_error,
};

const PERF_ERROR: &str = "Failed to create perf statistic.";

pub struct MetricsThresholds {
    pub report_id:   i32,
    pub branch_id:   i32,
    pub testbed_id:  i32,
    pub metrics_map: JsonMetricsMap,
    pub latency:     Option<ThresholdStatistic>,
    pub throughput:  Option<ThresholdStatistic>,
    pub compute:     Option<ThresholdStatistic>,
    pub memory:      Option<ThresholdStatistic>,
    pub storage:     Option<ThresholdStatistic>,
}

pub struct ThresholdStatistic {
    pub threshold_id: i32,
    pub statistic:    Statistic,
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
    ) -> Result<PerfAlerts, HttpError> {
        let alerts = PerfAlerts::new();

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

pub struct Statistic {
    pub id:          i32,
    pub uuid:        String,
    pub test:        StatisticKind,
    pub sample_size: i64,
    pub window:      i64,
    pub left_side:   Option<f32>,
    pub right_side:  Option<f32>,
}

struct Perf {
    pub id: i32,
    pub latency_id: Option<i32>,
    pub throughput_id: Option<i32>,
    pub compute_id: Option<i32>,
    pub memory_id: Option<i32>,
    pub storage_id: Option<i32>,
}

impl MetricsThresholds {
    pub fn new(
        conn: &SqliteConnection,
        report_id: i32,
        branch_id: i32,
        testbed_id: i32,
        benchmarks: JsonBenchmarks,
    ) -> Self {
        Self {
            report_id,
            branch_id,
            testbed_id,
            metrics_map: JsonMetricsMap::from(benchmarks),
            latency: ThresholdStatistic::new(conn, branch_id, testbed_id, PerfKind::Latency).ok(),
            throughput: ThresholdStatistic::new(conn, branch_id, testbed_id, PerfKind::Throughput)
                .ok(),
            compute: ThresholdStatistic::new(conn, branch_id, testbed_id, PerfKind::Compute).ok(),
            memory: ThresholdStatistic::new(conn, branch_id, testbed_id, PerfKind::Memory).ok(),
            storage: ThresholdStatistic::new(conn, branch_id, testbed_id, PerfKind::Storage).ok(),
        }
    }

    pub fn alerts(
        &self,
        conn: &SqliteConnection,
        benchmark_name: &str,
        benchmark_id: i32,
        metrics: &JsonMetrics,
    ) -> Result<PerfAlerts, HttpError> {
        let mut alerts = PerfAlerts::new();

        // TODO other perf kinds
        // if let Some(json_latency) = &json_perf.latency {
        //     if let Some(st) = &self.latency {
        //         alerts.append(&mut st.latency_alerts(
        //             conn,
        //             self.branch_id,
        //             self.testbed_id,
        //             benchmark_id,
        //             json_latency,
        //         )?)
        //     }
        // }

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

fn json_min_max_avg(conn: &SqliteConnection, id: i32) -> Option<JsonMinMaxAvg> {
    schema::min_max_avg::table
        .filter(schema::min_max_avg::id.eq(id))
        .first::<QueryMinMaxAvg>(conn)
        .map(|query| query.to_json())
        .ok()
}

#[derive(Default)]
struct PerfJson {
    pub latency:    Vec<JsonLatency>,
    pub throughput: Vec<JsonThroughput>,
    pub compute:    Vec<JsonMinMaxAvg>,
    pub memory:     Vec<JsonMinMaxAvg>,
    pub storage:    Vec<JsonMinMaxAvg>,
}

impl PerfJson {
    fn push(&mut self, conn: &SqliteConnection, perf: &Perf) {
        if let Some(id) = perf.latency_id {
            if let Ok(Ok(json)) = schema::latency::table
                .filter(schema::latency::id.eq(id))
                .first::<QueryLatency>(conn)
                .map(|query| query.to_json())
            {
                self.latency.push(json);
            }
        }
        if let Some(id) = perf.throughput_id {
            if let Ok(Ok(json)) = schema::throughput::table
                .filter(schema::throughput::id.eq(id))
                .first::<QueryThroughput>(conn)
                .map(|query| query.to_json())
            {
                self.throughput.push(json);
            }
        }
        if let Some(id) = perf.compute_id {
            if let Some(json) = json_min_max_avg(conn, id) {
                self.compute.push(json);
            }
        }
        if let Some(id) = perf.memory_id {
            if let Some(json) = json_min_max_avg(conn, id) {
                self.memory.push(json);
            }
        }
        if let Some(id) = perf.storage_id {
            if let Some(json) = json_min_max_avg(conn, id) {
                self.storage.push(json);
            }
        }
    }
}

pub type PerfAlerts = Vec<PerfAlert>;

pub struct PerfAlert {
    pub threshold_id: i32,
    pub statistic_id: i32,
    pub side:         bool,
    pub boundary:     f64,
    pub outlier:      f64,
}

impl PerfAlert {
    pub fn into_report_alert(
        self,
        conn: &SqliteConnection,
        report_id: i32,
        perf_id: Option<i32>,
    ) -> Result<JsonReportAlert, HttpError> {
        let Self {
            threshold_id,
            statistic_id,
            side,
            boundary,
            outlier,
        } = self;
        let uuid = Uuid::new_v4();
        let insert_alert = InsertAlert {
            uuid: uuid.to_string(),
            report_id,
            perf_id,
            threshold_id,
            statistic_id,
            side,
            boundary,
            outlier,
        };

        diesel::insert_into(schema::alert::table)
            .values(&insert_alert)
            .execute(conn)
            .map_err(|_| http_error!(PERF_ERROR))?;

        Ok(uuid.into())
    }
}
