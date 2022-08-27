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

pub mod alerts;
pub mod mean;
pub mod thresholds;

use self::{
    alerts::Alerts,
    thresholds::{
        Threshold,
        Thresholds,
    },
};

const PERF_ERROR: &str = "Failed to create perf statistic.";

pub struct Metrics {
    pub project_id:  i32,
    pub report_id:   i32,
    pub branch_id:   i32,
    pub testbed_id:  i32,
    pub metrics_map: JsonMetricsMap,
    pub thresholds:  Thresholds,
}

impl Metrics {
    pub fn new(
        conn: &SqliteConnection,
        project_id: i32,
        report_id: i32,
        branch_id: i32,
        testbed_id: i32,
        benchmarks: JsonBenchmarks,
    ) -> Self {
        Self {
            project_id,
            report_id,
            branch_id,
            testbed_id,
            metrics_map: JsonMetricsMap::from(benchmarks),
            thresholds: Thresholds::new(conn, branch_id, testbed_id),
        }
    }

    pub fn benchmark(
        &self,
        conn: &SqliteConnection,
        iteration: i32,
        benchmark_name: String,
        json_metrics: JsonMetrics,
    ) -> Result<(), HttpError> {
        let mut perf_alerts = None;

        let benchmark_id = if let Ok(benchmark_id) =
            QueryBenchmark::get_id_from_name(conn, self.project_id, &benchmark_name)
        {
            // Only generate alerts if the benchmark already exists
            // and a threshold is provided.
            // Note these alerts have not yet been committed to the database.
            perf_alerts = Some(self.alerts(conn, &benchmark_name, benchmark_id, &json_metrics)?);
            benchmark_id
        } else {
            let insert_benchmark = InsertBenchmark::new(self.project_id, benchmark_name);
            diesel::insert_into(schema::benchmark::table)
                .values(&insert_benchmark)
                .execute(conn)
                .map_err(|_| http_error!("Failed to create benchmark."))?;

            schema::benchmark::table
                .filter(schema::benchmark::uuid.eq(&insert_benchmark.uuid))
                .select(schema::benchmark::id)
                .first::<i32>(conn)
                .map_err(|_| http_error!("Failed to create benchmark."))?
        };

        let insert_perf =
            InsertPerf::from_json(conn, self.report_id, iteration, benchmark_id, json_metrics)?;

        diesel::insert_into(schema::perf::table)
            .values(&insert_perf)
            .execute(conn)
            .map_err(|_| http_error!("Failed to create benchmark data."))?;

        let perf_id = QueryPerf::get_id(conn, &insert_perf.uuid)?;

        // TODO move this over to an internal state/operation of metrics_threshold
        // That is it should manage all of this behind the scenes.
        // For t-tests, it won't include the perf_id but for z scores it will
        // Break this out into its own `metrics` module
        // Commit the alerts to the database now that the perf exists
        // let report_alerts = perf_alerts.map(|alerts| {
        //     alerts
        //         .into_iter()
        //         .filter_map(|perf_alert| {
        //             perf_alert
        //                 .to_json(conn, self.report_id, Some(perf_id))
        //                 .ok()
        //         })
        //         .collect()
        // });

        Ok(())
    }

    pub fn alerts(
        &self,
        conn: &SqliteConnection,
        benchmark_name: &str,
        benchmark_id: i32,
        metrics: &JsonMetrics,
    ) -> Result<Alerts, HttpError> {
        let mut alerts = Alerts::new();

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
