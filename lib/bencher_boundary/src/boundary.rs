use bencher_json::project::boundary::BoundaryLimit;
use bencher_json::project::threshold::StatisticKind;
use bencher_json::{Boundary, SampleSize};
use slog::Logger;

use crate::limits::{MetricsLimits, StatisticalTestKind};
use crate::{BoundaryError, MetricsData};

#[derive(Debug, Default)]
pub struct MetricsBoundary {
    pub limits: MetricsLimits,
    pub outlier: Option<BoundaryLimit>,
}

impl MetricsBoundary {
    pub fn new(
        log: &Logger,
        datum: f64,
        metrics_data: &MetricsData,
        statistic_kind: StatisticKind,
        min_sample_size: Option<SampleSize>,
        lower_boundary: Option<Boundary>,
        upper_boundary: Option<Boundary>,
    ) -> Result<Self, BoundaryError> {
        Self::new_inner(
            log,
            datum,
            metrics_data,
            statistic_kind,
            min_sample_size,
            lower_boundary,
            upper_boundary,
        )
        .map(Option::unwrap_or_default)
    }

    fn new_inner(
        log: &Logger,
        datum: f64,
        metrics_data: &MetricsData,
        statistic_kind: StatisticKind,
        min_sample_size: Option<SampleSize>,
        lower_boundary: Option<Boundary>,
        upper_boundary: Option<Boundary>,
    ) -> Result<Option<Self>, BoundaryError> {
        // If there is no boundary, then simply return.
        if lower_boundary.is_none() && upper_boundary.is_none() {
            return Ok(None);
        }
        let data = &metrics_data.data;
        // If there is a min sample size, then check to see if it is met.
        // Otherwise, simply return.
        if let Some(min_sample_size) = min_sample_size {
            if data.len() < min_sample_size.into() {
                return Ok(None);
            }
        }

        match statistic_kind {
            StatisticKind::Static => Ok(Some(Self::new_static(
                datum,
                lower_boundary,
                upper_boundary,
            ))),
            StatisticKind::Percentage => {
                Self::new_percentage(log, datum, data, lower_boundary, upper_boundary)
            },
            StatisticKind::ZScore => Self::new_statistical(
                log,
                datum,
                data,
                StatisticalTestKind::Z,
                lower_boundary,
                upper_boundary,
            ),
            StatisticKind::TTest => Self::new_statistical(
                log,
                datum,
                data,
                #[allow(clippy::cast_precision_loss)]
                StatisticalTestKind::T {
                    freedom: (data.len() - 1) as f64,
                },
                lower_boundary,
                upper_boundary,
            ),
            StatisticKind::IQR | StatisticKind::LogNormal => Ok(None),
        }
    }

    fn new_static(
        datum: f64,
        lower_boundary: Option<Boundary>,
        upper_boundary: Option<Boundary>,
    ) -> Self {
        let limits = MetricsLimits::new_static(lower_boundary, upper_boundary);
        let outlier = limits.outlier(datum);

        Self { limits, outlier }
    }

    fn new_percentage(
        log: &Logger,
        datum: f64,
        data: &[f64],
        lower_boundary: Option<Boundary>,
        upper_boundary: Option<Boundary>,
    ) -> Result<Option<Self>, BoundaryError> {
        let lower_boundary = lower_boundary.map(TryInto::try_into).transpose()?;
        let upper_boundary = upper_boundary.map(TryInto::try_into).transpose()?;

        // Get the mean of the historical data.
        let Some(mean) = mean(data) else {
            return Ok(None);
        };

        let limits = MetricsLimits::new_percentage(log, mean, lower_boundary, upper_boundary);
        let outlier = limits.outlier(datum);

        Ok(Some(Self { limits, outlier }))
    }

    fn new_statistical(
        log: &Logger,
        datum: f64,
        data: &[f64],
        test_kind: StatisticalTestKind,
        lower_boundary: Option<Boundary>,
        upper_boundary: Option<Boundary>,
    ) -> Result<Option<Self>, BoundaryError> {
        let lower_boundary = lower_boundary.map(TryInto::try_into).transpose()?;
        let upper_boundary = upper_boundary.map(TryInto::try_into).transpose()?;

        // Get the mean and standard deviation of the historical data.
        let Some(mean) = mean(data) else {
            return Ok(None);
        };
        let Some(std_dev) = std_deviation(mean, data) else {
            return Ok(None);
        };

        let limits = MetricsLimits::new_statistical(
            log,
            mean,
            std_dev,
            test_kind,
            lower_boundary,
            upper_boundary,
        )?;
        let outlier = limits.outlier(datum);

        Ok(Some(Self { limits, outlier }))
    }
}

#[allow(clippy::cast_precision_loss)]
fn mean(data: &[f64]) -> Option<f64> {
    if data.is_empty() {
        return None;
    }
    Some(data.iter().sum::<f64>() / data.len() as f64)
}

fn std_deviation(mean: f64, data: &[f64]) -> Option<f64> {
    variance(mean, data)
    // If the variance is zero then the standard deviation is not going to work with `statrs`
        .and_then(|std_dev| if std_dev == 0.0 { None } else { Some(std_dev) })
        .map(f64::sqrt)
}

#[allow(clippy::cast_precision_loss)]
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

#[cfg(test)]
#[allow(clippy::float_cmp, clippy::unreadable_literal, clippy::unwrap_used)]
mod test {
    use pretty_assertions::assert_eq;

    use super::{mean, std_deviation, variance};

    const DATA_ZERO: [f64; 0] = [];
    const DATA_ONE: [f64; 1] = [1.0];
    const DATA_TWO: [f64; 2] = [1.0, 2.0];
    const DATA_THREE: [f64; 3] = [1.0, 2.0, 3.0];
    const DATA_FIVE: [f64; 5] = [1.0, 2.0, 3.0, 4.0, 5.0];
    const DATA_CONST: [f64; 5] = [1.0, 1.0, 1.0, 1.0, 1.0];

    const MEAN_ZERO: f64 = 0.0;
    const MEAN_ONE: f64 = 1.0;
    const MEAN_TWO: f64 = 1.5;
    const MEAN_THREE: f64 = 2.0;
    const MEAN_FIVE: f64 = 3.0;

    #[test]
    fn test_mean_zero() {
        let m = mean(&DATA_ZERO);
        assert_eq!(m, None);
    }

    #[test]
    fn test_mean_one() {
        let m = mean(&DATA_ONE).unwrap();
        assert_eq!(m, MEAN_ONE);
    }

    #[test]
    fn test_mean_two() {
        let m = mean(&DATA_TWO).unwrap();
        assert_eq!(m, MEAN_TWO);
    }

    #[test]
    fn test_mean_three() {
        let m = mean(&DATA_THREE).unwrap();
        assert_eq!(m, MEAN_THREE);
    }

    #[test]
    fn test_mean_five() {
        let m = mean(&DATA_FIVE).unwrap();
        assert_eq!(m, MEAN_FIVE);
    }

    #[test]
    fn test_mean_const() {
        let m = mean(&DATA_CONST).unwrap();
        assert_eq!(m, MEAN_ONE);
    }

    #[test]
    fn test_variance_zero() {
        let v = variance(MEAN_ZERO, &DATA_ZERO);
        assert_eq!(v, None);

        let v = variance(MEAN_ONE, &DATA_ZERO);
        assert_eq!(v, None);

        let v = variance(MEAN_TWO, &DATA_ZERO);
        assert_eq!(v, None);

        let v = variance(MEAN_THREE, &DATA_ZERO);
        assert_eq!(v, None);

        let v = variance(MEAN_FIVE, &DATA_ZERO);
        assert_eq!(v, None);
    }

    #[test]
    fn test_variance_one() {
        let v = variance(MEAN_ZERO, &DATA_ONE);
        assert_eq!(v, None);

        let v = variance(MEAN_ONE, &DATA_ONE);
        assert_eq!(v, None);

        let v = variance(MEAN_TWO, &DATA_ONE);
        assert_eq!(v, None);

        let v = variance(MEAN_THREE, &DATA_ONE);
        assert_eq!(v, None);

        let v = variance(MEAN_FIVE, &DATA_ONE);
        assert_eq!(v, None);
    }

    #[test]
    fn test_variance_two() {
        let v = variance(MEAN_ZERO, &DATA_TWO).unwrap();
        assert_eq!(v, 2.5);

        let v = variance(MEAN_ONE, &DATA_TWO).unwrap();
        assert_eq!(v, 0.5);

        let v = variance(MEAN_TWO, &DATA_TWO).unwrap();
        assert_eq!(v, 0.25);

        let v = variance(MEAN_THREE, &DATA_TWO).unwrap();
        assert_eq!(v, 0.5);

        let v = variance(MEAN_FIVE, &DATA_TWO).unwrap();
        assert_eq!(v, 2.5);
    }

    #[test]
    fn test_variance_three() {
        let v = variance(MEAN_ZERO, &DATA_THREE).unwrap();
        assert_eq!(v, 4.666666666666667);

        let v = variance(MEAN_ONE, &DATA_THREE).unwrap();
        assert_eq!(v, 1.6666666666666667);

        let v = variance(MEAN_TWO, &DATA_THREE).unwrap();
        assert_eq!(v, 0.9166666666666666);

        let v = variance(MEAN_THREE, &DATA_THREE).unwrap();
        assert_eq!(v, 0.6666666666666666);

        let v = variance(MEAN_FIVE, &DATA_THREE).unwrap();
        assert_eq!(v, 1.6666666666666667);
    }

    #[test]
    fn test_variance_five() {
        let v = variance(MEAN_ZERO, &DATA_FIVE).unwrap();
        assert_eq!(v, 11.0);

        let v = variance(MEAN_ONE, &DATA_FIVE).unwrap();
        assert_eq!(v, 6.0);

        let v = variance(MEAN_TWO, &DATA_FIVE).unwrap();
        assert_eq!(v, 4.25);

        let v = variance(MEAN_THREE, &DATA_FIVE).unwrap();
        assert_eq!(v, 3.0);

        let v = variance(MEAN_FIVE, &DATA_FIVE).unwrap();
        assert_eq!(v, 2.0);
    }

    #[test]
    fn test_variance_const() {
        let v = variance(MEAN_ZERO, &DATA_CONST).unwrap();
        assert_eq!(v, 1.0);

        let v = variance(MEAN_ONE, &DATA_CONST).unwrap();
        assert_eq!(v, 0.0);

        let v = variance(MEAN_TWO, &DATA_CONST).unwrap();
        assert_eq!(v, 0.25);

        let v = variance(MEAN_THREE, &DATA_CONST).unwrap();
        assert_eq!(v, 1.0);

        let v = variance(MEAN_FIVE, &DATA_CONST).unwrap();
        assert_eq!(v, 4.0);
    }

    #[test]
    fn test_std_dev_zero() {
        let std_dev = std_deviation(MEAN_ZERO, &DATA_ZERO);
        assert_eq!(std_dev, None);

        let std_dev = std_deviation(MEAN_ONE, &DATA_ZERO);
        assert_eq!(std_dev, None);

        let std_dev = std_deviation(MEAN_TWO, &DATA_ZERO);
        assert_eq!(std_dev, None);

        let std_dev = std_deviation(MEAN_THREE, &DATA_ZERO);
        assert_eq!(std_dev, None);

        let std_dev = std_deviation(MEAN_FIVE, &DATA_ZERO);
        assert_eq!(std_dev, None);

        let std_dev = std_deviation(MEAN_ZERO, &DATA_ZERO);
        assert_eq!(std_dev, None);
    }

    #[test]
    fn test_std_dev_one() {
        let std_dev = std_deviation(MEAN_ZERO, &DATA_ONE);
        assert_eq!(std_dev, None);

        let std_dev = std_deviation(MEAN_ONE, &DATA_ONE);
        assert_eq!(std_dev, None);

        let std_dev = std_deviation(MEAN_TWO, &DATA_ONE);
        assert_eq!(std_dev, None);

        let std_dev = std_deviation(MEAN_THREE, &DATA_ONE);
        assert_eq!(std_dev, None);

        let std_dev = std_deviation(MEAN_FIVE, &DATA_ONE);
        assert_eq!(std_dev, None);
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn test_std_dev_two() {
        let std_dev = std_deviation(MEAN_ZERO, &DATA_TWO).unwrap();
        assert_eq!(std_dev, 1.5811388300841898);

        let std_dev = std_deviation(MEAN_ONE, &DATA_TWO).unwrap();
        assert_eq!(std_dev, 0.7071067811865476);

        let std_dev = std_deviation(MEAN_TWO, &DATA_TWO).unwrap();
        assert_eq!(std_dev, 0.5);

        let std_dev = std_deviation(MEAN_THREE, &DATA_TWO).unwrap();
        assert_eq!(std_dev, 0.7071067811865476);

        let std_dev = std_deviation(MEAN_FIVE, &DATA_TWO).unwrap();
        assert_eq!(std_dev, 1.5811388300841898);
    }

    #[test]
    fn test_std_dev_three() {
        let std_dev = std_deviation(MEAN_ZERO, &DATA_THREE).unwrap();
        assert_eq!(std_dev, 2.160246899469287);

        let std_dev = std_deviation(MEAN_ONE, &DATA_THREE).unwrap();
        assert_eq!(std_dev, 1.2909944487358056);

        let std_dev = std_deviation(MEAN_TWO, &DATA_THREE).unwrap();
        assert_eq!(std_dev, 0.9574271077563381);

        let std_dev = std_deviation(MEAN_THREE, &DATA_THREE).unwrap();
        assert_eq!(std_dev, 0.816496580927726);

        let std_dev = std_deviation(MEAN_FIVE, &DATA_THREE).unwrap();
        assert_eq!(std_dev, 1.2909944487358056);
    }

    #[test]
    #[allow(clippy::approx_constant)]
    fn test_std_dev_five() {
        let std_dev = std_deviation(MEAN_ZERO, &DATA_FIVE).unwrap();
        assert_eq!(std_dev, 3.3166247903554);

        let std_dev = std_deviation(MEAN_ONE, &DATA_FIVE).unwrap();
        assert_eq!(std_dev, 2.449489742783178);

        let std_dev = std_deviation(MEAN_TWO, &DATA_FIVE).unwrap();
        assert_eq!(std_dev, 2.0615528128088303);

        let std_dev = std_deviation(MEAN_THREE, &DATA_FIVE).unwrap();
        assert_eq!(std_dev, 1.7320508075688772);

        let std_dev = std_deviation(MEAN_FIVE, &DATA_FIVE).unwrap();
        assert_eq!(std_dev, 1.4142135623730951);
    }

    #[test]
    fn test_std_dev_const() {
        let std_dev = std_deviation(MEAN_ZERO, &DATA_CONST).unwrap();
        assert_eq!(std_dev, 1.0);

        let std_dev = std_deviation(MEAN_ONE, &DATA_CONST);
        assert_eq!(std_dev, None);

        let std_dev = std_deviation(MEAN_TWO, &DATA_CONST).unwrap();
        assert_eq!(std_dev, 0.5);

        let std_dev = std_deviation(MEAN_THREE, &DATA_CONST).unwrap();
        assert_eq!(std_dev, 1.0);

        let std_dev = std_deviation(MEAN_FIVE, &DATA_CONST).unwrap();
        assert_eq!(std_dev, 2.0);
    }
}
