use std::collections::{
    HashMap,
    VecDeque,
};

use bencher_json::report::JsonMetricsMap;
use diesel::{
    RunQueryDsl,
    SqliteConnection,
};
use dropshot::HttpError;
use statrs::{
    distribution::{
        Continuous,
        ContinuousCDF,
        Normal,
        StudentsT,
    },
    statistics::Distribution,
};
use uuid::Uuid;

use super::threshold::Threshold;
use crate::{
    db::{
        model::{
            metrics::data::MetricsData,
            threshold::{
                alert::{
                    InsertAlert,
                    Side,
                },
                statistic::StatisticKind,
                PerfKind,
            },
        },
        schema,
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

        // Query and cache the historical population/sample data for each benchmark
        let mut data = HashMap::with_capacity(benchmarks.len());
        for (benchmark_name, benchmark_id) in benchmarks {
            if let Some(metrics_data) = MetricsData::new(
                conn,
                branch_id,
                testbed_id,
                *benchmark_id,
                &threshold.statistic,
                kind,
            )? {
                data.insert(benchmark_name.clone(), metrics_data);
            } else {
                return Err(http_error!(PERF_ERROR));
            }
        }

        // If the threshold statistic is a t-test go ahead and perform it and create
        // alerts. Since this only needs to happen once, return None for the
        // latency threshold. Otherwise, return a Detector that will be used for the
        // other, per datum tests (i.e. z-score).
        Ok(match threshold.statistic.test.try_into()? {
            StatisticKind::Z => Some(Self {
                report_id,
                threshold,
                data,
            }),
            StatisticKind::T => {
                Self::t_test(conn, report_id, &threshold, metrics_map, &data)?;
                None
            },
        })
    }

    pub fn z_test(
        &mut self,
        conn: &SqliteConnection,
        perf_id: i32,
        benchmark_name: &str,
        datum: f64,
    ) -> Result<(), HttpError> {
        // Update cached metrics data
        if let Some(metrics_data) = self.data.get_mut(benchmark_name) {
            let data = &mut metrics_data.data;
            // Add the new metrics datum
            data.push_front(datum);
            // If there is a set sample size, then check to see if adding the new datum
            // caused us to exceed it. If so, then pop off the oldest datum.
            if let Some(sample_size) = self.threshold.statistic.sample_size {
                if data.len() > sample_size as usize {
                    data.pop_back();
                    debug_assert!(data.len() == sample_size as usize)
                }
            }
        }

        if let Some(metrics_data) = self.data.get(benchmark_name) {
            self.z_score(conn, perf_id, &metrics_data.data, datum)?;
        }

        Ok(())
    }

    fn z_score(
        &self,
        conn: &SqliteConnection,
        perf_id: i32,
        data: &VecDeque<f64>,
        datum: f64,
    ) -> Result<(), HttpError> {
        if let Some(mean) = mean(&data) {
            if let Some(std_dev) = std_deviation(mean, &data) {
                if let Some(z) = z_score(mean, std_dev, datum) {
                    let (side, boundary) = match z < 0.0 {
                        true => {
                            if let Some(left_side) = self.threshold.statistic.left_side {
                                (Side::Left, left_side)
                            } else {
                                return Ok(());
                            }
                        },
                        false => {
                            if let Some(right_side) = self.threshold.statistic.right_side {
                                (Side::Right, right_side)
                            } else {
                                return Ok(());
                            }
                        },
                    };

                    if let Ok(normal) = Normal::new(mean, std_dev) {
                        let percentile = normal.cdf(z.abs());
                        if percentile > boundary as f64 {
                            self.alert(conn, Some(perf_id), side, boundary, percentile)?;
                        }
                    }
                }
            }
        }

        Ok(())
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

    fn alert(
        &self,
        conn: &SqliteConnection,
        perf_id: Option<i32>,
        side: Side,
        boundary: f32,
        outlier: f64,
    ) -> Result<(), HttpError> {
        let insert_alert = InsertAlert {
            uuid: Uuid::new_v4().to_string(),
            report_id: self.report_id,
            perf_id,
            threshold_id: self.threshold.id,
            statistic_id: self.threshold.statistic.id,
            side: side.into(),
            boundary,
            outlier: outlier as f32,
        };

        diesel::insert_into(schema::alert::table)
            .values(&insert_alert)
            .execute(conn)
            .map_err(|_| http_error!(PERF_ERROR))?;

        Ok(())
    }
}

fn z_score(mean: f64, std_dev: f64, datum: f64) -> Option<f64> {
    if std_dev.is_normal() {
        Some((datum - mean) / std_dev)
    } else {
        None
    }
}

fn std_deviation(mean: f64, data: &VecDeque<f64>) -> Option<f64> {
    variance(mean, data).map(|variance| variance.sqrt())
}

fn variance(mean: f64, data: &VecDeque<f64>) -> Option<f64> {
    if data.is_empty() {
        return None;
    }
    Some(
        data.iter()
            .map(|value| (*value - mean).powi(2))
            .sum::<f64>()
            / data.len() as f64,
    )
}

fn mean(data: &VecDeque<f64>) -> Option<f64> {
    if data.is_empty() {
        return None;
    }
    Some(data.iter().sum::<f64>() / data.len() as f64)
}

#[cfg(test)]
mod test {
    #[test]
    fn test_stats() {
        use statrs::{
            distribution::{
                Continuous,
                ContinuousCDF,
                Normal,
            },
            statistics::Distribution,
        };

        let n = Normal::new(0.0, 1.0).unwrap();
        assert_eq!(n.mean().unwrap(), 0.0);
        assert_eq!(n.pdf(1.0), 0.2419707245191433497978);
        assert_eq!(n.cdf(1.0), 0.8413447460549428);
        assert_eq!(n.cdf(2.0), 0.9772498680528374);
    }
}
