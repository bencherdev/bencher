use bencher_json::JsonMetric;
use statrs::distribution::{ContinuousCDF, Normal, StudentsT};

use crate::{
    error::api_error,
    model::project::threshold::{alert::Side, statistic::StatisticKind},
    ApiError,
};

use super::data::MetricsData;

pub struct Outlier {
    pub side: Side,
    pub boundary: f32,
    pub percentile: f64,
}

impl Outlier {
    #[allow(
        clippy::arithmetic_side_effects,
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        clippy::float_arithmetic,
        clippy::integer_arithmetic
    )]
    pub fn detect(
        metric: JsonMetric,
        metrics_data: MetricsData,
        test: StatisticKind,
        min_sample_size: Option<i64>,
        left_side: Option<f32>,
        right_side: Option<f32>,
    ) -> Result<Option<Self>, ApiError> {
        // If there is a min sample size, then check to see if it is met.
        // Otherwise, simply return.
        if let Some(min_sample_size) = min_sample_size {
            if metrics_data.data.len() < min_sample_size as usize {
                return Ok(None);
            }
        }

        // Get the mean and standard deviation of the historical data
        let data = &metrics_data.data;
        let Some(mean) = mean(data) else {
            return Ok(None)
        };
        let Some(std_dev) = std_deviation(mean, data) else {
            return Ok(None);
        };

        // If the datum is less than the mean, check for a left side boundary.
        // If the datum is greater than or equal to the mean, check for a right side boundary.
        // Otherwise, simply return.
        let datum = metric.value.into();
        let (abs_datum, side, boundary) = if datum < mean {
            if let Some(left_side) = left_side {
                // Flip the datum to the other side of the mean, creating an absolute datum
                let abs_datum = mean * 2.0 - datum;
                (abs_datum, Side::Left, left_side)
            } else {
                return Ok(None);
            }
        } else if let Some(right_side) = right_side {
            (datum, Side::Right, right_side)
        } else {
            return Ok(None);
        };

        let percentile = match test {
            // Create a normal distribution and calculate the percentile for the absolute datum within that distribution
            StatisticKind::Z => {
                let normal = Normal::new(mean, std_dev).map_err(api_error!())?;
                normal.cdf(abs_datum)
            },
            // Create a Student's t distribution and calculate the percentile for the absolute datum within that distribution
            StatisticKind::T => {
                let students_t =
                    StudentsT::new(mean, std_dev, (data.len() - 1) as f64).map_err(api_error!())?;
                students_t.cdf(abs_datum)
            },
        };

        // If the percentile is greater than the boundary, then return the outlier
        Ok((percentile > boundary.into()).then_some(Self {
            side,
            boundary,
            percentile,
        }))
    }
}

fn std_deviation(mean: f64, data: &[f64]) -> Option<f64> {
    variance(mean, data).map(f64::sqrt)
}

#[allow(clippy::cast_precision_loss, clippy::float_arithmetic)]
fn variance(mean: f64, data: &[f64]) -> Option<f64> {
    // Do not calculate variance if there are less than 2 data points
    if data.len() < 2 {
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
