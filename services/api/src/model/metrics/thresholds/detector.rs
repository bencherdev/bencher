use std::collections::{HashMap, VecDeque};

use bencher_json::report::JsonMetricsMap;
use diesel::{RunQueryDsl, SqliteConnection};
use dropshot::HttpError;
use statrs::distribution::{ContinuousCDF, Normal, StudentsT};
use uuid::Uuid;

use super::threshold::Threshold;
use crate::{
    model::{
        metrics::data::MetricsData,
        threshold::{
            alert::{InsertAlert, Side},
            statistic::StatisticKind,
            PerfKind,
        },
    },
    schema,
    util::http_error,
};


pub struct Detector {
    pub threshold: Threshold,
    pub data: HashMap<String, MetricsData>,
}

impl Detector {
    pub fn new(
        conn: &mut SqliteConnection,
        branch_id: i32,
        testbed_id: i32,
        benchmarks: &[(String, i32)],
        _metrics_map: &JsonMetricsMap,
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
            let metrics_data = MetricsData::new(
                conn,
                branch_id,
                testbed_id,
                *benchmark_id,
                &threshold.statistic,
                kind,
            )?;
            data.insert(benchmark_name.clone(), metrics_data);
        }

        // TODO use metrics_map in a two sample t-test

        Ok(Some(Self { threshold, data }))
    }

    pub fn test(
        &mut self,
        conn: &mut SqliteConnection,
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
            let data = &metrics_data.data;
            if let Some(mean) = mean(data) {
                if let Some(std_dev) = std_deviation(mean, data) {
                    let (abs_datum, side, boundary) = match datum < mean {
                        true => {
                            if let Some(left_side) = self.threshold.statistic.left_side {
                                (mean * 2.0 - datum, Side::Left, left_side)
                            } else {
                                return Ok(());
                            }
                        },
                        false => {
                            if let Some(right_side) = self.threshold.statistic.right_side {
                                (datum, Side::Right, right_side)
                            } else {
                                return Ok(());
                            }
                        },
                    };

                    let percentile = match self.threshold.statistic.test.try_into()? {
                        StatisticKind::Z => {
                            let normal =
                                Normal::new(mean, std_dev).map_err(|_| http_error!("Failed to run metrics detector."))?;
                            normal.cdf(abs_datum)
                        },
                        StatisticKind::T => {
                            let students_t = StudentsT::new(mean, std_dev, (data.len() - 1) as f64)
                                .map_err(|_| http_error!("Failed to run metrics detector."))?;
                            students_t.cdf(abs_datum)
                        },
                    };

                    if percentile > boundary as f64 {
                        self.alert(conn, perf_id, side, boundary, percentile)?;
                    }
                }
            }
        }

        Ok(())
    }

    fn alert(
        &self,
        conn: &mut SqliteConnection,
        perf_id: i32,
        side: Side,
        boundary: f32,
        outlier: f64,
    ) -> Result<(), HttpError> {
        let insert_alert = InsertAlert {
            uuid: Uuid::new_v4().to_string(),
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
            .map_err(|_| http_error!("Failed to run metrics detector."))?;

        Ok(())
    }
}

#[allow(dead_code)]
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
    use statrs::{
        distribution::{Continuous, ContinuousCDF, Normal, StudentsT},
        statistics::Distribution,
    };

    #[test]
    fn test_normal() {
        let n = Normal::new(0.0, 1.0).unwrap();
        assert_eq!(n.mean().unwrap(), 0.0);
        assert_eq!(n.pdf(1.0), 0.2419707245191433497978);
        assert_eq!(n.cdf(0.0), 0.5);
        assert_eq!(n.cdf(1.0), 0.8413447460549428);
        assert_eq!(n.cdf(2.0), 0.9772498680528374);
    }

    #[test]
    fn test_students_t() {
        let students_t = StudentsT::new(0.0, 2.0, 10.0).unwrap();

        // assert_eq!(students_t.pdf(0.25), 0.37600028568971794);
        // assert_eq!(students_t.pdf(0.5), 0.33969513635207776);
        // assert_eq!(students_t.pdf(0.9), 0.2535299505598274);

        // assert_eq!(students_t.cdf(0.25), 0.5961758971316931);
        // assert_eq!(students_t.cdf(0.5), 0.6860531971285135);
        // assert_eq!(students_t.cdf(0.9), 0.8053603689969588);

        // Location 0
        // assert_eq!(students_t.mean().unwrap(), 0.0);

        // assert_eq!(students_t.cdf(0.0), 0.5);
        // assert_eq!(students_t.cdf(1.0), 0.8295534338489701);
        // assert_eq!(students_t.cdf(2.0), 0.9633059826146299);

        // assert_eq!(students_t.std_dev().unwrap(), 1.118033988749895);

        // Location 1
        // assert_eq!(students_t.mean().unwrap(), 1.0);

        // assert_eq!(students_t.cdf(1.0), 0.5);
        // assert_eq!(students_t.cdf(2.0), 0.8295534338489701);
        // assert_eq!(students_t.cdf(3.0), 0.9633059826146299);

        // assert_eq!(students_t.std_dev().unwrap(), 1.118033988749895);

        // Scale 2
        assert_eq!(students_t.mean().unwrap(), 0.0);

        assert_eq!(students_t.cdf(0.0), 0.5);
        assert_eq!(students_t.cdf(1.0), 0.6860531971285135);
        assert_eq!(students_t.cdf(2.0), 0.8295534338489701);

        assert_eq!(students_t.std_dev().unwrap(), 2.23606797749979);
    }
}
