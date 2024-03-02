use bencher_json::{project::boundary::BoundaryLimit, Boundary, ModelTest, SampleSize};
use slog::Logger;

use crate::limits::{MetricsLimits, NormalTestKind};
use crate::ln::Ln;
use crate::mean::Mean;
use crate::quartiles::Quartiles;
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
        model_test: ModelTest,
        min_sample_size: Option<SampleSize>,
        lower_boundary: Option<Boundary>,
        upper_boundary: Option<Boundary>,
    ) -> Result<Self, BoundaryError> {
        Self::new_inner(
            log,
            datum,
            metrics_data,
            model_test,
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
        model_test: ModelTest,
        min_sample_size: Option<SampleSize>,
        lower_boundary: Option<Boundary>,
        upper_boundary: Option<Boundary>,
    ) -> Result<Option<Self>, BoundaryError> {
        // If there is no boundary, then simply return.
        if lower_boundary.is_none() && upper_boundary.is_none() {
            slog::debug!(
                log,
                "No lower or upper boundary for threshold model test {model_test:?}",
            );
            return Ok(None);
        }
        let data = &metrics_data.data;
        let data_len = data.len();
        // If there is a min sample size, then check to see if it is met.
        // Otherwise, simply return.
        if let Some(min_sample_size) = min_sample_size {
            if data_len < min_sample_size.into() {
                slog::debug!(
                    log,
                    "Data length ({data_len}) is less than min sample size ({min_sample_size})",
                );
                return Ok(None);
            }
        } else if data_len == 0 {
            slog::debug!(log, "No data for threshold model test {model_test:?}");
            return Ok(None);
        }

        match model_test {
            ModelTest::Static => Ok(Some(Self::new_static(
                datum,
                lower_boundary,
                upper_boundary,
            ))),
            ModelTest::Percentage => {
                Self::new_percentage(log, datum, data, lower_boundary, upper_boundary)
            },
            ModelTest::ZScore => Self::new_normal(
                log,
                datum,
                data,
                NormalTestKind::Z,
                lower_boundary,
                upper_boundary,
            ),
            ModelTest::TTest => Self::new_normal(
                log,
                datum,
                data,
                #[allow(clippy::cast_precision_loss)]
                NormalTestKind::T {
                    freedom: (data.len() - 1) as f64,
                },
                lower_boundary,
                upper_boundary,
            ),
            ModelTest::LogNormal => {
                Self::new_log_normal(log, datum, data, lower_boundary, upper_boundary)
            },
            ModelTest::Iqr => {
                Self::new_iqr(log, datum, data, false, lower_boundary, upper_boundary)
            },
            ModelTest::DeltaIqr => {
                Self::new_iqr(log, datum, data, true, lower_boundary, upper_boundary)
            },
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
        let lower_boundary = lower_boundary
            .map(TryInto::try_into)
            .transpose()
            .map_err(BoundaryError::Valid)?;
        let upper_boundary = upper_boundary
            .map(TryInto::try_into)
            .transpose()
            .map_err(BoundaryError::Valid)?;

        // Get the mean of the historical data.
        let Some(Mean { mean }) = Mean::new(data) else {
            return Ok(None);
        };

        let limits = MetricsLimits::new_percentage(log, mean, lower_boundary, upper_boundary);
        let outlier = limits.outlier(datum);

        Ok(Some(Self { limits, outlier }))
    }

    fn new_normal(
        log: &Logger,
        datum: f64,
        data: &[f64],
        test_kind: NormalTestKind,
        lower_boundary: Option<Boundary>,
        upper_boundary: Option<Boundary>,
    ) -> Result<Option<Self>, BoundaryError> {
        let lower_boundary = lower_boundary
            .map(TryInto::try_into)
            .transpose()
            .map_err(BoundaryError::Valid)?;
        let upper_boundary = upper_boundary
            .map(TryInto::try_into)
            .transpose()
            .map_err(BoundaryError::Valid)?;

        // Get the mean and standard deviation of the historical data.
        let Some(mean) = Mean::new(data) else {
            return Ok(None);
        };
        let Some(std_dev) = mean.std_deviation(data) else {
            return Ok(None);
        };
        let Mean { mean } = mean;

        let limits = MetricsLimits::new_normal(
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

    fn new_log_normal(
        log: &Logger,
        datum: f64,
        data: &[f64],
        lower_boundary: Option<Boundary>,
        upper_boundary: Option<Boundary>,
    ) -> Result<Option<Self>, BoundaryError> {
        let lower_boundary = lower_boundary
            .map(TryInto::try_into)
            .transpose()
            .map_err(BoundaryError::Valid)?;
        let upper_boundary = upper_boundary
            .map(TryInto::try_into)
            .transpose()
            .map_err(BoundaryError::Valid)?;

        let Some(ln) = Ln::new(data) else {
            return Ok(None);
        };

        let limits = MetricsLimits::new_log_normal(log, ln, lower_boundary, upper_boundary)?;
        let outlier = limits.outlier(datum);

        Ok(Some(Self { limits, outlier }))
    }

    fn new_iqr(
        log: &Logger,
        datum: f64,
        data: &[f64],
        delta: bool,
        lower_boundary: Option<Boundary>,
        upper_boundary: Option<Boundary>,
    ) -> Result<Option<Self>, BoundaryError> {
        let lower_boundary = lower_boundary
            .map(TryInto::try_into)
            .transpose()
            .map_err(BoundaryError::Valid)?;
        let upper_boundary = upper_boundary
            .map(TryInto::try_into)
            .transpose()
            .map_err(BoundaryError::Valid)?;

        let Some(quartiles) = Quartiles::new(data) else {
            return Ok(None);
        };
        let delta_quartiles = if delta {
            if let Some(delta_quartiles) = Quartiles::new_delta(data) {
                Some(delta_quartiles)
            } else {
                return Ok(None);
            }
        } else {
            None
        };

        let limits = MetricsLimits::new_iqr(
            log,
            quartiles,
            delta_quartiles,
            lower_boundary,
            upper_boundary,
        );
        let outlier = limits.outlier(datum);

        Ok(Some(Self { limits, outlier }))
    }
}
