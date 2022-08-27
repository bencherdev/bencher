use std::collections::HashMap;

use bencher_json::report::{
    JsonLatency,
    JsonMetricsMap,
};
use diesel::SqliteConnection;
use dropshot::HttpError;

use super::threshold::Threshold;
use crate::{
    db::model::{
        metrics::std_deviation::StdDev,
        threshold::{
            statistic::StatisticKind,
            PerfKind,
        },
    },
    util::http_error,
};

const PERF_ERROR: &str = "Failed to create perf statistic.";

pub struct Latency {
    pub report_id:  i32,
    pub threshold:  Threshold,
    pub deviations: HashMap<String, StdDev>,
}

impl Latency {
    pub fn new(
        conn: &SqliteConnection,
        branch_id: i32,
        testbed_id: i32,
        report_id: i32,
        benchmarks: &[(String, i32)],
        metrics_map: &JsonMetricsMap,
        kind: PerfKind,
    ) -> Result<Option<Self>, HttpError> {
        // Check to see if there is a latency threshold for this branch/testbed pair
        let threshold = if let Some(threshold) = Threshold::new(conn, branch_id, testbed_id, kind) {
            threshold
        } else {
            return Ok(None);
        };

        // Calculate the sample means
        let mut deviations = HashMap::with_capacity(benchmarks.len());
        for (benchmark_name, benchmark_id) in benchmarks {
            if let Some(json) = StdDev::new(
                conn,
                branch_id,
                testbed_id,
                *benchmark_id,
                &threshold.statistic,
                kind,
            )? {
                deviations.insert(benchmark_name.clone(), json);
            } else {
                return Err(http_error!(PERF_ERROR));
            }
        }

        // If the threshold statistic is a t-test go ahead and perform it and create
        // alerts. Since this only needs to happen once, return None for the
        // latency threshold.
        Ok(if let StatisticKind::T = threshold.statistic.test {
            Self::t_test(conn, report_id, &threshold, metrics_map, &deviations)?;
            None
        } else {
            Some(Self {
                report_id,
                threshold,
                deviations,
            })
        })
    }

    pub fn z_score(
        &self,
        conn: &SqliteConnection,
        perf_id: i32,
        benchmark_name: &str,
        json_latency: JsonLatency,
    ) -> Result<(), HttpError> {
        if let Some(std_dev) = self.deviations.get(benchmark_name) {
            let mut data = std_dev.data.clone();
            let datum = json_latency.duration as f64;
            data.push(datum);
            if let Some(mean) = StdDev::mean(&data) {
                let std_deviation = StdDev::std_deviation(mean, &data);
                let z = (datum - mean) / std_deviation;
            }
        }

        Ok(())
    }

    pub fn t_test(
        conn: &SqliteConnection,
        report_id: i32,
        threshold: &Threshold,
        metrics_map: &JsonMetricsMap,
        deviations: &HashMap<String, StdDev>,
    ) -> Result<(), HttpError> {
        for (benchmark_name, metrics_list) in &metrics_map.inner {
            if let Some(std_dev) = deviations.get(benchmark_name) {
                // TODO perform a t test with the sample mean and threshold
                let latency_data = &metrics_list.latency;
            }
        }

        Ok(())
    }
}
