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
// d4f62a1c38dea6053ce31d1f1fba7e53105d2e66: `compute_change_points` and
// `TTestStats` of `otava/analysis.py`, and `ChangePoint` and `BaseStats` of
// `otava/change_point_divisive/base.py`. Changes: the change point and its
// t-test statistics are one struct without the candidate's `qhat`, a series
// with a non-finite value is refused, and a window length below 2 is an error
// in place of an assertion.

//! Changepoint detection, ported from Apache Otava.

use statrs::function::beta::BetaFuncError;

mod analysis;
mod calculator;
mod detector;
mod tester;

/// Find the changepoints in a series with the method of Apache Otava at commit `d4f62a1`.
///
/// A changepoint is a step: the mean of the series moves at one point and stays moved.
/// The method is built to find steps, not drift.
/// A slow ramp can still be flagged, as one step somewhere inside the ramp.
/// A single large outlier can also produce a false changepoint.
///
/// This is a faithful port of the full analysis that `otava analyze` runs with its default algorithm.
/// The E-divisive statistic proposes candidates in windows of `window_len` points that step by half a window.
/// A two-sided Student's t-test with a relaxed p value keeps the weak candidates.
/// A merge then prunes them against `max_pvalue` and `min_magnitude`.
/// The same series and the same 3 parameters give the changepoints and statistics of Otava.
/// The p value comes from the regularized incomplete beta function of `statrs`
/// in place of scipy's Student's t distribution.
/// Up to 100,000 degrees of freedom, the two p values agree to a relative 1e-9,
/// except for subnormal p values below 1e-307.
/// Above that the error grows, to about 4e-9 at 1,000,000 degrees of freedom and 2e-7 at 10,000,000.
/// The two p values are not always equal to the bit.
/// So when two weak changepoints tie, or nearly tie, for the largest p value,
/// the merge in the port can remove them in another order than Otava.
/// This happens mostly on integer series at a high `max_pvalue`, where exact ties are common.
///
/// It departs from Otava in one place: a series with a non-finite value is refused.
/// Otava finds no changepoint at all in such a series.
///
/// Prior art:
/// - David S. Matteson and Nicholas A. James, "A Nonparametric Approach for Multiple Change Point Analysis of Multivariate Data" (2014), for E-divisive.
/// - David Daly et al., "The Use of Change Point Detection to Identify Software Performance Regressions in a Continuous Integration System" (2020), MongoDB's `signal-processing-algorithms`.
/// - Matt Fleming et al., "Hunter: Using Change Point Detection to Hunt for Performance Regressions" (2023), now Apache Otava.
pub fn detect(
    series: &[f64],
    parameters: Parameters,
) -> Result<Vec<Changepoint>, ChangepointError> {
    if parameters.window_len < 2 {
        return Err(ChangepointError::WindowLength(parameters.window_len));
    }
    if let Some((index, &value)) = series
        .iter()
        .enumerate()
        .find(|(_, value)| !value.is_finite())
    {
        return Err(ChangepointError::NonFinite { index, value });
    }
    analysis::compute_change_points(series, parameters)
}

/// The 3 parameters of an analysis, as Otava names them.
#[derive(Debug, Clone, Copy)]
pub struct Parameters {
    /// The number of points in each window.
    /// The next window starts half a window later, or at the last changepoint found if that is later.
    pub window_len: usize,
    /// A changepoint is kept only if its p value is below this.
    /// The first pass relaxes it, by a factor of 10 below 0.05 and of 2 below 0.5.
    pub max_pvalue: f64,
    /// A changepoint is kept only if its `magnitude` is above this.
    /// At 0, every changepoint with a nonzero magnitude passes.
    pub min_magnitude: f64,
}

/// A changepoint and the statistics of the two sides it splits.
///
/// The side before runs from the previous changepoint that `detect` returns, or the start of the series,
/// up to this changepoint.
/// The side after runs from this changepoint up to the next one that `detect` returns, or the end of the series.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Changepoint {
    /// The index of the first point after the change.
    pub index: usize,
    /// The mean of the side before.
    pub mean_before: f64,
    /// The mean of the side after.
    pub mean_after: f64,
    /// The sample standard deviation of the side before, or 0 if it holds 1 point.
    pub stddev_before: f64,
    /// The sample standard deviation of the side after, or 0 if it holds 1 point.
    pub stddev_after: f64,
    /// The two-sided p value of Student's t-test with pooled variance on the two sides.
    /// It is 1 with no degrees of freedom, and NaN if both sides are constant at the same value.
    pub pvalue: f64,
    /// The t statistic of that test, positive when the mean before is the larger.
    /// It is 0 with no degrees of freedom, infinite if both sides are constant at different values,
    /// and NaN if both sides are constant at the same value.
    pub tstatistic: f64,
    /// The number of points on the two sides less 2.
    pub degrees_of_freedom: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum ChangepointError {
    #[error("The window length must be at least 2, but it is {0}")]
    WindowLength(usize),
    #[error("The series has a non-finite value at index {index}: {value}")]
    NonFinite { index: usize, value: f64 },
    #[error("Failed to compute the Student's t p value with {freedom} degrees of freedom: {error}")]
    IncompleteBeta { freedom: f64, error: BetaFuncError },
}

impl Changepoint {
    /// The relative change from before to after, or 0 if the mean before is 0.
    pub fn forward_rel_change(&self) -> f64 {
        if self.mean_before == 0.0 {
            0.0
        } else {
            self.mean_after / self.mean_before - 1.0
        }
    }

    /// The relative change from after to before, or 0 if the mean after is 0.
    pub fn backward_rel_change(&self) -> f64 {
        if self.mean_after == 0.0 {
            0.0
        } else {
            self.mean_before / self.mean_after - 1.0
        }
    }

    pub fn forward_change_percent(&self) -> f64 {
        self.forward_rel_change() * 100.0
    }

    pub fn backward_change_percent(&self) -> f64 {
        self.backward_rel_change() * 100.0
    }

    /// The larger of the absolute forward and backward relative changes.
    pub fn magnitude(&self) -> f64 {
        let forward = self.forward_rel_change().abs();
        let backward = self.backward_rel_change().abs();
        if backward > forward {
            backward
        } else {
            forward
        }
    }
}

#[expect(
    clippy::cast_precision_loss,
    reason = "a count below 2^53 converts exactly"
)]
fn count(n: usize) -> f64 {
    n as f64
}

#[cfg(test)]
mod tests {
    // The 14 point sequence of `tests/change_point_divisive_test.py` in Otava at commit `d4f62a1`.
    pub const SEQUENCE: [f64; 14] = [
        0.3, 2.4, 1.5, -0.9, -0.5, 99.7, 98.3, 99.1, 149.0, 149.7, 149.5, 149.1, 148.8, 150.0,
    ];

    #[track_caller]
    pub fn assert_relative_eq(actual: f64, expected: f64) {
        if expected.is_nan() {
            assert!(actual.is_nan(), "expected NaN, got {actual}");
        } else {
            assert!(
                (actual - expected).abs() <= 1e-9 * expected.abs(),
                "expected {expected}, got {actual}"
            );
        }
    }
}
