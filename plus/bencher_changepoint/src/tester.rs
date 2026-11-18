// Licensed to the Apache Software Foundation (ASF) under one
// or more contributor license agreements.  See the NOTICE file
// distributed with this work for additional information
// regarding copyright ownership.  The ASF licenses this file
// to you under the Apache License, Version 2.0 (the
// "License"); you may not use this file except in compliance
// with the License.  You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing,
// software distributed under the License is distributed on an
// "AS IS" BASIS, WITHOUT WARRANTIES OR CONDITIONS OF ANY
// KIND, either express or implied.  See the License for the
// specific language governing permissions and limitations
// under the License.
//
// Translated to Rust by Bencher from Apache Otava at commit
// d4f62a1c38dea6053ce31d1f1fba7e53105d2e66: `TTestSignificanceTester` of
// `otava/analysis.py`, and `BaseStats.calculate`, `SignificanceTester.compare`,
// `SignificanceTester.get_sides`, and `SignificanceTester.get_intervals` of
// `otava/change_point_divisive/base.py`, with the t-test of scipy's
// `ttest_ind_from_stats` and the pairwise sum of numpy that Otava calls.
// Its tests translate, from the same commit, `test_significance_tester` of
// `tests/analysis_test.py`; the sorted half of
// `test_get_intervals_requires_sorted_change_points` of
// `tests/change_point_divisive_test.py`; `test_basestats_calculate_means_and_std`,
// `test_basestats_single_element_side_has_zero_std`,
// `test_basestats_relative_change_and_magnitude`, `test_basestats_zero_mean_guard`,
// and `test_tester_get_intervals` of `tests/change_point_classes_test.py`; and
// `test_ttest_uses_sample_variance_for_significance`,
// `test_ttest_matches_raw_sample_student_test`,
// `test_ttest_reports_sample_standard_deviations`,
// `test_ttest_two_singletons_keep_zero_df_fallback`, and
// `test_ttest_identical_constant_samples_keep_undefined_result` of
// `tests/ttest_test.py`, with the values of scipy's `ttest_ind` written out.
// Changes: the caller passes the two sides of a candidate in place of a list
// of intervals, the p value comes from the regularized incomplete beta function
// of `statrs` in place of scipy's Student's t distribution, and change points
// are not checked to be sorted, since the port only ever builds them sorted.

use std::{iter, ops::Range};

use statrs::function::beta::checked_beta_reg;

use crate::{Changepoint, ChangepointError, count};

/// Otava's `get_intervals`: the spans between sorted change points, from the start to the end of the series.
pub fn intervals(change_points: &[Changepoint], len: usize) -> Vec<Range<usize>> {
    let starts = iter::once(0).chain(change_points.iter().map(|cp| cp.index));
    let ends = change_points
        .iter()
        .map(|cp| cp.index)
        .chain(iter::once(len));
    starts
        .zip(ends)
        .map(|(start, end)| start..end)
        .filter(|interval| interval.start != interval.end)
        .collect()
}

/// The change point at the end of `left` and the start of `right`, with the t-test of its two sides.
pub fn change_point(
    series: &[f64],
    left: Range<usize>,
    right: Range<usize>,
) -> Result<Changepoint, ChangepointError> {
    let side = |range: Range<usize>| -> Vec<f64> {
        series
            .iter()
            .skip(range.start)
            .take(range.len())
            .copied()
            .collect()
    };
    compare(left.end, &side(left), &side(right))
}

/// Otava's `TTestSignificanceTester.compare`.
fn compare(index: usize, left: &[f64], right: &[f64]) -> Result<Changepoint, ChangepointError> {
    let mean_before = mean(left);
    let mean_after = mean(right);
    let stddev_before = if left.len() >= 2 {
        stddev(left, mean_before)
    } else {
        0.0
    };
    let stddev_after = if right.len() >= 2 {
        stddev(right, mean_after)
    } else {
        0.0
    };
    let degrees_of_freedom = (left.len() + right.len()).saturating_sub(2);
    let (tstatistic, pvalue) = if degrees_of_freedom > 0 {
        ttest_ind_from_stats(
            (mean_before, stddev_before, left.len()),
            (mean_after, stddev_after, right.len()),
        )?
    } else {
        (0.0, 1.0)
    };
    Ok(Changepoint {
        index,
        mean_before,
        mean_after,
        stddev_before,
        stddev_after,
        pvalue,
        tstatistic,
        degrees_of_freedom,
    })
}

/// The t statistic and the two-sided p value of scipy's `ttest_ind_from_stats` with equal variances.
fn ttest_ind_from_stats(
    (mean_1, std_1, n_1): (f64, f64, usize),
    (mean_2, std_2, n_2): (f64, f64, usize),
) -> Result<(f64, f64), ChangepointError> {
    let var_1 = if n_1 == 1 { 0.0 } else { std_1 * std_1 };
    let var_2 = if n_2 == 1 { 0.0 } else { std_2 * std_2 };
    let freedom = count(n_1) + count(n_2) - 2.0;
    let pooled = (count(n_1 - 1) * var_1 + count(n_2 - 1) * var_2) / freedom;
    let denominator = (pooled * (1.0 / count(n_1) + 1.0 / count(n_2))).sqrt();
    let t = (mean_1 - mean_2) / denominator;
    // Two equal constant sides give 0 / 0, and scipy returns NaN for both.
    if t.is_nan() {
        return Ok((t, f64::NAN));
    }
    Ok((t, two_sided_pvalue(t, freedom)?))
}

/// The two-sided p value of Student's t distribution: `I_x(freedom / 2, 1 / 2)` at `x = freedom / (freedom + t^2)`.
/// For a small `|t|`, `x` rounds to 1 and that p value to exactly 1,
/// so above 0.5 it is 1 minus the complementary `I_y(1 / 2, freedom / 2)` at `y = t^2 / (freedom + t^2)`.
fn two_sided_pvalue(t: f64, freedom: f64) -> Result<f64, ChangepointError> {
    let square = t * t;
    let beta_reg = |a, b, x| {
        checked_beta_reg(a, b, x)
            .map_err(|error| ChangepointError::IncompleteBeta { freedom, error })
    };
    let pvalue = beta_reg(freedom / 2.0, 0.5, freedom / (freedom + square))?;
    if pvalue > 0.5 {
        Ok(1.0 - beta_reg(0.5, freedom / 2.0, square / (freedom + square))?)
    } else {
        Ok(pvalue)
    }
}

fn mean(values: &[f64]) -> f64 {
    pairwise_sum(values) / count(values.len())
}

/// The sample standard deviation, as `numpy.std` computes it with `ddof=1`.
fn stddev(values: &[f64], mean: f64) -> f64 {
    let squares: Vec<f64> = values
        .iter()
        .map(|value| {
            let deviation = value - mean;
            deviation * deviation
        })
        .collect();
    (pairwise_sum(&squares) / count(values.len() - 1)).sqrt()
}

/// numpy's pairwise summation, which `numpy.mean` and `numpy.std` use.
fn pairwise_sum(values: &[f64]) -> f64 {
    const BLOCK: usize = 128;
    if values.len() < 8 {
        values.iter().fold(0.0, |sum, value| sum + value)
    } else if values.len() <= BLOCK {
        let (chunks, remainder) = values.as_chunks::<8>();
        let mut partial = [0.0; 8];
        for chunk in chunks {
            for (sum, value) in partial.iter_mut().zip(chunk) {
                *sum += value;
            }
        }
        let [p0, p1, p2, p3, p4, p5, p6, p7] = partial;
        let head = ((p0 + p1) + (p2 + p3)) + ((p4 + p5) + (p6 + p7));
        remainder.iter().fold(head, |sum, value| sum + value)
    } else {
        let half = values.len().div_euclid(2);
        let (front, back) = values.split_at(half - half.rem_euclid(8));
        pairwise_sum(front) + pairwise_sum(back)
    }
}

#[cfg(test)]
#[expect(clippy::unreadable_literal, reason = "the values as scipy prints them")]
mod tests {
    use super::{change_point, compare, intervals};
    use crate::{Changepoint, tests::assert_relative_eq};

    #[test]
    fn significance_tester() {
        let series = [1.00, 1.02, 1.05, 0.95, 0.98, 1.00, 1.02, 1.05, 0.95, 0.98];
        let cp = change_point(&series, 0..5, 5..10).unwrap();
        assert!(cp.pvalue > 0.001, "{}", cp.pvalue);
        assert!(0.99 < cp.pvalue && cp.pvalue < 1.01, "{}", cp.pvalue);

        let series = [1.00, 1.02, 1.05, 0.95, 0.98, 0.80, 0.82, 0.85, 0.79, 0.77];
        let cp = change_point(&series, 0..5, 5..10).unwrap();
        assert!(cp.pvalue <= 0.001, "{}", cp.pvalue);
        assert!(0.00 < cp.pvalue && cp.pvalue < 0.001, "{}", cp.pvalue);
    }

    #[test]
    fn means_and_std() {
        let cp = compare(3, &[10.0, 10.0, 10.0], &[20.0, 20.0, 20.0]).unwrap();
        assert_relative_eq(cp.mean_before, 10.0);
        assert_relative_eq(cp.mean_after, 20.0);
        assert_relative_eq(cp.stddev_before, 0.0);
        assert_relative_eq(cp.stddev_after, 0.0);
        // Two constant sides with different means give an infinite t statistic.
        assert_relative_eq(cp.pvalue, 0.0);
    }

    #[test]
    fn single_element_side_has_zero_std() {
        let cp = compare(1, &[5.0], &[7.0, 9.0]).unwrap();
        assert_relative_eq(cp.stddev_before, 0.0);
        assert_relative_eq(cp.stddev_after, 2.0f64.sqrt());
    }

    #[test]
    fn relative_change_and_magnitude() {
        let cp = compare(2, &[10.0, 10.0], &[20.0, 20.0]).unwrap();
        assert_relative_eq(cp.forward_rel_change(), 1.0);
        assert_relative_eq(cp.backward_rel_change(), -0.5);
        assert_relative_eq(cp.forward_change_percent(), 100.0);
        assert_relative_eq(cp.backward_change_percent(), -50.0);
        assert_relative_eq(cp.magnitude(), 1.0);
    }

    #[test]
    fn zero_mean_guard() {
        let cp = compare(2, &[0.0, 0.0], &[1.0, 1.0]).unwrap();
        assert_relative_eq(cp.forward_rel_change(), 0.0);
        let cp = compare(2, &[1.0, 1.0], &[0.0, 0.0]).unwrap();
        assert_relative_eq(cp.backward_rel_change(), 0.0);
    }

    #[test]
    fn ttest_uses_sample_variance_for_significance() {
        let cp = compare(2, &[0.0, 1.0], &[3.0, 4.0]).unwrap();
        assert_relative_eq(cp.tstatistic, -4.242640687119285);
        assert_relative_eq(cp.pvalue, 0.05131670194948621);
        assert_eq!(cp.degrees_of_freedom, 2);
        assert!(cp.pvalue > 0.05, "{}", cp.pvalue);
    }

    #[test]
    fn pvalue_near_one_keeps_its_digits() {
        // `freedom / (freedom + t^2)` rounds to 1 here, and scipy's p value is still below 1.
        let cp = compare(2, &[0.0, 2.0], &[1e-8, 2.00000001]).unwrap();
        assert_relative_eq(cp.tstatistic, -7.071067768891265e-9);
        assert_relative_eq(cp.pvalue, 0.999999995);
    }

    #[test]
    fn ttest_matches_raw_sample_student_test() {
        // The two samples, and the t statistic, p value, and degrees of freedom of scipy's `ttest_ind` on them.
        type Case = (&'static [f64], &'static [f64], f64, f64, usize);
        let cases: [Case; 7] = [
            (
                &[0.0, 1.0],
                &[3.0, 4.0],
                -4.242640687119285,
                0.05131670194948621,
                2,
            ),
            (
                &[1.0, 2.0, 5.0],
                &[0.0, 4.0, 6.0],
                -0.31234752377721225,
                0.7703819196992678,
                4,
            ),
            (
                &[1.0, 4.0],
                &[2.0, 3.0, 6.0, 8.0, 12.0],
                -1.1878759481495218,
                0.28822821475154026,
                5,
            ),
            (
                &[2.0],
                &[1.0, 3.0, 6.0, 8.0],
                -0.7191949522280763,
                0.5239811368232432,
                3,
            ),
            (
                &[1.0, 3.0, 6.0, 8.0],
                &[2.0],
                0.7191949522280763,
                0.5239811368232432,
                3,
            ),
            (
                &[2.0, 2.0],
                &[1.0, 3.0, 6.0],
                -0.7108186533109109,
                0.5284760805447716,
                3,
            ),
            (&[2.0, 2.0], &[3.0, 3.0, 3.0], f64::NEG_INFINITY, 0.0, 3),
        ];
        for (left, right, tstatistic, pvalue, degrees_of_freedom) in cases {
            let cp = compare(left.len(), left, right).unwrap();
            if tstatistic.is_infinite() {
                assert!(
                    cp.tstatistic.is_infinite()
                        && cp.tstatistic.is_sign_negative() == tstatistic.is_sign_negative(),
                    "{left:?} {right:?}: {}",
                    cp.tstatistic
                );
            } else {
                assert_relative_eq(cp.tstatistic, tstatistic);
            }
            assert_relative_eq(cp.pvalue, pvalue);
            assert_eq!(
                cp.degrees_of_freedom, degrees_of_freedom,
                "{left:?} {right:?}"
            );
        }
    }

    #[test]
    fn ttest_reports_sample_standard_deviations() {
        let cp = compare(2, &[0.0, 2.0], &[4.0, 8.0, 12.0]).unwrap();
        assert_relative_eq(cp.mean_before, 1.0);
        assert_relative_eq(cp.mean_after, 8.0);
        assert_relative_eq(cp.stddev_before, 2.0f64.sqrt());
        assert_relative_eq(cp.stddev_after, 4.0);
    }

    #[test]
    fn two_singletons_keep_zero_df_fallback() {
        for right in [1.0, 2.0] {
            let cp = compare(1, &[1.0], &[right]).unwrap();
            assert_relative_eq(cp.pvalue, 1.0);
            assert_relative_eq(cp.tstatistic, 0.0);
            assert_eq!(cp.degrees_of_freedom, 0);
            assert_relative_eq(cp.stddev_before, 0.0);
            assert_relative_eq(cp.stddev_after, 0.0);
        }
    }

    #[test]
    fn identical_constant_samples_keep_undefined_result() {
        let cp = compare(2, &[2.0, 2.0], &[2.0, 2.0, 2.0]).unwrap();
        assert!(cp.tstatistic.is_nan(), "{}", cp.tstatistic);
        assert!(cp.pvalue.is_nan(), "{}", cp.pvalue);
    }

    #[test]
    fn get_intervals() {
        let at = |index| Changepoint {
            index,
            mean_before: 1.0,
            mean_after: 2.0,
            stddev_before: 0.1,
            stddev_after: 0.1,
            pvalue: 0.001,
            tstatistic: 0.1,
            degrees_of_freedom: 18,
        };
        assert_eq!(
            intervals(&[at(5), at(10), at(15)], 20),
            [0..5, 5..10, 10..15, 15..20]
        );
        assert_eq!(intervals(&[at(3)], 6), [0..3, 3..6]);
        let whole = intervals(&[], 6);
        assert_eq!(whole.len(), 1);
        assert_eq!(whole.first(), Some(&(0..6)));
    }
}
