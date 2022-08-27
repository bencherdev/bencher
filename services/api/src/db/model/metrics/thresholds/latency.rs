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
        metrics::sample_mean::{
            MeanKind,
            SampleMean,
        },
        threshold::{
            statistic::StatisticKind,
            PerfKind,
        },
    },
    util::http_error,
};

const PERF_ERROR: &str = "Failed to create perf statistic.";

pub struct Latency {
    pub threshold:    Threshold,
    pub sample_means: HashMap<String, JsonLatency>,
}

impl Latency {
    pub fn new(
        conn: &SqliteConnection,
        branch_id: i32,
        testbed_id: i32,
        benchmarks: &[(String, i32)],
        metrics_map: &JsonMetricsMap,
    ) -> Result<Option<Self>, HttpError> {
        let threshold = if let Some(threshold) =
            Threshold::new(conn, branch_id, testbed_id, PerfKind::Latency)
        {
            threshold
        } else {
            return Ok(None);
        };

        let mut sample_means = HashMap::with_capacity(benchmarks.len());
        for (benchmark_name, benchmark_id) in benchmarks {
            if let SampleMean::Latency(json) = SampleMean::new(
                conn,
                branch_id,
                testbed_id,
                *benchmark_id,
                &threshold.statistic,
                MeanKind::Latency,
            )? {
                if let Some(json) = json {
                    sample_means.insert(benchmark_name.clone(), json);
                }
            } else {
                return Err(http_error!(PERF_ERROR));
            }
        }

        // TODO check threshold kind
        // if it is a t-test go ahead and perform it and create alerts and then return
        // None since the threshold won't be needed for every perf
        Ok(if let StatisticKind::T = threshold.statistic.test {
            None
        } else {
            Some(Self {
                threshold,
                sample_means,
            })
        })
    }

    pub fn z_score(
        &self,
        conn: &SqliteConnection,
        benchmark_name: &str,
        json_latency: JsonLatency,
    ) {
        if let Some(sample_mean) = self.sample_means.get(benchmark_name) {
            // TODO use the sample mean to compare against the self.threshold
            // and the json_latency
        }
    }
}
