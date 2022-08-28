use std::collections::HashMap;

use bencher_json::report::JsonMetricsMap;
use diesel::SqliteConnection;
use dropshot::HttpError;

use super::threshold::Threshold;
use crate::{
    db::model::{
        metrics::data::MetricsData,
        threshold::{
            statistic::StatisticKind,
            PerfKind,
        },
    },
    util::http_error,
};

const PERF_ERROR: &str = "Failed to create perf statistic.";

pub struct Detector {
    pub report_id: i32,
    pub threshold: Threshold,
    pub data:      HashMap<String, MetricsData>,
}

impl Detector {
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
        let mut data = HashMap::with_capacity(benchmarks.len());
        for (benchmark_name, benchmark_id) in benchmarks {
            if let Some(json) = MetricsData::new(
                conn,
                branch_id,
                testbed_id,
                *benchmark_id,
                &threshold.statistic,
                kind,
            )? {
                data.insert(benchmark_name.clone(), json);
            } else {
                return Err(http_error!(PERF_ERROR));
            }
        }

        // If the threshold statistic is a t-test go ahead and perform it and create
        // alerts. Since this only needs to happen once, return None for the
        // latency threshold.
        Ok(if let StatisticKind::T = threshold.statistic.test {
            Self::t_test(conn, report_id, &threshold, metrics_map, &data)?;
            None
        } else {
            Some(Self {
                report_id,
                threshold,
                data,
            })
        })
    }

    pub fn t_test(
        conn: &SqliteConnection,
        report_id: i32,
        threshold: &Threshold,
        metrics_map: &JsonMetricsMap,
        data: &HashMap<String, MetricsData>,
    ) -> Result<(), HttpError> {
        for (benchmark_name, metrics_list) in &metrics_map.inner {
            if let Some(std_dev) = data.get(benchmark_name) {
                // TODO perform a t test with the sample mean and threshold
                let latency_data = &metrics_list.latency;
            }
        }

        Ok(())
    }

    pub fn z_score(
        &mut self,
        conn: &SqliteConnection,
        perf_id: i32,
        benchmark_name: &str,
        datum: f64,
    ) -> Result<(), HttpError> {
        if let Some(metrics_data) = self.data.get_mut(benchmark_name) {
            let mut data = metrics_data.data;
            // Add the new metrics datum
            data.push_left(datum);
            // If there was a set sample size, then pop off the oldest datum
            if self.threshold.statistic.sample_size.is_some() {
                data.pop_right();
            }
            if let Some(mean) = MetricsData::mean(&data) {
                let std_dev = MetricsData::std_deviation(mean, &data);
                let z = (datum - mean) / std_dev;
            }
        }

        Ok(())
    }
}
