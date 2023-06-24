use bencher_json::JsonMetric;

use crate::{
    model::project::threshold::{alert::Side, statistic::StatisticKind},
    ApiError,
};

use super::{data::MetricsData, limits::Limits};

#[derive(Default)]
pub struct Boundary {
    pub limits: Limits,
    pub outlier: Option<Side>,
}

impl Boundary {
    pub fn new(
        metric: JsonMetric,
        metrics_data: MetricsData,
        test: StatisticKind,
        min_sample_size: Option<i64>,
        left_side: Option<f64>,
        right_side: Option<f64>,
    ) -> Result<Self, ApiError> {
        let data = &metrics_data.data;
        let data_len = data.len();

        // If there is a min sample size, then check to see if it is met.
        // Otherwise, simply return.
        if let Some(min_sample_size) = min_sample_size {
            if data_len < min_sample_size as usize {
                return Ok(Self::default());
            }
        }

        // Get the mean, standard deviation, and degrees of freedom of the historical data.
        let Some(mean) = mean(data) else {
            return Ok(Self::default())
        };
        let Some(std_dev) = std_deviation(mean, data) else {
            return Ok(Self::default());
        };
        let freedom = (data_len - 1) as f64;

        let limits = Limits::new(mean, std_dev, freedom, test, left_side, right_side)?;

        let datum = metric.value.into();
        let outlier = limits.outlier(datum);

        Ok(Self { limits, outlier })
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

#[cfg(test)]
mod test {
    use pretty_assertions::assert_eq;
    use statrs::distribution::{ContinuousCDF, Normal, StudentsT};

    #[test]
    fn test_outlier() {
        let normal = Normal::new(1.0, 1.0).unwrap();
        let percentile = normal.cdf(2.0);
        assert_eq!(percentile, 0.8413447460549428);

        let inverse = normal.inverse_cdf(0.6);
        assert_eq!(inverse, 1.2533471031357997);

        let inverse = normal.inverse_cdf(0.8413447460549428);
        assert_eq!(inverse, 1.999999999943794);

        let students_t = StudentsT::new(1.0, 1.0, 10.0).unwrap();
        let percentile = students_t.cdf(2.0);
        assert_eq!(percentile, 0.8295534338489701);

        let inverse = students_t.inverse_cdf(0.8295534338489701);
        assert_eq!(inverse, 2.000000000000001);
    }
}
