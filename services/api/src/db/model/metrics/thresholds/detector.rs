use std::{
    collections::{
        HashMap,
        VecDeque,
    },
    f64::consts::PI,
};

use bencher_json::report::JsonMetricsMap;
use diesel::SqliteConnection;
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

use super::threshold::Threshold;
use crate::{
    db::model::{
        metrics::data::MetricsData,
        threshold::{
            alert::Side,
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
            self.z_score(&metrics_data.data, datum);
        }

        Ok(())
    }

    fn z_score(&self, data: &VecDeque<f64>, datum: f64) -> Option<f64> {
        if let Some(mean) = mean(&data) {
            if let Some(std_dev) = std_deviation(mean, &data) {
                if let Some(z) = z_score(mean, std_dev, datum) {
                    let (side, side_percentile) = match z < 0.0 {
                        true => {
                            if let Some(left_side) = self.threshold.statistic.left_side {
                                (Side::Left, left_side)
                            } else {
                                return None;
                            }
                        },
                        false => {
                            if let Some(right_side) = self.threshold.statistic.right_side {
                                (Side::Right, right_side)
                            } else {
                                return None;
                            }
                        },
                    };

                    if let Ok(normal) = Normal::new(mean, std_dev) {
                        let percentile = normal.cdf(z.abs());
                        if percentile > side_percentile as f64 {
                            // Generate alert
                        }
                    }
                }
            }
        }

        None
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

fn percentile(z: f64) -> f64 {
    // quad(normalProbabilityDensity, np.NINF, 1.25)
    // https://doc.rust-lang.org/std/primitive.f64.html#associatedconstant.NEG_INFINITY
    // to z
    standard_normal_probability_density(z)
}

// Probability Density Function for a Standard Normal Distribution
// mean (μ) of 0 and a standard deviation (σ) of 1
fn standard_normal_probability_density(x: f64) -> f64 {
    normal_probability_density(0.0, 1.0, x)
}

// Probability Density Function for a Normal Distribution
fn normal_probability_density(mean: f64, std_dev: f64, x: f64) -> f64 {
    let constant = 1.0 / (2.0 * PI * std_dev.powi(2)).sqrt();
    constant * (-(x - mean).powi(2) / (2.0 * std_dev.powi(2))).exp()
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
