use bencher_json::Boundary;
use slog::{debug, Logger};
use statrs::distribution::{ContinuousCDF, Normal, StudentsT};

use crate::{error::api_error, model::project::threshold::alert::Limit, ApiError};

#[derive(Default)]
pub struct MetricsLimits {
    pub lower: Option<MetricsLimit>,
    pub upper: Option<MetricsLimit>,
}

#[derive(Debug, PartialEq)]
pub struct MetricsLimit {
    pub value: f64,
}

#[derive(Clone, Copy)]
pub enum TestKind {
    Z,
    T { freedom: f64 },
}

impl MetricsLimits {
    pub fn new(
        log: &Logger,
        mean: f64,
        std_dev: f64,
        test_kind: TestKind,
        lower_boundary: Option<Boundary>,
        upper_boundary: Option<Boundary>,
    ) -> Result<Self, ApiError> {
        if lower_boundary.is_none() && upper_boundary.is_none() {
            return Ok(Self::default());
        }

        Ok(match test_kind {
            // Create a normal distribution and calculate the boundary limits for the threshold based on the boundary percentiles.
            TestKind::Z => {
                debug!(log, "Normal distribution: mean={mean}, std_dev={std_dev}");
                let normal = Normal::new(mean, std_dev).map_err(api_error!())?;
                let lower = lower_boundary.map(|limit| {
                    let abs_limit = normal.inverse_cdf(limit.into());
                    MetricsLimit::lower(mean, abs_limit)
                });
                let upper = upper_boundary.map(|limit| {
                    let abs_limit = normal.inverse_cdf(limit.into());
                    MetricsLimit::upper(abs_limit)
                });
                Self { lower, upper }
            },
            // Create a Student's t distribution and calculate the boundary limits for the threshold based on the boundary percentiles.
            TestKind::T { freedom } => {
                debug!(
                    log,
                    "Students T distribution: mean={mean}, std_dev={std_dev}, freedom={freedom}"
                );
                let students_t = StudentsT::new(mean, std_dev, freedom).map_err(api_error!())?;
                let lower = lower_boundary.map(|limit| {
                    let abs_limit = students_t.inverse_cdf(limit.into());
                    MetricsLimit::lower(mean, abs_limit)
                });
                let upper = upper_boundary.map(|limit| {
                    let abs_limit = students_t.inverse_cdf(limit.into());
                    MetricsLimit::upper(abs_limit)
                });
                Self { lower, upper }
            },
        })
    }

    // An outlier occurs when the  datum exceeds a boundary limit.
    pub fn outlier(&self, datum: f64) -> Option<Limit> {
        match (self.lower.as_ref(), self.upper.as_ref()) {
            (Some(lower), Some(upper)) => {
                if datum < lower.value {
                    Some(Limit::Lower)
                } else if datum > upper.value {
                    Some(Limit::Upper)
                } else {
                    None
                }
            },
            (Some(lower), None) => (datum < lower.value).then_some(Limit::Lower),
            (None, Some(upper)) => (datum > upper.value).then_some(Limit::Upper),
            (None, None) => None,
        }
    }
}

impl MetricsLimit {
    // Flip the absolute limit to the other side of the mean, creating the actual boundary limit.
    fn lower(mean: f64, abs_limit: f64) -> Self {
        Self {
            value: mean * 2.0 - abs_limit,
        }
    }

    fn upper(abs_limit: f64) -> Self {
        Self { value: abs_limit }
    }
}

impl From<MetricsLimit> for f64 {
    fn from(limit: MetricsLimit) -> Self {
        limit.value
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unreadable_literal, clippy::unwrap_used)]
mod test {
    use bencher_json::Boundary;
    use once_cell::sync::Lazy;
    use pretty_assertions::assert_eq;

    use crate::util::logger::bootstrap_logger;

    use super::{Limit, MetricsLimit, MetricsLimits, TestKind};

    const MEAN: f64 = 0.0;
    const STD_DEV: f64 = 1.0;
    const FREEDOM: f64 = 5.0;

    static PERCENTILE: Lazy<Boundary> = Lazy::new(|| 0.85.try_into().expect("Failed to boundary."));
    const Z_LIMIT: f64 = 1.0364333894937896;
    const T_LIMIT: f64 = 1.1557673428942912;

    const DATUM_NEGATIVE_OUTLIER: f64 = -1.5;
    const DATUM_NEGATIVE: f64 = -1.0;
    const DATUM_ZERO: f64 = 0.0;
    const DATUM_POSITIVE: f64 = 1.0;
    const DATUM_POSITIVE_OUTLIER: f64 = 1.5;

    #[test]
    fn test_limits_z_none() {
        let log = bootstrap_logger();
        let limits = MetricsLimits::new(&log, MEAN, STD_DEV, TestKind::Z, None, None).unwrap();
        assert_eq!(limits.lower, None);
        assert_eq!(limits.upper, None);

        let side = limits.outlier(DATUM_NEGATIVE_OUTLIER);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_ZERO);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_POSITIVE_OUTLIER);
        assert_eq!(side, None);
    }

    #[test]
    fn test_limits_z_left() {
        let log = bootstrap_logger();
        let limits =
            MetricsLimits::new(&log, MEAN, STD_DEV, TestKind::Z, Some(*PERCENTILE), None).unwrap();
        assert_eq!(limits.lower, Some(MetricsLimit { value: -Z_LIMIT }));
        assert_eq!(limits.upper, None);

        let side = limits.outlier(DATUM_NEGATIVE_OUTLIER);
        assert_eq!(side, Some(Limit::Lower));

        let side = limits.outlier(DATUM_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_ZERO);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_POSITIVE_OUTLIER);
        assert_eq!(side, None);
    }

    #[test]
    fn test_limits_z_right() {
        let log = bootstrap_logger();
        let limits =
            MetricsLimits::new(&log, MEAN, STD_DEV, TestKind::Z, None, Some(*PERCENTILE)).unwrap();
        assert_eq!(limits.lower, None);
        assert_eq!(limits.upper, Some(MetricsLimit { value: Z_LIMIT }));

        let side = limits.outlier(DATUM_NEGATIVE_OUTLIER);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_ZERO);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_POSITIVE_OUTLIER);
        assert_eq!(side, Some(Limit::Upper));
    }

    #[test]
    fn test_limits_z_both() {
        let log = bootstrap_logger();
        let limits = MetricsLimits::new(
            &log,
            MEAN,
            STD_DEV,
            TestKind::Z,
            Some(*PERCENTILE),
            Some(*PERCENTILE),
        )
        .unwrap();
        assert_eq!(limits.lower, Some(MetricsLimit { value: -Z_LIMIT }));
        assert_eq!(limits.upper, Some(MetricsLimit { value: Z_LIMIT }));

        let side = limits.outlier(DATUM_NEGATIVE_OUTLIER);
        assert_eq!(side, Some(Limit::Lower));

        let side = limits.outlier(DATUM_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_ZERO);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_POSITIVE_OUTLIER);
        assert_eq!(side, Some(Limit::Upper));
    }

    #[test]
    fn test_limits_z_docs() {
        let log = bootstrap_logger();
        let boundary = 0.977.try_into().expect("Failed to create boundary.");
        let limits = MetricsLimits::new(
            &log,
            100.0,
            10.0,
            TestKind::Z,
            Some(boundary),
            Some(boundary),
        )
        .unwrap();
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
        assert_eq!(side, Some(Limit::Lower));

        let side = limits.outlier(90.0);
        assert_eq!(side, None);

        let side = limits.outlier(100.0);
        assert_eq!(side, None);

        let side = limits.outlier(110.0);
        assert_eq!(side, None);

        let side = limits.outlier(125.0);
        assert_eq!(side, Some(Limit::Upper));
    }

    #[test]
    fn test_limits_t_none() {
        let log = bootstrap_logger();
        let limits = MetricsLimits::new(
            &log,
            MEAN,
            STD_DEV,
            TestKind::T { freedom: FREEDOM },
            None,
            None,
        )
        .unwrap();
        assert_eq!(limits.lower, None);
        assert_eq!(limits.upper, None);

        let side = limits.outlier(DATUM_NEGATIVE_OUTLIER);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_ZERO);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_POSITIVE_OUTLIER);
        assert_eq!(side, None);
    }

    #[test]
    fn test_limits_t_left() {
        let log = bootstrap_logger();
        let limits = MetricsLimits::new(
            &log,
            MEAN,
            STD_DEV,
            TestKind::T { freedom: FREEDOM },
            Some(*PERCENTILE),
            None,
        )
        .unwrap();
        assert_eq!(limits.lower, Some(MetricsLimit { value: -T_LIMIT }));
        assert_eq!(limits.upper, None);

        let side = limits.outlier(DATUM_NEGATIVE_OUTLIER);
        assert_eq!(side, Some(Limit::Lower));

        let side = limits.outlier(DATUM_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_ZERO);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_POSITIVE_OUTLIER);
        assert_eq!(side, None);
    }

    #[test]
    fn test_limits_t_right() {
        let log = bootstrap_logger();
        let limits = MetricsLimits::new(
            &log,
            MEAN,
            STD_DEV,
            TestKind::T { freedom: FREEDOM },
            None,
            Some(*PERCENTILE),
        )
        .unwrap();
        assert_eq!(limits.lower, None);
        assert_eq!(limits.upper, Some(MetricsLimit { value: T_LIMIT }));

        let side = limits.outlier(DATUM_NEGATIVE_OUTLIER);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_ZERO);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_POSITIVE_OUTLIER);
        assert_eq!(side, Some(Limit::Upper));
    }

    #[test]
    fn test_limits_t_both() {
        let log = bootstrap_logger();
        let limits = MetricsLimits::new(
            &log,
            MEAN,
            STD_DEV,
            TestKind::T { freedom: FREEDOM },
            Some(*PERCENTILE),
            Some(*PERCENTILE),
        )
        .unwrap();
        assert_eq!(limits.lower, Some(MetricsLimit { value: -T_LIMIT }));
        assert_eq!(limits.upper, Some(MetricsLimit { value: T_LIMIT }));

        let side = limits.outlier(DATUM_NEGATIVE_OUTLIER);
        assert_eq!(side, Some(Limit::Lower));

        let side = limits.outlier(DATUM_NEGATIVE);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_ZERO);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_POSITIVE);
        assert_eq!(side, None);

        let side = limits.outlier(DATUM_POSITIVE_OUTLIER);
        assert_eq!(side, Some(Limit::Upper));
    }
}
