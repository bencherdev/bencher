use bencher_json::project::boundary::BoundaryLimit;
use bencher_json::project::threshold::StatisticKind;
use bencher_json::{Boundary, SampleSize};
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
            StatisticKind::ZScore => Self::new_normal(
                log,
                datum,
                data,
                NormalTestKind::Z,
                lower_boundary,
                upper_boundary,
            ),
            StatisticKind::TTest => Self::new_normal(
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
            StatisticKind::LogNormal => {
                Self::new_log_normal(log, datum, data, lower_boundary, upper_boundary)
            },
            StatisticKind::Iqr => {
                Self::new_iqr(log, datum, data, false, lower_boundary, upper_boundary)
            },
            StatisticKind::DeltaIqr => {
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
        let lower_boundary = lower_boundary.map(TryInto::try_into).transpose()?;
        let upper_boundary = upper_boundary.map(TryInto::try_into).transpose()?;

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
        let lower_boundary = lower_boundary.map(TryInto::try_into).transpose()?;
        let upper_boundary = upper_boundary.map(TryInto::try_into).transpose()?;

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
        let lower_boundary = lower_boundary.map(TryInto::try_into).transpose()?;
        let upper_boundary = upper_boundary.map(TryInto::try_into).transpose()?;

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
        deltas: bool,
        lower_boundary: Option<Boundary>,
        upper_boundary: Option<Boundary>,
    ) -> Result<Option<Self>, BoundaryError> {
        let lower_boundary = lower_boundary.map(TryInto::try_into).transpose()?;
        let upper_boundary = upper_boundary.map(TryInto::try_into).transpose()?;

        let Some(quartiles) = Quartiles::new(data) else {
            return Ok(None);
        };
        let delta_quartiles = if deltas {
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
