use bencher_json::JsonMetric;
use diesel::RunQueryDsl;
use statrs::distribution::{ContinuousCDF, Normal, StudentsT};
use uuid::Uuid;

use crate::{
    context::DbConnection,
    error::api_error,
    model::project::threshold::{
        alert::{InsertAlert, Side},
        statistic::StatisticKind,
    },
    schema, ApiError,
};

pub mod data;
pub mod threshold;

use data::MetricsData;
use threshold::MetricsThreshold;

#[derive(Debug, Clone)]
pub struct Detector {
    pub branch_id: i32,
    pub testbed_id: i32,
    pub metric_kind_id: i32,
    pub threshold: MetricsThreshold,
}

impl Detector {
    pub fn new(
        conn: &mut DbConnection,
        branch_id: i32,
        testbed_id: i32,
        metric_kind_id: i32,
    ) -> Result<Option<Self>, ApiError> {
        // Check to see if there is a threshold for the branch/testbed/metric kind grouping.
        // If not, then there will be nothing to detect.
        let threshold = if let Some(threshold) =
            MetricsThreshold::new(conn, branch_id, testbed_id, metric_kind_id)
        {
            threshold
        } else {
            return Ok(None);
        };

        Ok(Some(Self {
            branch_id,
            testbed_id,
            metric_kind_id,
            threshold,
        }))
    }

    #[allow(
        clippy::arithmetic_side_effects,
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        clippy::float_arithmetic,
        clippy::integer_arithmetic
    )]
    pub fn detect(
        &self,
        conn: &mut DbConnection,
        perf_id: i32,
        benchmark_id: i32,
        metric: JsonMetric,
    ) -> Result<(), ApiError> {
        // Query the historical population/sample data for the benchmark
        let metrics_data = MetricsData::new(
            conn,
            self.branch_id,
            self.testbed_id,
            self.metric_kind_id,
            benchmark_id,
            &self.threshold.statistic,
        )?;

        // If there is a min sample size, then check to see if it is met.
        // Otherwise, simply return.
        if let Some(min_sample_size) = self.threshold.statistic.min_sample_size {
            if metrics_data.data.len() < min_sample_size as usize {
                return Ok(());
            }
        }

        let data = &metrics_data.data;
        let datum = metric.value.into();
        if let Some(mean) = mean(data) {
            if let Some(std_dev) = std_deviation(mean, data) {
                let (abs_datum, side, boundary) = if datum < mean {
                    if let Some(left_side) = self.threshold.statistic.left_side {
                        (mean * 2.0 - datum, Side::Left, left_side)
                    } else {
                        return Ok(());
                    }
                } else if let Some(right_side) = self.threshold.statistic.right_side {
                    (datum, Side::Right, right_side)
                } else {
                    return Ok(());
                };

                let percentile = match self.threshold.statistic.test.try_into()? {
                    StatisticKind::Z => {
                        let normal = Normal::new(mean, std_dev).map_err(api_error!())?;
                        normal.cdf(abs_datum)
                    },
                    StatisticKind::T => {
                        let students_t = StudentsT::new(mean, std_dev, (data.len() - 1) as f64)
                            .map_err(api_error!())?;
                        students_t.cdf(abs_datum)
                    },
                };

                if percentile > f64::from(boundary) {
                    self.alert(conn, perf_id, side, boundary, percentile)?;
                }
            }
        }

        Ok(())
    }

    #[allow(clippy::cast_possible_truncation)]
    fn alert(
        &self,
        conn: &mut DbConnection,
        perf_id: i32,
        side: Side,
        boundary: f32,
        outlier: f64,
    ) -> Result<(), ApiError> {
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
            .map_err(api_error!())?;

        Ok(())
    }
}

fn std_deviation(mean: f64, data: &[f64]) -> Option<f64> {
    variance(mean, data).map(f64::sqrt)
}

#[allow(clippy::cast_precision_loss, clippy::float_arithmetic)]
fn variance(mean: f64, data: &[f64]) -> Option<f64> {
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

#[allow(clippy::cast_precision_loss, clippy::float_arithmetic)]
fn mean(data: &[f64]) -> Option<f64> {
    if data.is_empty() {
        return None;
    }
    Some(data.iter().sum::<f64>() / data.len() as f64)
}
