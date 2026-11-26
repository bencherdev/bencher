use std::num::NonZeroU32;

use bencher_json::{Boundary, ModelTest};

/// Whether a test's boundary is a probability, which covers a whole family at BMF version 1.
///
/// A family is every check a model makes in one report, on one side.
/// A percentage, IQR, delta IQR, or static boundary is a size, and covers one check.
pub fn is_corrected(model_test: ModelTest) -> bool {
    match model_test {
        ModelTest::TTest | ModelTest::ZScore | ModelTest::LogNormal => true,
        ModelTest::Static | ModelTest::Percentage | ModelTest::Iqr | ModelTest::DeltaIqr => false,
    }
}

/// The level each check of a family runs at, so the family as a whole keeps the boundary's
/// chance of a false alert: `1 - (1 - boundary) / family`, the Bonferroni correction.
///
/// A level that rounds to 1 is held to the largest probability boundary.
pub fn per_test_level(boundary: Boundary, family: NonZeroU32) -> Boundary {
    let level = 1.0 - (1.0 - f64::from(boundary)) / f64::from(family.get());
    Boundary::try_from(level.min(f64::from(Boundary::MAX_STATISTICAL))).unwrap_or(boundary)
}

#[cfg(test)]
mod tests {
    use std::num::NonZeroU32;

    use bencher_json::{Boundary, CdfBoundary, ModelTest};
    use statrs::distribution::{ContinuousCDF as _, Normal};

    use super::per_test_level;
    use crate::{MetricsBoundary, MetricsData};

    fn family(size: u32) -> NonZeroU32 {
        NonZeroU32::new(size).expect("a family has a member")
    }

    fn boundary(boundary: f64) -> Boundary {
        Boundary::try_from(boundary).expect("a valid boundary")
    }

    // The docs' example: 200 lines at 0.99 test each line at 0.99995, which moves a normal
    // limit from about 2.33 to about 3.89 standard deviations.
    #[test]
    fn two_hundred_lines_at_0_99_test_each_at_0_99995() {
        let level = f64::from(per_test_level(Boundary::NINETY_NINE, family(200)));
        assert!((level - 0.999_95).abs() < 1e-12, "{level}");

        let normal = Normal::standard();
        let per_line = normal.inverse_cdf(0.99);
        let per_report = normal.inverse_cdf(level);
        assert!((per_line - 2.33).abs() < 0.01, "{per_line}");
        assert!((per_report - 3.89).abs() < 0.01, "{per_report}");
    }

    // A level that rounds to 1 is no probability boundary, and would fail the test it
    // corrects, so it stays at the largest one.
    #[test]
    fn a_level_that_rounds_to_1_stays_a_probability() {
        let level = per_test_level(Boundary::MAX_STATISTICAL, family(u32::MAX));
        assert_eq!(level, Boundary::MAX_STATISTICAL);
        CdfBoundary::try_from(level).expect("a probability boundary");
    }

    /// A seeded simulation of unchanged version 1 reports: each report has `LINES` lines,
    /// each line a history of `HISTORY` normal draws and one more draw under test, all from
    /// one distribution, so every alert is false. Each line is checked by a t-test with an
    /// upper boundary of `BOUNDARY` at the level its report's family of `LINES` gives it.
    ///
    /// The share of reports with any false alert must stay at or below `1 - BOUNDARY`, within
    /// 3 standard errors of a binomial share over `REPORTS` reports. The same reports
    /// checked per line, uncorrected, must almost all raise one, so the simulation can tell
    /// the two apart.
    #[test]
    fn unchanged_reports_raise_a_false_alert_at_most_as_often_as_the_boundary_states() {
        const REPORTS: u32 = 2_000;
        const LINES: u32 = 100;
        const HISTORY: usize = 20;
        const BOUNDARY: f64 = 0.95;

        let log = slog::Logger::root(slog::Discard, slog::o!());
        let stated = boundary(BOUNDARY);
        let level = per_test_level(stated, family(LINES));
        let mut draws = Draws::new(0x00C0_FFEE);

        let mut corrected_reports = 0u32;
        let mut per_line_reports = 0u32;
        for _ in 0..REPORTS {
            let mut corrected = false;
            let mut per_line = false;
            for _ in 0..LINES {
                let metrics_data = MetricsData {
                    data: std::iter::repeat_with(|| draws.normal())
                        .take(HISTORY)
                        .collect(),
                };
                let datum = draws.normal();
                let alerts = |upper| {
                    MetricsBoundary::new(
                        &log,
                        datum,
                        &metrics_data,
                        ModelTest::TTest,
                        None,
                        None,
                        Some(upper),
                    )
                    .expect("a t-test boundary")
                    .outlier
                    .is_some()
                };
                corrected |= alerts(level);
                per_line |= alerts(stated);
            }
            corrected_reports += u32::from(corrected);
            per_line_reports += u32::from(per_line);
        }

        let reports = f64::from(REPORTS);
        let chance = 1.0 - BOUNDARY;
        let standard_error = (chance * BOUNDARY / reports).sqrt();
        let corrected_share = f64::from(corrected_reports) / reports;
        let per_line_share = f64::from(per_line_reports) / reports;
        assert!(
            corrected_share <= chance + 3.0 * standard_error,
            "{corrected_share} of reports raised a false alert, against a stated chance of {chance} (standard error {standard_error})"
        );
        assert!(
            per_line_share > 0.85,
            "{per_line_share} of reports checked per line raised a false alert"
        );
    }

    /// Standard normal draws from a fixed seed: `SplitMix64` for the uniforms, then the
    /// normal quantile of each.
    struct Draws {
        state: u64,
        normal: Normal,
    }

    impl Draws {
        fn new(seed: u64) -> Self {
            Self {
                state: seed,
                normal: Normal::standard(),
            }
        }

        fn normal(&mut self) -> f64 {
            let uniform = self.uniform();
            self.normal.inverse_cdf(uniform)
        }

        /// A uniform draw in (0, 1), never 0 or 1, from the top 53 bits of the next word.
        fn uniform(&mut self) -> f64 {
            self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.state;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^= z >> 31;
            #[expect(clippy::cast_precision_loss, reason = "53 bits fit an f64 exactly")]
            let bits = (z >> 11) as f64;
            (bits + 0.5) / 9_007_199_254_740_992.0
        }
    }
}
