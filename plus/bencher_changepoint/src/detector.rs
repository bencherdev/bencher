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
// d4f62a1c38dea6053ce31d1f1fba7e53105d2e66:
// `ChangePointDetector.get_change_points` of
// `otava/change_point_divisive/detector.py`, and `SignificanceTester.is_significant`
// of `otava/change_point_divisive/base.py`, with the t-test as its significance
// tester. Its tests translate `test_ttest` and, at the bounds 7 and 21 only,
// `test_detector_limits_analysis_to_requested_window` of
// `tests/change_point_divisive_test.py`. Changes: the caller passes the window
// itself in place of its bounds, and the indices it returns are relative to the
// window.

use crate::{
    Changepoint, ChangepointError,
    calculator::Calculator,
    tester::{change_point, intervals},
};

/// E-divisive on one window: split the interval with the best candidate while
/// the t-test finds the split significant, with `<=` as Otava's `is_significant`.
pub fn change_points(
    window: &[f64],
    max_pvalue: f64,
) -> Result<Vec<Changepoint>, ChangepointError> {
    let calculator = Calculator::new(window);
    let mut change_points: Vec<Changepoint> = Vec::new();
    while let Some((interval, candidate)) =
        calculator.next_candidate(&intervals(&change_points, window.len()))
    {
        let cp = change_point(
            window,
            interval.start..candidate.index,
            candidate.index..interval.end,
        )?;
        if cp.pvalue <= max_pvalue {
            change_points.push(cp);
            change_points.sort_by_key(|cp| cp.index);
        } else {
            break;
        }
    }
    Ok(change_points)
}

#[cfg(test)]
mod tests {
    use super::change_points;
    use crate::tests::{SEQUENCE, assert_relative_eq};

    #[test]
    fn ttest() {
        let indices: Vec<usize> = change_points(&SEQUENCE, 0.01)
            .unwrap()
            .iter()
            .map(|cp| cp.index)
            .collect();
        assert_eq!(indices, [5, 8]);
    }

    #[test]
    fn detector_limits_analysis_to_requested_window() {
        let sequence: Vec<f64> = [500.0; 7]
            .into_iter()
            .chain(SEQUENCE)
            .chain([-500.0; 7])
            .collect();
        let (start, end) = (7, 21);
        let points = change_points(&sequence[start..end], 0.01).unwrap();
        let indices: Vec<usize> = points.iter().map(|cp| cp.index + start).collect();
        assert_eq!(indices, [12, 15]);
        assert_relative_eq(points[0].mean_before, 0.56);
        assert_relative_eq(points[1].mean_after, 149.35);
    }
}
