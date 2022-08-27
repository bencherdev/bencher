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
pub mod sample_mean;
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
    pub project_id: i32,
    pub report_id:  i32,
    pub branch_id:  i32,
    pub testbed_id: i32,
    pub thresholds: Thresholds,
}

impl Metrics {
    pub fn new(
        conn: &SqliteConnection,
        project_id: i32,
        branch_id: i32,
        testbed_id: i32,
        report_id: i32,
        benchmarks: JsonBenchmarks,
    ) -> Result<Self, HttpError> {
        Ok(Self {
            project_id,
            branch_id,
            testbed_id,
            report_id,
            thresholds: Thresholds::new(conn, project_id, branch_id, testbed_id, benchmarks)?,
        })
    }

    pub fn benchmark(
        &self,
        conn: &SqliteConnection,
        iteration: i32,
        benchmark_name: String,
        json_metrics: JsonMetrics,
    ) -> Result<(), HttpError> {
        // All benchmarks should already exist
        let benchmark_id =
            QueryBenchmark::get_id_from_name(conn, self.project_id, &benchmark_name)?;

        let insert_perf =
            InsertPerf::from_json(conn, self.report_id, iteration, benchmark_id, json_metrics)?;

        diesel::insert_into(schema::perf::table)
            .values(&insert_perf)
            .execute(conn)
            .map_err(|_| http_error!("Failed to create benchmark data."))?;

        let perf_id = QueryPerf::get_id(conn, &insert_perf.uuid)?;

        // TODO move this into a call to thresholds
        // thresholds.z_score
        if let Some(latency) = &self.thresholds.latency {
            if let Some(sample_mean) = latency.sample_means.get(&benchmark_name) {
                // Then do z score test with sample mean and latency.threshold
            }
        }

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
