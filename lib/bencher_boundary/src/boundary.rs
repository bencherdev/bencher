use bencher_json::{Boundary, ModelTest, SampleSize, project::boundary::BoundaryLimit};
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
                #[expect(
                    clippy::cast_precision_loss,
                    reason = "sample size as f64 is fine for stats"
                )]
                NormalTestKind::T {
                    sample_size: data.len() as f64,
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

#[cfg(test)]
#[expect(
    clippy::unreadable_literal,
    reason = "float literals are test expected values"
)]
mod tests {
    use std::sync::LazyLock;

    use bencher_json::{Boundary, ModelTest, SampleSize, project::boundary::BoundaryLimit};
    use bencher_logger::bootstrap_logger;
    use ordered_float::OrderedFloat;
    use pretty_assertions::assert_eq;

    use crate::{BoundaryError, MetricsData};

    use super::MetricsBoundary;

    // All model test variants, for behavior that should be uniform across them.
    const ALL_MODEL_TESTS: &[ModelTest] = &[
        ModelTest::Static,
        ModelTest::Percentage,
        ModelTest::ZScore,
        ModelTest::TTest,
        ModelTest::LogNormal,
        ModelTest::Iqr,
        ModelTest::DeltaIqr,
    ];

    const DATA_EMPTY: &[f64] = &[];
    // mean 3.0 | sample std dev sqrt(2.5) | quartiles (2.0, 3.0, 4.0)
    const DATA_FIVE: &[f64] = &[1.0, 2.0, 3.0, 4.0, 5.0];
    // mean 0.0 | sample std dev exactly 1.0
    const DATA_NORMAL: &[f64] = &[-1.0, -1.0, 0.0, 1.0, 1.0];
    // Constant data: sample variance is zero
    const DATA_CONST: &[f64] = &[1.0, 1.0, 1.0, 1.0, 1.0];
    // All-zero data: mean (percentage baseline) is zero
    const DATA_ZEROES: &[f64] = &[0.0, 0.0, 0.0, 0.0, 0.0];
    // Non-positive data points: natural log is undefined (zero) or NaN (negative)
    const DATA_WITH_ZERO: &[f64] = &[1.0, 2.0, 0.0, 4.0, 5.0];
    const DATA_NEGATIVE: &[f64] = &[-1.0, -2.0, -3.0, -4.0, -5.0];
    const DATA_SINGLE: &[f64] = &[3.0];

    static STATIC_LOWER: LazyLock<Boundary> = LazyLock::new(|| boundary(-5.0));
    static STATIC_UPPER: LazyLock<Boundary> = LazyLock::new(|| boundary(5.0));
    // 50% above and below the mean
    static PERCENTAGE_BOUNDARY: LazyLock<Boundary> = LazyLock::new(|| boundary(0.5));
    // 85th percentile of the cumulative distribution function
    static CDF_BOUNDARY: LazyLock<Boundary> = LazyLock::new(|| boundary(0.85));
    // 1.5x the inter-quartile range
    static IQR_BOUNDARY: LazyLock<Boundary> = LazyLock::new(|| boundary(1.5));
    // A valid `Boundary` for all model tests: percentage, CDF, and IQR
    static UNIVERSAL_BOUNDARY: LazyLock<Boundary> = LazyLock::new(|| boundary(0.85));
    // A valid `Boundary` that is invalid for percentage, CDF, and IQR model tests
    static NEGATIVE_BOUNDARY: LazyLock<Boundary> = LazyLock::new(|| boundary(-0.5));
    // A valid `Boundary` that is invalid as a CDF boundary (must be at least 0.5)
    static LOW_CDF_BOUNDARY: LazyLock<Boundary> = LazyLock::new(|| boundary(0.3));

    static MIN_SAMPLE_SIZE_FIVE: LazyLock<SampleSize> = LazyLock::new(|| sample_size(5));
    static MIN_SAMPLE_SIZE_SIX: LazyLock<SampleSize> = LazyLock::new(|| sample_size(6));

    // Percentage: mean 3.0 +/- 50%
    const PERCENTAGE_BASELINE: f64 = 3.0;
    const PERCENTAGE_LOWER_LIMIT: f64 = 1.5;
    const PERCENTAGE_UPPER_LIMIT: f64 = 4.5;

    // ZScore: mean 0.0, std dev 1.0, so the limits are +/- the 85th percentile z-score
    const Z_BASELINE: f64 = 0.0;
    const Z_LIMIT: f64 = 1.0364333894937896;

    // TTest: mean 0.0, std dev 1.0, sample size 5 (freedom 4)
    // limit = t-quantile(0.85, freedom = 4) * sqrt(1 + 1/5) = 1.1895668524436934 * sqrt(1.2)
    const T_BASELINE: f64 = 0.0;
    const T_LIMIT: f64 = 1.3031051974876533;

    // LogNormal over DATA_FIVE: location 0.9574983485564091, scale 0.6355094387463041
    // baseline = e^location
    // upper = e^(location + z(0.85) * scale)
    // lower = 2 * baseline - upper (flipped to the other side of the baseline)
    const LOG_NORMAL_BASELINE: f64 = 2.6051710846973517;
    const LOG_NORMAL_LOWER_LIMIT: f64 = 0.17661070852081817;
    const LOG_NORMAL_UPPER_LIMIT: f64 = 5.033731460873885;

    // IQR over DATA_FIVE: q2 3.0 -/+ (q3 4.0 - q1 2.0) * 1.5
    const IQR_BASELINE: f64 = 3.0;
    const IQR_LOWER_LIMIT: f64 = 0.0;
    const IQR_UPPER_LIMIT: f64 = 6.0;

    // DeltaIQR over DATA_FIVE: delta quartiles (0.3125, 0.41666..., 0.625)
    // q2 3.0 -/+ q2 3.0 * (0.625 - 0.3125) * 1.5 = 3.0 -/+ 1.40625
    const DELTA_IQR_BASELINE: f64 = 3.0;
    const DELTA_IQR_LOWER_LIMIT: f64 = 1.59375;
    const DELTA_IQR_UPPER_LIMIT: f64 = 4.40625;

    fn boundary(value: f64) -> Boundary {
        value.try_into().expect("Failed to parse boundary.")
    }

    fn sample_size(value: u32) -> SampleSize {
        value.try_into().expect("Failed to parse sample size.")
    }

    fn new_boundary(
        datum: f64,
        data: &[f64],
        model_test: ModelTest,
        min_sample_size: Option<SampleSize>,
        lower_boundary: Option<Boundary>,
        upper_boundary: Option<Boundary>,
    ) -> Result<MetricsBoundary, BoundaryError> {
        let log = bootstrap_logger();
        let metrics_data = MetricsData {
            data: data.to_vec(),
        };
        MetricsBoundary::new(
            &log,
            datum,
            &metrics_data,
            model_test,
            min_sample_size,
            lower_boundary,
            upper_boundary,
        )
    }

    fn outlier_for(
        datum: f64,
        data: &[f64],
        model_test: ModelTest,
        lower_boundary: Option<Boundary>,
        upper_boundary: Option<Boundary>,
    ) -> Option<BoundaryLimit> {
        new_boundary(
            datum,
            data,
            model_test,
            None,
            lower_boundary,
            upper_boundary,
        )
        .expect("Failed to create metrics boundary.")
        .outlier
    }

    fn assert_limits(
        metrics_boundary: &MetricsBoundary,
        baseline: Option<f64>,
        lower: Option<f64>,
        upper: Option<f64>,
    ) {
        let limits = &metrics_boundary.limits;
        assert_eq!(
            limits.baseline.map(OrderedFloat::from),
            baseline.map(OrderedFloat::from)
        );
        assert_eq!(
            limits
                .lower
                .as_ref()
                .map(|limit| OrderedFloat::from(limit.value)),
            lower.map(OrderedFloat::from)
        );
        assert_eq!(
            limits
                .upper
                .as_ref()
                .map(|limit| OrderedFloat::from(limit.value)),
            upper.map(OrderedFloat::from)
        );
    }

    fn assert_no_boundary(metrics_boundary: &MetricsBoundary) {
        assert_limits(metrics_boundary, None, None, None);
        assert_eq!(metrics_boundary.outlier, None);
    }

    // If neither boundary is set, then no boundary is calculated for any model test.
    #[test]
    fn boundary_none_configured() {
        for &model_test in ALL_MODEL_TESTS {
            let metrics_boundary = new_boundary(1_000.0, DATA_FIVE, model_test, None, None, None)
                .expect("Failed to create metrics boundary.");
            assert_no_boundary(&metrics_boundary);
        }
    }

    // If there is no historical data, then no boundary is calculated for any model test.
    // Note that this is true even for `Static`, which does not otherwise use the data.
    #[test]
    fn boundary_empty_data() {
        for &model_test in ALL_MODEL_TESTS {
            let metrics_boundary = new_boundary(
                1_000.0,
                DATA_EMPTY,
                model_test,
                None,
                Some(*UNIVERSAL_BOUNDARY),
                Some(*UNIVERSAL_BOUNDARY),
            )
            .expect("Failed to create metrics boundary.");
            assert_no_boundary(&metrics_boundary);
        }
    }

    // If the min sample size is not met, then no boundary is calculated for any model test.
    #[test]
    fn boundary_min_sample_size_not_met() {
        for &model_test in ALL_MODEL_TESTS {
            let metrics_boundary = new_boundary(
                1_000.0,
                DATA_FIVE,
                model_test,
                Some(*MIN_SAMPLE_SIZE_SIX),
                Some(*UNIVERSAL_BOUNDARY),
                Some(*UNIVERSAL_BOUNDARY),
            )
            .expect("Failed to create metrics boundary.");
            assert_no_boundary(&metrics_boundary);
        }
    }

    // If the min sample size is exactly met, then a boundary is calculated for every model test.
    #[test]
    fn boundary_min_sample_size_met() {
        for &model_test in ALL_MODEL_TESTS {
            let metrics_boundary = new_boundary(
                3.0,
                DATA_FIVE,
                model_test,
                Some(*MIN_SAMPLE_SIZE_FIVE),
                None,
                Some(*UNIVERSAL_BOUNDARY),
            )
            .expect("Failed to create metrics boundary.");
            assert!(
                metrics_boundary.limits.upper.is_some(),
                "Expected an upper limit for {model_test:?}"
            );
        }
    }

    #[test]
    fn boundary_static() {
        let metrics_boundary = new_boundary(
            1.0,
            DATA_FIVE,
            ModelTest::Static,
            None,
            Some(*STATIC_LOWER),
            Some(*STATIC_UPPER),
        )
        .expect("Failed to create metrics boundary.");
        // Static limits are the boundary values themselves, and there is no baseline.
        assert_limits(&metrics_boundary, None, Some(-5.0), Some(5.0));
        assert_eq!(metrics_boundary.outlier, None);

        let lower = Some(*STATIC_LOWER);
        let upper = Some(*STATIC_UPPER);

        // The limits themselves are inclusive: an outlier must exceed the limit.
        let side = outlier_for(5.0, DATA_FIVE, ModelTest::Static, lower, upper);
        assert_eq!(side, None);
        let side = outlier_for(-5.0, DATA_FIVE, ModelTest::Static, lower, upper);
        assert_eq!(side, None);

        let side = outlier_for(5.1, DATA_FIVE, ModelTest::Static, lower, upper);
        assert_eq!(side, Some(BoundaryLimit::Upper));
        let side = outlier_for(-5.1, DATA_FIVE, ModelTest::Static, lower, upper);
        assert_eq!(side, Some(BoundaryLimit::Lower));
    }

    #[test]
    fn boundary_static_single_sided() {
        let metrics_boundary = new_boundary(
            -10.0,
            DATA_FIVE,
            ModelTest::Static,
            None,
            None,
            Some(*STATIC_UPPER),
        )
        .expect("Failed to create metrics boundary.");
        assert_limits(&metrics_boundary, None, None, Some(5.0));
        // Without a lower boundary, a very low datum is not an outlier.
        assert_eq!(metrics_boundary.outlier, None);

        let metrics_boundary = new_boundary(
            10.0,
            DATA_FIVE,
            ModelTest::Static,
            None,
            Some(*STATIC_LOWER),
            None,
        )
        .expect("Failed to create metrics boundary.");
        assert_limits(&metrics_boundary, None, Some(-5.0), None);
        // Without an upper boundary, a very high datum is not an outlier.
        assert_eq!(metrics_boundary.outlier, None);
    }

    #[test]
    fn boundary_percentage() {
        let metrics_boundary = new_boundary(
            3.0,
            DATA_FIVE,
            ModelTest::Percentage,
            None,
            Some(*PERCENTAGE_BOUNDARY),
            Some(*PERCENTAGE_BOUNDARY),
        )
        .expect("Failed to create metrics boundary.");
        assert_limits(
            &metrics_boundary,
            Some(PERCENTAGE_BASELINE),
            Some(PERCENTAGE_LOWER_LIMIT),
            Some(PERCENTAGE_UPPER_LIMIT),
        );
        assert_eq!(metrics_boundary.outlier, None);

        let lower = Some(*PERCENTAGE_BOUNDARY);
        let upper = Some(*PERCENTAGE_BOUNDARY);

        // The limits themselves are inclusive.
        let side = outlier_for(1.5, DATA_FIVE, ModelTest::Percentage, lower, upper);
        assert_eq!(side, None);
        let side = outlier_for(4.5, DATA_FIVE, ModelTest::Percentage, lower, upper);
        assert_eq!(side, None);

        let side = outlier_for(1.4, DATA_FIVE, ModelTest::Percentage, lower, upper);
        assert_eq!(side, Some(BoundaryLimit::Lower));
        let side = outlier_for(4.6, DATA_FIVE, ModelTest::Percentage, lower, upper);
        assert_eq!(side, Some(BoundaryLimit::Upper));
    }

    // With a zero baseline, a percentage boundary degenerates to both limits at zero:
    // any nonzero datum is an outlier.
    #[test]
    fn boundary_percentage_zero_baseline() {
        let metrics_boundary = new_boundary(
            0.0,
            DATA_ZEROES,
            ModelTest::Percentage,
            None,
            Some(*PERCENTAGE_BOUNDARY),
            Some(*PERCENTAGE_BOUNDARY),
        )
        .expect("Failed to create metrics boundary.");
        assert_limits(&metrics_boundary, Some(0.0), Some(0.0), Some(0.0));
        assert_eq!(metrics_boundary.outlier, None);

        let lower = Some(*PERCENTAGE_BOUNDARY);
        let upper = Some(*PERCENTAGE_BOUNDARY);

        let side = outlier_for(0.1, DATA_ZEROES, ModelTest::Percentage, lower, upper);
        assert_eq!(side, Some(BoundaryLimit::Upper));
        let side = outlier_for(-0.1, DATA_ZEROES, ModelTest::Percentage, lower, upper);
        assert_eq!(side, Some(BoundaryLimit::Lower));
    }

    // A negative boundary is a valid `Boundary` but an invalid `PercentageBoundary`.
    #[test]
    fn boundary_percentage_invalid_boundary() {
        let result = new_boundary(
            3.0,
            DATA_FIVE,
            ModelTest::Percentage,
            None,
            Some(*NEGATIVE_BOUNDARY),
            None,
        );
        assert!(matches!(result, Err(BoundaryError::Valid(_))));
    }

    #[test]
    fn boundary_z_score() {
        let metrics_boundary = new_boundary(
            0.0,
            DATA_NORMAL,
            ModelTest::ZScore,
            None,
            Some(*CDF_BOUNDARY),
            Some(*CDF_BOUNDARY),
        )
        .expect("Failed to create metrics boundary.");
        assert_limits(
            &metrics_boundary,
            Some(Z_BASELINE),
            Some(-Z_LIMIT),
            Some(Z_LIMIT),
        );
        assert_eq!(metrics_boundary.outlier, None);

        let lower = Some(*CDF_BOUNDARY);
        let upper = Some(*CDF_BOUNDARY);

        let side = outlier_for(1.0, DATA_NORMAL, ModelTest::ZScore, lower, upper);
        assert_eq!(side, None);
        let side = outlier_for(-1.0, DATA_NORMAL, ModelTest::ZScore, lower, upper);
        assert_eq!(side, None);

        let side = outlier_for(1.5, DATA_NORMAL, ModelTest::ZScore, lower, upper);
        assert_eq!(side, Some(BoundaryLimit::Upper));
        let side = outlier_for(-1.5, DATA_NORMAL, ModelTest::ZScore, lower, upper);
        assert_eq!(side, Some(BoundaryLimit::Lower));
    }

    // Constant data has zero variance, so no boundary can be calculated:
    // even a wildly different datum is not detected as an outlier.
    #[test]
    fn boundary_z_score_constant_data() {
        let metrics_boundary = new_boundary(
            1_000.0,
            DATA_CONST,
            ModelTest::ZScore,
            None,
            Some(*CDF_BOUNDARY),
            Some(*CDF_BOUNDARY),
        )
        .expect("Failed to create metrics boundary.");
        assert_no_boundary(&metrics_boundary);
    }

    // A CDF boundary must be at least 0.5, so 0.3 is a valid `Boundary` but invalid here.
    #[test]
    fn boundary_z_score_invalid_boundary() {
        let result = new_boundary(
            0.0,
            DATA_NORMAL,
            ModelTest::ZScore,
            None,
            None,
            Some(*LOW_CDF_BOUNDARY),
        );
        assert!(matches!(result, Err(BoundaryError::Valid(_))));
    }

    #[test]
    fn boundary_t_test() {
        let metrics_boundary = new_boundary(
            0.0,
            DATA_NORMAL,
            ModelTest::TTest,
            None,
            Some(*CDF_BOUNDARY),
            Some(*CDF_BOUNDARY),
        )
        .expect("Failed to create metrics boundary.");
        assert_limits(
            &metrics_boundary,
            Some(T_BASELINE),
            Some(-T_LIMIT),
            Some(T_LIMIT),
        );
        assert_eq!(metrics_boundary.outlier, None);

        let lower = Some(*CDF_BOUNDARY);
        let upper = Some(*CDF_BOUNDARY);

        let side = outlier_for(1.3, DATA_NORMAL, ModelTest::TTest, lower, upper);
        assert_eq!(side, None);
        let side = outlier_for(-1.3, DATA_NORMAL, ModelTest::TTest, lower, upper);
        assert_eq!(side, None);

        let side = outlier_for(1.4, DATA_NORMAL, ModelTest::TTest, lower, upper);
        assert_eq!(side, Some(BoundaryLimit::Upper));
        let side = outlier_for(-1.4, DATA_NORMAL, ModelTest::TTest, lower, upper);
        assert_eq!(side, Some(BoundaryLimit::Lower));
    }

    // At a small sample size, the t-test prediction interval must be wider than
    // the z-score interval for the same data and percentile.
    #[test]
    fn boundary_t_test_wider_than_z_score() {
        let t_boundary = new_boundary(
            0.0,
            DATA_NORMAL,
            ModelTest::TTest,
            None,
            Some(*CDF_BOUNDARY),
            Some(*CDF_BOUNDARY),
        )
        .expect("Failed to create metrics boundary.");
        let z_boundary = new_boundary(
            0.0,
            DATA_NORMAL,
            ModelTest::ZScore,
            None,
            Some(*CDF_BOUNDARY),
            Some(*CDF_BOUNDARY),
        )
        .expect("Failed to create metrics boundary.");

        let t_lower = t_boundary
            .limits
            .lower
            .expect("Missing t lower limit")
            .value;
        let t_upper = t_boundary
            .limits
            .upper
            .expect("Missing t upper limit")
            .value;
        let z_lower = z_boundary
            .limits
            .lower
            .expect("Missing z lower limit")
            .value;
        let z_upper = z_boundary
            .limits
            .upper
            .expect("Missing z upper limit")
            .value;

        assert!(
            t_upper > z_upper,
            "t upper limit ({t_upper}) must exceed z upper limit ({z_upper})"
        );
        assert!(
            t_lower < z_lower,
            "t lower limit ({t_lower}) must fall below z lower limit ({z_lower})"
        );
    }

    // Constant data has zero variance, so no boundary can be calculated.
    #[test]
    fn boundary_t_test_constant_data() {
        let metrics_boundary = new_boundary(
            1_000.0,
            DATA_CONST,
            ModelTest::TTest,
            None,
            Some(*CDF_BOUNDARY),
            Some(*CDF_BOUNDARY),
        )
        .expect("Failed to create metrics boundary.");
        assert_no_boundary(&metrics_boundary);
    }

    #[test]
    fn boundary_log_normal() {
        let metrics_boundary = new_boundary(
            3.0,
            DATA_FIVE,
            ModelTest::LogNormal,
            None,
            Some(*CDF_BOUNDARY),
            Some(*CDF_BOUNDARY),
        )
        .expect("Failed to create metrics boundary.");
        assert_limits(
            &metrics_boundary,
            Some(LOG_NORMAL_BASELINE),
            Some(LOG_NORMAL_LOWER_LIMIT),
            Some(LOG_NORMAL_UPPER_LIMIT),
        );
        assert_eq!(metrics_boundary.outlier, None);

        let lower = Some(*CDF_BOUNDARY);
        let upper = Some(*CDF_BOUNDARY);

        let side = outlier_for(0.2, DATA_FIVE, ModelTest::LogNormal, lower, upper);
        assert_eq!(side, None);
        let side = outlier_for(5.0, DATA_FIVE, ModelTest::LogNormal, lower, upper);
        assert_eq!(side, None);

        let side = outlier_for(6.0, DATA_FIVE, ModelTest::LogNormal, lower, upper);
        assert_eq!(side, Some(BoundaryLimit::Upper));
        let side = outlier_for(0.1, DATA_FIVE, ModelTest::LogNormal, lower, upper);
        assert_eq!(side, Some(BoundaryLimit::Lower));
    }

    // The natural log of zero is negative infinity and the natural log of a negative
    // number is NaN, so `Ln` returns `None` and no boundary is calculated.
    #[test]
    fn boundary_log_normal_non_positive_data() {
        for data in [DATA_WITH_ZERO, DATA_NEGATIVE] {
            let metrics_boundary = new_boundary(
                1_000.0,
                data,
                ModelTest::LogNormal,
                None,
                Some(*CDF_BOUNDARY),
                Some(*CDF_BOUNDARY),
            )
            .expect("Failed to create metrics boundary.");
            assert_no_boundary(&metrics_boundary);
        }
    }

    #[test]
    fn boundary_iqr() {
        let metrics_boundary = new_boundary(
            3.0,
            DATA_FIVE,
            ModelTest::Iqr,
            None,
            Some(*IQR_BOUNDARY),
            Some(*IQR_BOUNDARY),
        )
        .expect("Failed to create metrics boundary.");
        assert_limits(
            &metrics_boundary,
            Some(IQR_BASELINE),
            Some(IQR_LOWER_LIMIT),
            Some(IQR_UPPER_LIMIT),
        );
        assert_eq!(metrics_boundary.outlier, None);

        let lower = Some(*IQR_BOUNDARY);
        let upper = Some(*IQR_BOUNDARY);

        // The limits themselves are inclusive.
        let side = outlier_for(0.0, DATA_FIVE, ModelTest::Iqr, lower, upper);
        assert_eq!(side, None);
        let side = outlier_for(6.0, DATA_FIVE, ModelTest::Iqr, lower, upper);
        assert_eq!(side, None);

        let side = outlier_for(-0.5, DATA_FIVE, ModelTest::Iqr, lower, upper);
        assert_eq!(side, Some(BoundaryLimit::Lower));
        let side = outlier_for(6.5, DATA_FIVE, ModelTest::Iqr, lower, upper);
        assert_eq!(side, Some(BoundaryLimit::Upper));
    }

    #[test]
    fn boundary_delta_iqr() {
        let metrics_boundary = new_boundary(
            3.0,
            DATA_FIVE,
            ModelTest::DeltaIqr,
            None,
            Some(*IQR_BOUNDARY),
            Some(*IQR_BOUNDARY),
        )
        .expect("Failed to create metrics boundary.");
        assert_limits(
            &metrics_boundary,
            Some(DELTA_IQR_BASELINE),
            Some(DELTA_IQR_LOWER_LIMIT),
            Some(DELTA_IQR_UPPER_LIMIT),
        );
        assert_eq!(metrics_boundary.outlier, None);

        let lower = Some(*IQR_BOUNDARY);
        let upper = Some(*IQR_BOUNDARY);

        let side = outlier_for(2.0, DATA_FIVE, ModelTest::DeltaIqr, lower, upper);
        assert_eq!(side, None);
        let side = outlier_for(4.0, DATA_FIVE, ModelTest::DeltaIqr, lower, upper);
        assert_eq!(side, None);

        let side = outlier_for(1.0, DATA_FIVE, ModelTest::DeltaIqr, lower, upper);
        assert_eq!(side, Some(BoundaryLimit::Lower));
        let side = outlier_for(5.0, DATA_FIVE, ModelTest::DeltaIqr, lower, upper);
        assert_eq!(side, Some(BoundaryLimit::Upper));
    }

    // For the same data, datum, and multiplier, the delta IQR limits are tighter here:
    // a datum of 5.0 is within the plain IQR limits but outside the delta IQR limits.
    #[test]
    fn boundary_iqr_vs_delta_iqr() {
        let lower = Some(*IQR_BOUNDARY);
        let upper = Some(*IQR_BOUNDARY);

        let side = outlier_for(5.0, DATA_FIVE, ModelTest::Iqr, lower, upper);
        assert_eq!(side, None);

        let side = outlier_for(5.0, DATA_FIVE, ModelTest::DeltaIqr, lower, upper);
        assert_eq!(side, Some(BoundaryLimit::Upper));
    }

    // A single datum is enough for plain IQR (a degenerate zero-width interval),
    // but delta IQR needs at least two data points to compute deltas.
    #[test]
    fn boundary_iqr_single_datum() {
        let metrics_boundary = new_boundary(
            3.0,
            DATA_SINGLE,
            ModelTest::Iqr,
            None,
            Some(*IQR_BOUNDARY),
            Some(*IQR_BOUNDARY),
        )
        .expect("Failed to create metrics boundary.");
        assert_limits(&metrics_boundary, Some(3.0), Some(3.0), Some(3.0));
        assert_eq!(metrics_boundary.outlier, None);

        let side = outlier_for(
            3.1,
            DATA_SINGLE,
            ModelTest::Iqr,
            Some(*IQR_BOUNDARY),
            Some(*IQR_BOUNDARY),
        );
        assert_eq!(side, Some(BoundaryLimit::Upper));

        let metrics_boundary = new_boundary(
            1_000.0,
            DATA_SINGLE,
            ModelTest::DeltaIqr,
            None,
            Some(*IQR_BOUNDARY),
            Some(*IQR_BOUNDARY),
        )
        .expect("Failed to create metrics boundary.");
        assert_no_boundary(&metrics_boundary);
    }

    // A negative boundary is a valid `Boundary` but an invalid `IqrBoundary`.
    #[test]
    fn boundary_iqr_invalid_boundary() {
        for &model_test in &[ModelTest::Iqr, ModelTest::DeltaIqr] {
            let result = new_boundary(
                3.0,
                DATA_FIVE,
                model_test,
                None,
                None,
                Some(*NEGATIVE_BOUNDARY),
            );
            assert!(matches!(result, Err(BoundaryError::Valid(_))));
        }
    }
}
