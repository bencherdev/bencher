use bencher_json::{
    project::boundary::BoundaryLimit, Boundary, CdfBoundary, IqrBoundary, PercentageBoundary,
};
use slog::{debug, Logger};
use statrs::distribution::{ContinuousCDF, LogNormal, Normal, StudentsT};

use crate::{ln::Ln, quartiles::Quartiles, BoundaryError};

mod limit;

use limit::MetricsLimit;

#[derive(Debug, Default)]
pub struct MetricsLimits {
    pub baseline: Option<f64>,
    pub lower: Option<MetricsLimit>,
    pub upper: Option<MetricsLimit>,
}

#[derive(Clone, Copy)]
pub enum NormalTestKind {
    Z,
    T { freedom: f64 },
}

impl MetricsLimits {
    pub fn new_static(lower_boundary: Option<Boundary>, upper_boundary: Option<Boundary>) -> Self {
        Self {
            baseline: None,
            lower: lower_boundary.map(Into::into),
            upper: upper_boundary.map(Into::into),
        }
    }

    pub fn new_percentage(
        log: &Logger,
        mean: f64,
        lower_boundary: Option<PercentageBoundary>,
        upper_boundary: Option<PercentageBoundary>,
    ) -> Self {
        if lower_boundary.is_none() && upper_boundary.is_none() {
            return Self::default();
        }

        debug!(log, "Percentage: mean={mean}");
        let lower = lower_boundary.map(|boundary| MetricsLimit::percentage_lower(mean, boundary));
        let upper = upper_boundary.map(|boundary| MetricsLimit::percentage_upper(mean, boundary));

        Self {
            baseline: Some(mean),
            lower,
            upper,
        }
    }

    pub fn new_normal(
        log: &Logger,
        mean: f64,
        std_dev: f64,
        test_kind: NormalTestKind,
        lower_boundary: Option<CdfBoundary>,
        upper_boundary: Option<CdfBoundary>,
    ) -> Result<Self, BoundaryError> {
        if lower_boundary.is_none() && upper_boundary.is_none() {
            return Ok(Self::default());
        }

        Ok(match test_kind {
            // Create a normal distribution and calculate the boundary limits for the threshold based on the boundary percentiles.
            NormalTestKind::Z => {
                debug!(log, "Normal distribution: mean={mean}, std_dev={std_dev}");
                let normal = Normal::new(mean, std_dev).map_err(|error| BoundaryError::Normal {
                    mean,
                    std_dev,
                    error,
                })?;
                let lower = lower_boundary.map(|limit| {
                    let abs_limit = normal.inverse_cdf(limit.into());
                    MetricsLimit::inverse_cdf_lower(mean, abs_limit)
                });
                let upper = upper_boundary.map(|limit| {
                    let abs_limit = normal.inverse_cdf(limit.into());
                    MetricsLimit::inverse_cdf_upper(abs_limit)
                });
                Self {
                    baseline: Some(mean),
                    lower,
                    upper,
                }
            },
            // Create a Student's t distribution and calculate the boundary limits for the threshold based on the boundary percentiles.
            NormalTestKind::T { freedom } => {
                debug!(
                    log,
                    "Students T distribution: mean={mean}, scale={std_dev}, freedom={freedom}"
                );
                let students_t = StudentsT::new(mean, std_dev, freedom).map_err(|error| {
                    BoundaryError::StudentsT {
                        mean,
                        std_dev,
                        freedom,
                        error,
                    }
                })?;
                let lower = lower_boundary.map(|limit| {
                    let abs_limit = students_t.inverse_cdf(limit.into());
                    MetricsLimit::inverse_cdf_lower(mean, abs_limit)
                });
                let upper = upper_boundary.map(|limit| {
                    let abs_limit = students_t.inverse_cdf(limit.into());
                    MetricsLimit::inverse_cdf_upper(abs_limit)
                });
                Self {
                    baseline: Some(mean),
                    lower,
                    upper,
                }
            },
        })
    }

    pub fn new_log_normal(
        log: &Logger,
        ln: Ln,
        lower_boundary: Option<CdfBoundary>,
        upper_boundary: Option<CdfBoundary>,
    ) -> Result<Self, BoundaryError> {
        if lower_boundary.is_none() && upper_boundary.is_none() {
            return Ok(Self::default());
        }

        let Ln { location, scale } = ln;
        debug!(
            log,
            "Log Normal distribution: location={location}, scale={scale}"
        );
        let normal = LogNormal::new(location, scale).map_err(|error| BoundaryError::LogNormal {
            location,
            scale,
            error,
        })?;
        let baseline = location.exp();
        let lower = lower_boundary.map(|limit| {
            let abs_limit = normal.inverse_cdf(limit.into());
            MetricsLimit::inverse_cdf_lower(baseline, abs_limit)
        });
        let upper = upper_boundary.map(|limit| {
            let abs_limit = normal.inverse_cdf(limit.into());
            MetricsLimit::inverse_cdf_upper(abs_limit)
        });

        Ok(Self {
            baseline: Some(baseline),
            lower,
            upper,
        })
    }

    pub fn new_iqr(
        log: &Logger,
        quartiles: Quartiles,
        delta_quartiles: Option<Quartiles>,
        lower_boundary: Option<IqrBoundary>,
        upper_boundary: Option<IqrBoundary>,
    ) -> Self {
        if lower_boundary.is_none() && upper_boundary.is_none() {
            return Self::default();
        }

        if let Some(delta_quartiles) = delta_quartiles {
            debug!(
                log,
                "Delta IQR: quartiles={quartiles:?}, delta_quartiles={delta_quartiles:?}"
            );
            let lower = lower_boundary.map(|boundary| {
                MetricsLimit::delta_iqr_lower(quartiles, delta_quartiles, boundary)
            });
            let upper = upper_boundary.map(|boundary| {
                MetricsLimit::delta_iqr_upper(quartiles, delta_quartiles, boundary)
            });

            Self {
                baseline: Some(quartiles.q2),
                lower,
                upper,
            }
        } else {
            debug!(log, "IQR: quartiles={quartiles:?}");
            let lower = lower_boundary.map(|boundary| MetricsLimit::iqr_lower(quartiles, boundary));
            let upper = upper_boundary.map(|boundary| MetricsLimit::iqr_upper(quartiles, boundary));

            Self {
                baseline: Some(quartiles.q2),
                lower,
                upper,
            }
        }
    }

    // An outlier occurs when the  datum exceeds a boundary limit.
    pub fn outlier(&self, datum: f64) -> Option<BoundaryLimit> {
        match (self.lower.as_ref(), self.upper.as_ref()) {
            (Some(lower), Some(upper)) => {
                if datum < lower.value {
                    Some(BoundaryLimit::Lower)
                } else if datum > upper.value {
                    Some(BoundaryLimit::Upper)
                } else {
                    None
                }
            },
            (Some(lower), None) => (datum < lower.value).then_some(BoundaryLimit::Lower),
            (None, Some(upper)) => (datum > upper.value).then_some(BoundaryLimit::Upper),
            (None, None) => None,
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unreadable_literal, clippy::unwrap_used)]
mod test {
    use bencher_json::{
        project::boundary::BoundaryLimit, Boundary, CdfBoundary, IqrBoundary, PercentageBoundary,
    };
    use bencher_logger::bootstrap_logger;
    use once_cell::sync::Lazy;
    use ordered_float::OrderedFloat;
    use pretty_assertions::assert_eq;

    use crate::{ln::Ln, quartiles::Quartiles};

    use super::{MetricsLimit, MetricsLimits, NormalTestKind};

    const MEAN: f64 = 0.0;
    const STD_DEV: f64 = 1.0;
    const FREEDOM: f64 = 5.0;

    static NEGATIVE_STATIC_LIMIT: Lazy<Boundary> =
        Lazy::new(|| (-5.0).try_into().expect("Failed to parse boundary."));
    static STATIC_LIMIT: Lazy<Boundary> =
        Lazy::new(|| 5.0.try_into().expect("Failed to parse boundary."));

    const STATIC_NEGATIVE_OUTLIER: f64 = -10.0;
    const STATIC_NEGATIVE: f64 = -3.0;
    const STATIC_ONE: f64 = 1.0;
    const STATIC_POSITIVE: f64 = 3.0;
    const STATIC_POSITIVE_OUTLIER: f64 = 10.0;

    static PERCENTAGE: Lazy<PercentageBoundary> = Lazy::new(|| {
        5.0.try_into()
            .expect("Failed to parse percentage boundary.")
    });
    const PERCENTAGE_NEGATIVE: f64 = -4.0;
    const PERCENTAGE_POSITIVE: f64 = 6.0;

    static PERCENTILE: Lazy<CdfBoundary> = Lazy::new(|| {
        0.85.try_into()
            .expect("Failed to parse statistical boundary.")
    });
    const Z_LIMIT: f64 = 1.0364333894937896;
    const T_LIMIT: f64 = 1.1557673428942912;

    const LOG_DATA: &[f64] = &[
        1.0, 2.0, 3.0, 4.0, 5.0, 1.0, 2.0, 3.0, 4.0, 5.0, 1.0, 2.0, 3.0, 4.0, 5.0, 1.0, 2.0, 3.0,
        4.0, 5.0, 1.0, 2.0, 3.0, 4.0, 5.0, 1.0, 2.0, 3.0, 4.0, 5.0,
    ];
    const LOG_LOCATION: f64 = 2.6051710846973517;
    const LOG_LOWER: f64 = 0.5147092348243909;
    const LOG_UPPER: f64 = 4.6956329345703125;

    const IQR_Q1: f64 = 1.0;
    const IQR_Q2: f64 = 2.0;
    const IQR_Q3: f64 = 3.0;

    const QUARTILES: Quartiles = Quartiles {
        q1: IQR_Q1,
        q2: IQR_Q2,
        q3: IQR_Q3,
    };
    const DELTA_QUARTILES: Quartiles = Quartiles {
        q1: 0.5,
        q2: 1.0,
        q3: 1.5,
    };
    static IQR_MULTIPLIER: Lazy<IqrBoundary> = Lazy::new(|| {
        1.5.try_into()
            .expect("Failed to parse inter-quartile range boundary.")
    });
    const IQR_NEGATIVE_LIMIT: f64 = -1.0;
    const IQR_POSITIVE_LIMIT: f64 = 5.0;

    const LOG_NORMAL_NEGATIVE_OUTLIER: f64 = -3.0;
    const NORMAL_NEGATIVE_OUTLIER: f64 = -1.5;
    const NORMAL_NEGATIVE: f64 = -1.0;
    const NORMAL_ZERO: f64 = 0.0;
    const NORMAL_POSITIVE: f64 = 1.0;
    const NORMAL_POSITIVE_OUTLIER: f64 = 1.5;
    const LOG_NORMAL_POSITIVE_OUTLIER: f64 = 3.0;
    const IQR_POSITIVE_OUTLIER: f64 = 6.0;

    #[test]
    fn test_limits_static_none() {
        let limits = MetricsLimits::new_static(None, None);
        assert_eq!(limits.baseline, None);
        assert_eq!(limits.lower, None);
        assert_eq!(limits.upper, None);

        let side = limits.outlier(STATIC_NEGATIVE_OUTLIER);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_ONE);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_POSITIVE_OUTLIER);
        assert_eq!(side, None);
    }

    #[test]
    fn test_limits_static_lower() {
        let limits = MetricsLimits::new_static(Some(*NEGATIVE_STATIC_LIMIT), None);
        assert_eq!(limits.baseline, None);
        assert_eq!(
            limits.lower,
            Some(MetricsLimit {
                value: (*NEGATIVE_STATIC_LIMIT).into()
            })
        );
        assert_eq!(limits.upper, None);

        let side = limits.outlier(STATIC_NEGATIVE_OUTLIER);
        assert_eq!(side, Some(BoundaryLimit::Lower));

        let side = limits.outlier(STATIC_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_ONE);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_POSITIVE_OUTLIER);
        assert_eq!(side, None);
    }

    #[test]
    fn test_limits_static_upper() {
        let _log = bootstrap_logger();
        let limits = MetricsLimits::new_static(None, Some(*STATIC_LIMIT));
        assert_eq!(limits.baseline, None);
        assert_eq!(limits.lower, None);
        assert_eq!(
            limits.upper,
            Some(MetricsLimit {
                value: (*STATIC_LIMIT).into()
            })
        );

        let side = limits.outlier(STATIC_NEGATIVE_OUTLIER);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_ONE);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_POSITIVE_OUTLIER);
        assert_eq!(side, Some(BoundaryLimit::Upper));
    }

    #[test]
    fn test_limits_static_both() {
        let limits = MetricsLimits::new_static(Some(*NEGATIVE_STATIC_LIMIT), Some(*STATIC_LIMIT));
        assert_eq!(limits.baseline, None);
        assert_eq!(
            limits.lower,
            Some(MetricsLimit {
                value: (*NEGATIVE_STATIC_LIMIT).into()
            })
        );
        assert_eq!(
            limits.upper,
            Some(MetricsLimit {
                value: (*STATIC_LIMIT).into()
            })
        );

        let side = limits.outlier(STATIC_NEGATIVE_OUTLIER);
        assert_eq!(side, Some(BoundaryLimit::Lower));

        let side = limits.outlier(STATIC_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_ONE);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_POSITIVE_OUTLIER);
        assert_eq!(side, Some(BoundaryLimit::Upper));
    }

    #[test]
    fn test_limits_percentage_none() {
        let log = bootstrap_logger();
        let limits = MetricsLimits::new_percentage(&log, STATIC_ONE, None, None);
        assert_eq!(limits.baseline, None);
        assert_eq!(limits.lower, None);
        assert_eq!(limits.upper, None);

        let side = limits.outlier(STATIC_NEGATIVE_OUTLIER);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_ONE);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_POSITIVE_OUTLIER);
        assert_eq!(side, None);
    }

    #[test]
    fn test_limits_percentage_lower() {
        let log = bootstrap_logger();
        let limits = MetricsLimits::new_percentage(&log, STATIC_ONE, Some(*PERCENTAGE), None);
        assert_eq!(
            OrderedFloat::from(limits.baseline.unwrap()),
            OrderedFloat::from(STATIC_ONE)
        );
        assert_eq!(
            limits.lower,
            Some(MetricsLimit {
                value: PERCENTAGE_NEGATIVE
            })
        );
        assert_eq!(limits.upper, None);

        let side = limits.outlier(STATIC_NEGATIVE_OUTLIER);
        assert_eq!(side, Some(BoundaryLimit::Lower));

        let side = limits.outlier(STATIC_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_ONE);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_POSITIVE_OUTLIER);
        assert_eq!(side, None);
    }

    #[test]
    fn test_limits_percentage_upper() {
        let log = bootstrap_logger();
        let limits = MetricsLimits::new_percentage(&log, STATIC_ONE, None, Some(*PERCENTAGE));
        assert_eq!(
            OrderedFloat::from(limits.baseline.unwrap()),
            OrderedFloat::from(STATIC_ONE)
        );
        assert_eq!(limits.lower, None);
        assert_eq!(
            limits.upper,
            Some(MetricsLimit {
                value: PERCENTAGE_POSITIVE
            })
        );

        let side = limits.outlier(STATIC_NEGATIVE_OUTLIER);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_ONE);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_POSITIVE_OUTLIER);
        assert_eq!(side, Some(BoundaryLimit::Upper));
    }

    #[test]
    fn test_limits_percentage_both() {
        let log = bootstrap_logger();
        let limits =
            MetricsLimits::new_percentage(&log, STATIC_ONE, Some(*PERCENTAGE), Some(*PERCENTAGE));
        assert_eq!(
            OrderedFloat::from(limits.baseline.unwrap()),
            OrderedFloat::from(STATIC_ONE)
        );
        assert_eq!(
            limits.lower,
            Some(MetricsLimit {
                value: PERCENTAGE_NEGATIVE
            })
        );
        assert_eq!(
            limits.upper,
            Some(MetricsLimit {
                value: PERCENTAGE_POSITIVE
            })
        );

        let side = limits.outlier(STATIC_NEGATIVE_OUTLIER);
        assert_eq!(side, Some(BoundaryLimit::Lower));

        let side = limits.outlier(STATIC_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_ONE);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(STATIC_POSITIVE_OUTLIER);
        assert_eq!(side, Some(BoundaryLimit::Upper));
    }

    #[test]
    fn test_limits_z_none() {
        let log = bootstrap_logger();
        let limits =
            MetricsLimits::new_normal(&log, MEAN, STD_DEV, NormalTestKind::Z, None, None).unwrap();
        assert_eq!(limits.baseline, None);
        assert_eq!(limits.lower, None);
        assert_eq!(limits.upper, None);

        let side = limits.outlier(NORMAL_NEGATIVE_OUTLIER);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_ZERO);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE_OUTLIER);
        assert_eq!(side, None);
    }

    #[test]
    fn test_limits_z_lower() {
        let log = bootstrap_logger();
        let limits = MetricsLimits::new_normal(
            &log,
            MEAN,
            STD_DEV,
            NormalTestKind::Z,
            Some(*PERCENTILE),
            None,
        )
        .unwrap();
        assert_eq!(
            OrderedFloat::from(limits.baseline.unwrap()),
            OrderedFloat::from(MEAN)
        );
        assert_eq!(limits.lower, Some(MetricsLimit { value: -Z_LIMIT }));
        assert_eq!(limits.upper, None);

        let side = limits.outlier(NORMAL_NEGATIVE_OUTLIER);
        assert_eq!(side, Some(BoundaryLimit::Lower));

        let side = limits.outlier(NORMAL_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_ZERO);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE_OUTLIER);
        assert_eq!(side, None);
    }

    #[test]
    fn test_limits_z_upper() {
        let log = bootstrap_logger();
        let limits = MetricsLimits::new_normal(
            &log,
            MEAN,
            STD_DEV,
            NormalTestKind::Z,
            None,
            Some(*PERCENTILE),
        )
        .unwrap();
        assert_eq!(
            OrderedFloat::from(limits.baseline.unwrap()),
            OrderedFloat::from(MEAN)
        );
        assert_eq!(limits.lower, None);
        assert_eq!(limits.upper, Some(MetricsLimit { value: Z_LIMIT }));

        let side = limits.outlier(NORMAL_NEGATIVE_OUTLIER);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_ZERO);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE_OUTLIER);
        assert_eq!(side, Some(BoundaryLimit::Upper));
    }

    #[test]
    fn test_limits_z_both() {
        let log = bootstrap_logger();
        let limits = MetricsLimits::new_normal(
            &log,
            MEAN,
            STD_DEV,
            NormalTestKind::Z,
            Some(*PERCENTILE),
            Some(*PERCENTILE),
        )
        .unwrap();
        assert_eq!(
            OrderedFloat::from(limits.baseline.unwrap()),
            OrderedFloat::from(MEAN)
        );
        assert_eq!(limits.lower, Some(MetricsLimit { value: -Z_LIMIT }));
        assert_eq!(limits.upper, Some(MetricsLimit { value: Z_LIMIT }));

        let side = limits.outlier(NORMAL_NEGATIVE_OUTLIER);
        assert_eq!(side, Some(BoundaryLimit::Lower));

        let side = limits.outlier(NORMAL_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_ZERO);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE_OUTLIER);
        assert_eq!(side, Some(BoundaryLimit::Upper));
    }

    #[test]
    fn test_limits_z_docs() {
        const MEAN_100: f64 = 100.0;
        let log = bootstrap_logger();
        let boundary = 0.977.try_into().expect("Failed to create boundary.");
        let limits = MetricsLimits::new_normal(
            &log,
            MEAN_100,
            10.0,
            NormalTestKind::Z,
            Some(boundary),
            Some(boundary),
        )
        .unwrap();
        assert_eq!(
            OrderedFloat::from(limits.baseline.unwrap()),
            OrderedFloat::from(MEAN_100)
        );
        assert_eq!(
            limits.lower,
            Some(MetricsLimit {
                value: 80.04606689832175
            })
        );
        assert_eq!(
            limits.upper,
            Some(MetricsLimit {
                value: 119.95393310167825
            })
        );

        let side = limits.outlier(75.0);
        assert_eq!(side, Some(BoundaryLimit::Lower));

        let side = limits.outlier(90.0);
        assert_eq!(side, None);

        let side = limits.outlier(100.0);
        assert_eq!(side, None);

        let side = limits.outlier(110.0);
        assert_eq!(side, None);

        let side = limits.outlier(125.0);
        assert_eq!(side, Some(BoundaryLimit::Upper));
    }

    #[test]
    fn test_limits_t_none() {
        let log = bootstrap_logger();
        let limits = MetricsLimits::new_normal(
            &log,
            MEAN,
            STD_DEV,
            NormalTestKind::T { freedom: FREEDOM },
            None,
            None,
        )
        .unwrap();
        assert_eq!(limits.baseline, None);
        assert_eq!(limits.lower, None);
        assert_eq!(limits.upper, None);

        let side = limits.outlier(NORMAL_NEGATIVE_OUTLIER);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_ZERO);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE_OUTLIER);
        assert_eq!(side, None);
    }

    #[test]
    fn test_limits_t_lower() {
        let log = bootstrap_logger();
        let limits = MetricsLimits::new_normal(
            &log,
            MEAN,
            STD_DEV,
            NormalTestKind::T { freedom: FREEDOM },
            Some(*PERCENTILE),
            None,
        )
        .unwrap();
        assert_eq!(
            OrderedFloat::from(limits.baseline.unwrap()),
            OrderedFloat::from(MEAN)
        );
        assert_eq!(limits.lower, Some(MetricsLimit { value: -T_LIMIT }));
        assert_eq!(limits.upper, None);

        let side = limits.outlier(NORMAL_NEGATIVE_OUTLIER);
        assert_eq!(side, Some(BoundaryLimit::Lower));

        let side = limits.outlier(NORMAL_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_ZERO);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE_OUTLIER);
        assert_eq!(side, None);
    }

    #[test]
    fn test_limits_t_upper() {
        let log = bootstrap_logger();
        let limits = MetricsLimits::new_normal(
            &log,
            MEAN,
            STD_DEV,
            NormalTestKind::T { freedom: FREEDOM },
            None,
            Some(*PERCENTILE),
        )
        .unwrap();
        assert_eq!(
            OrderedFloat::from(limits.baseline.unwrap()),
            OrderedFloat::from(MEAN)
        );
        assert_eq!(limits.lower, None);
        assert_eq!(limits.upper, Some(MetricsLimit { value: T_LIMIT }));

        let side = limits.outlier(NORMAL_NEGATIVE_OUTLIER);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_ZERO);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE_OUTLIER);
        assert_eq!(side, Some(BoundaryLimit::Upper));
    }

    #[test]
    fn test_limits_t_both() {
        let log = bootstrap_logger();
        let limits = MetricsLimits::new_normal(
            &log,
            MEAN,
            STD_DEV,
            NormalTestKind::T { freedom: FREEDOM },
            Some(*PERCENTILE),
            Some(*PERCENTILE),
        )
        .unwrap();
        assert_eq!(
            OrderedFloat::from(limits.baseline.unwrap()),
            OrderedFloat::from(MEAN)
        );
        assert_eq!(limits.lower, Some(MetricsLimit { value: -T_LIMIT }));
        assert_eq!(limits.upper, Some(MetricsLimit { value: T_LIMIT }));

        let side = limits.outlier(NORMAL_NEGATIVE_OUTLIER);
        assert_eq!(side, Some(BoundaryLimit::Lower));

        let side = limits.outlier(NORMAL_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_ZERO);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE_OUTLIER);
        assert_eq!(side, Some(BoundaryLimit::Upper));
    }

    #[test]
    fn test_limits_t_docs() {
        const MEAN_100: f64 = 100.0;
        let log = bootstrap_logger();
        let boundary = 0.977.try_into().expect("Failed to create boundary.");
        let limits = MetricsLimits::new_normal(
            &log,
            MEAN_100,
            10.0,
            NormalTestKind::T {
                freedom: 25.0 - 1.0,
            },
            Some(boundary),
            Some(boundary),
        )
        .unwrap();
        assert_eq!(
            OrderedFloat::from(limits.baseline.unwrap()),
            OrderedFloat::from(MEAN_100)
        );
        assert_eq!(
            limits.lower,
            Some(MetricsLimit {
                value: 78.95585277295345
            })
        );
        assert_eq!(
            limits.upper,
            Some(MetricsLimit {
                value: 121.04414722704655
            })
        );

        let side = limits.outlier(75.0);
        assert_eq!(side, Some(BoundaryLimit::Lower));

        let side = limits.outlier(90.0);
        assert_eq!(side, None);

        let side = limits.outlier(100.0);
        assert_eq!(side, None);

        let side = limits.outlier(110.0);
        assert_eq!(side, None);

        let side = limits.outlier(125.0);
        assert_eq!(side, Some(BoundaryLimit::Upper));
    }

    #[test]
    fn test_limits_log_normal_none() {
        let log = bootstrap_logger();
        let ln = Ln::new(LOG_DATA).unwrap();
        let limits = MetricsLimits::new_log_normal(&log, ln, None, None).unwrap();
        assert_eq!(limits.baseline, None);
        assert_eq!(limits.lower, None);
        assert_eq!(limits.upper, None);

        let side = limits.outlier(LOG_NORMAL_NEGATIVE_OUTLIER);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_NEGATIVE_OUTLIER);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_ZERO);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE_OUTLIER);
        assert_eq!(side, None);

        let side = limits.outlier(LOG_NORMAL_POSITIVE_OUTLIER);
        assert_eq!(side, None);
    }

    #[test]
    fn test_limits_log_normal_lower() {
        let log = bootstrap_logger();
        let ln = Ln::new(LOG_DATA).unwrap();
        let limits = MetricsLimits::new_log_normal(&log, ln, Some(*PERCENTILE), None).unwrap();
        assert_eq!(
            OrderedFloat::from(limits.baseline.unwrap()),
            OrderedFloat::from(LOG_LOCATION)
        );
        assert_eq!(limits.lower, Some(MetricsLimit { value: LOG_LOWER }));
        assert_eq!(limits.upper, None);

        let side = limits.outlier(0.0);
        assert_eq!(side, Some(BoundaryLimit::Lower));

        let side = limits.outlier(3.0);
        assert_eq!(side, None);

        let side = limits.outlier(5.0);
        assert_eq!(side, None);
    }

    #[test]
    fn test_limits_log_normal_upper() {
        let log = bootstrap_logger();
        let ln = Ln::new(LOG_DATA).unwrap();
        let limits = MetricsLimits::new_log_normal(&log, ln, None, Some(*PERCENTILE)).unwrap();
        assert_eq!(
            OrderedFloat::from(limits.baseline.unwrap()),
            OrderedFloat::from(LOG_LOCATION)
        );
        assert_eq!(limits.lower, None);
        assert_eq!(limits.upper, Some(MetricsLimit { value: LOG_UPPER }));

        let side = limits.outlier(0.0);
        assert_eq!(side, None);

        let side = limits.outlier(3.0);
        assert_eq!(side, None);

        let side = limits.outlier(5.0);
        assert_eq!(side, Some(BoundaryLimit::Upper));
    }

    #[test]
    fn test_limits_long_normal_both() {
        let log = bootstrap_logger();
        let ln = Ln::new(LOG_DATA).unwrap();
        let limits =
            MetricsLimits::new_log_normal(&log, ln, Some(*PERCENTILE), Some(*PERCENTILE)).unwrap();
        assert_eq!(
            OrderedFloat::from(limits.baseline.unwrap()),
            OrderedFloat::from(LOG_LOCATION)
        );
        assert_eq!(limits.lower, Some(MetricsLimit { value: LOG_LOWER }));
        assert_eq!(limits.upper, Some(MetricsLimit { value: LOG_UPPER }));

        let side = limits.outlier(0.0);
        assert_eq!(side, Some(BoundaryLimit::Lower));

        let side = limits.outlier(3.0);
        assert_eq!(side, None);

        let side = limits.outlier(5.0);
        assert_eq!(side, Some(BoundaryLimit::Upper));
    }

    #[test]
    fn test_limits_log_normal_docs() {
        let log = bootstrap_logger();
        let ln = Ln::new(&[
            98.0, 99.0, 100.0, 101.0, 102.0, 98.0, 99.0, 100.0, 101.0, 102.0, 98.0, 99.0, 100.0,
            101.0, 102.0, 98.0, 99.0, 100.0, 101.0, 102.0, 98.0, 99.0, 100.0, 101.0, 102.0, 200.0,
        ])
        .unwrap();
        let boundary = 0.977.try_into().expect("Failed to create boundary.");
        let limits =
            MetricsLimits::new_log_normal(&log, ln, Some(boundary), Some(boundary)).unwrap();
        assert_eq!(
            OrderedFloat::from(limits.baseline.unwrap()),
            OrderedFloat::from(102.69192869301948)
        );
        assert_eq!(
            limits.lower,
            Some(MetricsLimit {
                value: 71.20122493974989
            })
        );
        assert_eq!(
            limits.upper,
            Some(MetricsLimit {
                value: 134.18263244628906
            })
        );

        let side = limits.outlier(70.0);
        assert_eq!(side, Some(BoundaryLimit::Lower));

        let side = limits.outlier(90.0);
        assert_eq!(side, None);

        let side = limits.outlier(100.0);
        assert_eq!(side, None);

        let side = limits.outlier(110.0);
        assert_eq!(side, None);

        let side = limits.outlier(140.0);
        assert_eq!(side, Some(BoundaryLimit::Upper));
    }

    #[test]
    fn test_limits_iqr_none() {
        let log = bootstrap_logger();
        let limits = MetricsLimits::new_iqr(&log, QUARTILES, None, None, None);
        assert_eq!(limits.baseline, None);
        assert_eq!(limits.lower, None);
        assert_eq!(limits.upper, None);

        let side = limits.outlier(NORMAL_NEGATIVE_OUTLIER);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_ZERO);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE_OUTLIER);
        assert_eq!(side, None);
    }

    #[test]
    fn test_limits_iqr_lower() {
        let log = bootstrap_logger();
        let limits = MetricsLimits::new_iqr(&log, QUARTILES, None, Some(*IQR_MULTIPLIER), None);
        assert_eq!(
            OrderedFloat::from(limits.baseline.unwrap()),
            OrderedFloat::from(IQR_Q2)
        );
        assert_eq!(
            limits.lower,
            Some(MetricsLimit {
                value: IQR_NEGATIVE_LIMIT
            })
        );
        assert_eq!(limits.upper, None);

        let side = limits.outlier(NORMAL_NEGATIVE_OUTLIER);
        assert_eq!(side, Some(BoundaryLimit::Lower));

        let side = limits.outlier(NORMAL_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_ZERO);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(IQR_POSITIVE_OUTLIER);
        assert_eq!(side, None);
    }

    #[test]
    fn test_limits_iqr_upper() {
        let log = bootstrap_logger();
        let limits = MetricsLimits::new_iqr(&log, QUARTILES, None, None, Some(*IQR_MULTIPLIER));
        assert_eq!(
            OrderedFloat::from(limits.baseline.unwrap()),
            OrderedFloat::from(IQR_Q2)
        );
        assert_eq!(limits.lower, None);
        assert_eq!(
            limits.upper,
            Some(MetricsLimit {
                value: IQR_POSITIVE_LIMIT
            })
        );

        let side = limits.outlier(NORMAL_NEGATIVE_OUTLIER);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_ZERO);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(IQR_POSITIVE_OUTLIER);
        assert_eq!(side, Some(BoundaryLimit::Upper));
    }

    #[test]
    fn test_limits_iqr_both() {
        let log = bootstrap_logger();
        let limits = MetricsLimits::new_iqr(
            &log,
            QUARTILES,
            None,
            Some(*IQR_MULTIPLIER),
            Some(*IQR_MULTIPLIER),
        );
        assert_eq!(
            OrderedFloat::from(limits.baseline.unwrap()),
            OrderedFloat::from(IQR_Q2)
        );
        assert_eq!(
            limits.lower,
            Some(MetricsLimit {
                value: IQR_NEGATIVE_LIMIT
            })
        );
        assert_eq!(
            limits.upper,
            Some(MetricsLimit {
                value: IQR_POSITIVE_LIMIT
            })
        );

        let side = limits.outlier(NORMAL_NEGATIVE_OUTLIER);
        assert_eq!(side, Some(BoundaryLimit::Lower));

        let side = limits.outlier(NORMAL_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_ZERO);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(IQR_POSITIVE_OUTLIER);
        assert_eq!(side, Some(BoundaryLimit::Upper));
    }

    #[test]
    fn test_limits_delta_iqr_none() {
        let log = bootstrap_logger();
        let limits = MetricsLimits::new_iqr(&log, QUARTILES, Some(DELTA_QUARTILES), None, None);
        assert_eq!(limits.baseline, None);
        assert_eq!(limits.lower, None);
        assert_eq!(limits.upper, None);

        let side = limits.outlier(NORMAL_NEGATIVE_OUTLIER);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_ZERO);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE_OUTLIER);
        assert_eq!(side, None);
    }

    #[test]
    fn test_limits_delta_iqr_lower() {
        let log = bootstrap_logger();
        let limits = MetricsLimits::new_iqr(
            &log,
            QUARTILES,
            Some(DELTA_QUARTILES),
            Some(*IQR_MULTIPLIER),
            None,
        );
        assert_eq!(
            OrderedFloat::from(limits.baseline.unwrap()),
            OrderedFloat::from(IQR_Q2)
        );
        assert_eq!(
            limits.lower,
            Some(MetricsLimit {
                value: IQR_NEGATIVE_LIMIT
            })
        );
        assert_eq!(limits.upper, None);

        let side = limits.outlier(NORMAL_NEGATIVE_OUTLIER);
        assert_eq!(side, Some(BoundaryLimit::Lower));

        let side = limits.outlier(NORMAL_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_ZERO);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(IQR_POSITIVE_OUTLIER);
        assert_eq!(side, None);
    }

    #[test]
    fn test_limits_delta_iqr_upper() {
        let log = bootstrap_logger();
        let limits = MetricsLimits::new_iqr(
            &log,
            QUARTILES,
            Some(DELTA_QUARTILES),
            None,
            Some(*IQR_MULTIPLIER),
        );
        assert_eq!(
            OrderedFloat::from(limits.baseline.unwrap()),
            OrderedFloat::from(IQR_Q2)
        );
        assert_eq!(limits.lower, None);
        assert_eq!(
            limits.upper,
            Some(MetricsLimit {
                value: IQR_POSITIVE_LIMIT
            })
        );

        let side = limits.outlier(NORMAL_NEGATIVE_OUTLIER);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_ZERO);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(IQR_POSITIVE_OUTLIER);
        assert_eq!(side, Some(BoundaryLimit::Upper));
    }

    #[test]
    fn test_limits_delta_iqr_both() {
        let log = bootstrap_logger();
        let limits = MetricsLimits::new_iqr(
            &log,
            QUARTILES,
            Some(DELTA_QUARTILES),
            Some(*IQR_MULTIPLIER),
            Some(*IQR_MULTIPLIER),
        );
        assert_eq!(
            OrderedFloat::from(limits.baseline.unwrap()),
            OrderedFloat::from(IQR_Q2)
        );
        assert_eq!(
            limits.lower,
            Some(MetricsLimit {
                value: IQR_NEGATIVE_LIMIT
            })
        );
        assert_eq!(
            limits.upper,
            Some(MetricsLimit {
                value: IQR_POSITIVE_LIMIT
            })
        );

        let side = limits.outlier(NORMAL_NEGATIVE_OUTLIER);
        assert_eq!(side, Some(BoundaryLimit::Lower));

        let side = limits.outlier(NORMAL_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_ZERO);
        assert_eq!(side, None);

        let side = limits.outlier(NORMAL_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(IQR_POSITIVE_OUTLIER);
        assert_eq!(side, Some(BoundaryLimit::Upper));
    }

    #[test]
    fn test_limits_delta_iqr_docs() {
        let log = bootstrap_logger();
        let quartiles = Quartiles {
            q1: 95.0,
            q2: 100.0,
            q3: 105.0,
        };
        let delta_quartiles = Quartiles {
            q1: 0.1,
            q2: 0.2,
            q3: 0.3,
        };
        let multiplier = IqrBoundary::try_from(2.0).unwrap();
        let limits = MetricsLimits::new_iqr(
            &log,
            quartiles,
            Some(delta_quartiles),
            Some(multiplier),
            Some(multiplier),
        );
        assert_eq!(
            OrderedFloat::from(limits.baseline.unwrap()),
            OrderedFloat::from(100.0)
        );
        assert_eq!(limits.lower, Some(MetricsLimit { value: 60.0 }));
        assert_eq!(limits.upper, Some(MetricsLimit { value: 140.0 }));

        let side = limits.outlier(50.0);
        assert_eq!(side, Some(BoundaryLimit::Lower));

        let side = limits.outlier(100.0);
        assert_eq!(side, None);

        let side = limits.outlier(150.0);
        assert_eq!(side, Some(BoundaryLimit::Upper));
    }
}
