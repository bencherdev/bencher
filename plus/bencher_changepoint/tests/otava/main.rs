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
// Translated to Rust by Bencher from the tests of Apache Otava at commit
// d4f62a1c38dea6053ce31d1f1fba7e53105d2e66: from `tests/analysis_test.py`,
// `test_single_series` without its incremental calls and
// `test_merge_refreshes_adjacent_statistics`; every test of
// `tests/tigerbeetle_test.py`; from `tests/series_test.py`,
// `test_change_point_detection`, `test_change_point_detection_many`,
// `test_change_point_min_magnitude`, and `test_div_by_zero`; and from
// `tests/csv_e2e_test.py`, the change points of `test_bundled_csv_example` and
// `test_analyze_csv`. Changes: each test calls the full analysis on one series
// at a time, `test_tb_statistics_match_final_segments` checks the t-test
// against its own sums in place of scipy's `ttest_ind`, and `vectors` adds the
// statistics that Otava computes, from `generate.py`. The edge tests are
// Bencher's.
//
// The Otava tests at that commit that are not ported, and why:
// - The permutation path, which draws random numbers: `test_single_series_original`
//   of `tests/analysis_test.py`, `test_permutation_calculation`,
//   `test_permutation_test`, `test_permutation_test_integers`,
//   `test_copy_permutation_stats`, and `test_permutation_stats_tojson` of
//   `tests/change_point_divisive_test.py`, and `test_orig_edivisive` of
//   `tests/series_test.py`.
// - Unseeded or numpy random streams, which Rust cannot reproduce:
//   `test_split_respects_windows_for_array_and_list_input` of
//   `tests/change_point_divisive_test.py`,
//   `test_change_point_detection_performance` of `tests/series_test.py`, and
//   `test_ttest_matches_raw_sample_student_test_for_seeded_samples` of
//   `tests/ttest_test.py`.
// - The incremental path, which the port leaves out: the incremental calls of
//   `test_single_series`, and `test_incremental_otava`, `test_can_append`,
//   `test_append_on_stable_series`, `test_append_invalidates_by_time_view`,
//   `test_append_does_not_rebuild_unread_by_time_view`,
//   `test_append_refreshes_change_points_timestamp`, and
//   `test_append_computes_change_points_first` of `tests/series_test.py`.
// - Negative window bounds, which a Rust slice does not have: the `(-21, -7)`
//   cases of `test_detector_limits_analysis_to_requested_window`.
// - A sorted order check, since the port only ever builds change points
//   sorted: the unsorted half of
//   `test_get_intervals_requires_sorted_change_points`.
// - An empty side, which the port never forms: `test_ttest_rejects_empty_samples`
//   of `tests/ttest_test.py`, and `test_basestats_empty_side_raises` and
//   `test_tester_compare_empty_raises` of `tests/change_point_classes_test.py`.
// - A given p value or a list of intervals, which the port's tester does not
//   take: `test_basestats_pvalue_defaults_to_one`,
//   `test_tester_compare_returns_basestats`, `test_tester_get_sides_split_step`,
//   `test_tester_get_sides_merge_step`, `test_tester_get_sides_not_exist`, and
//   `test_tester_is_significant` of `tests/change_point_classes_test.py`.
// - Copies, JSON, and the containers of change points by time and by metric,
//   which the port does not have: `test_copy_ttest_stats` and
//   `test_ttest_stats_tojson` of `tests/change_point_divisive_test.py`, the
//   rest of `tests/change_point_classes_test.py`, the JSON and serialization
//   half of `test_ttest_reports_sample_standard_deviations`, and
//   `test_get_stable_range`, `test_analysis_options_is_pydantic_model`,
//   `test_analysis_options_rejects_unknown_fields`, the
//   `test_analyzed_series_json_*` and `test_series_*` tests, `test_validate`,
//   `test_change_points_computed_lazily_and_cached`, and
//   `test_from_json_does_not_recompute` of `tests/series_test.py`.
// - The command line, the importers, the configuration, the reports, and the
//   notifications, which are outside the analysis: every other test file, and
//   the branch tests of `tests/csv_e2e_test.py`.

#![expect(
    unused_crate_dependencies,
    clippy::tests_outside_test_module,
    clippy::unreadable_literal,
    clippy::unwrap_used,
    reason = "integration test file"
)]

#[rustfmt::skip]
#[expect(
    clippy::approx_constant,
    reason = "a generated standard deviation equals 1 over the square root of 2"
)]
mod vectors;

use bencher_changepoint::{Changepoint, ChangepointError, Parameters, detect};
use vectors::{
    CASES, CONSTANT, CSV_METRIC_1, CSV_METRIC_2, DIV_BY_ZERO, Expected, MERGE_NEIGHBORS,
    MERGE_NEIGHBORS_TAIL, ONE_OUTLIER, PLATEAUS, SERIES_1, SERIES_2, SINGLE, STEP_AT_LAST_POINT,
    TIGERBEETLE, TWO_POINTS, TWO_STEPS_IN_ONE_WINDOW,
};

/// The window length and p value of the Otava command line.
const DEFAULT_WINDOW_LEN: usize = 50;
const DEFAULT_MAX_PVALUE: f64 = 0.001;
const NONE: [usize; 0] = [];

#[test]
fn otava_vectors() {
    let mut failures = Vec::new();
    for case in CASES {
        let parameters = Parameters {
            window_len: case.window_len,
            max_pvalue: case.max_pvalue,
            min_magnitude: case.min_magnitude,
        };
        let name = format!(
            "{} window {} p {} magnitude {}",
            case.series, case.window_len, case.max_pvalue, case.min_magnitude
        );
        let actual = detect(case.values, parameters).unwrap();
        let actual_indices: Vec<usize> = actual.iter().map(|cp| cp.index).collect();
        let expected_indices: Vec<usize> = case.changepoints.iter().map(|cp| cp.index).collect();
        if actual_indices != expected_indices {
            failures.push(format!(
                "{name}: indices {actual_indices:?} != {expected_indices:?}"
            ));
            continue;
        }
        for (actual, expected) in actual.iter().zip(case.changepoints) {
            if let Some(field) = differs(actual, expected) {
                failures.push(format!("{name}: index {}: {field}", expected.index));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn single_series() {
    assert_eq!(indices(SINGLE, 10, 0.0001, 0.0), [10]);
}

#[test]
fn tigerbeetle() {
    let tigerbeetle =
        |max_pvalue, min_magnitude| indices(TIGERBEETLE, 30, max_pvalue, min_magnitude);
    assert_eq!(tigerbeetle(0.01, 0.05), [15, 71]);
    assert_eq!(tigerbeetle(0.05, 0.05), [15, 71]);
    assert_eq!(tigerbeetle(0.1, 0.05), [10, 11, 15, 71]);
    assert_eq!(tigerbeetle(0.2, 0.05), [10, 11, 15, 71]);
    assert_eq!(
        tigerbeetle(0.2, 0.0),
        [
            3, 6, 7, 10, 11, 13, 15, 16, 28, 29, 35, 37, 39, 41, 44, 48, 49, 56, 61, 71, 74, 76,
            82, 84, 91, 95, 108, 117, 131, 142, 148, 165, 169, 192, 197, 201, 202, 206, 212, 220,
            230, 241, 249, 260, 282, 288, 305, 306, 330, 363
        ]
    );
    assert_eq!(
        tigerbeetle(0.1, 0.0),
        [
            3, 10, 11, 15, 28, 29, 35, 37, 41, 44, 48, 49, 61, 71, 95, 117, 131, 142, 148, 165,
            169, 192, 206, 212, 236, 260, 268, 278, 282, 363
        ]
    );
    assert_eq!(
        tigerbeetle(0.01, 0.0),
        [15, 61, 71, 95, 117, 131, 142, 148, 192, 212, 260]
    );
    assert_eq!(
        tigerbeetle(0.001, 0.0),
        [15, 61, 71, 95, 117, 131, 142, 148, 192, 212]
    );
    assert_eq!(tigerbeetle(0.0001, 0.0), [71, 95, 131, 192, 212]);
    assert_eq!(tigerbeetle(0.00001, 0.0), [71, 95, 131, 192]);
}

#[test]
fn tigerbeetle_statistics_match_final_segments() {
    for (max_pvalue, min_magnitude) in [
        (0.001, 0.0),
        (0.01, 0.0),
        (0.1, 0.0),
        (0.2, 0.0),
        (0.1, 0.05),
        (0.2, 0.05),
    ] {
        let parameters = Parameters {
            window_len: 30,
            max_pvalue,
            min_magnitude,
        };
        let points = detect(TIGERBEETLE, parameters).unwrap();
        assert_ne!(points.len(), 0, "p {max_pvalue} magnitude {min_magnitude}");
        let bounds: Vec<usize> = std::iter::once(0)
            .chain(points.iter().map(|point| point.index))
            .chain(std::iter::once(TIGERBEETLE.len()))
            .collect();
        let ends = bounds.iter().zip(&bounds[1..]).zip(&bounds[2..]);
        for (point, ((&start, &split), &end)) in points.iter().zip(ends) {
            let (left, right) = (&TIGERBEETLE[start..split], &TIGERBEETLE[split..end]);
            let (mean_left, std_left) = mean_and_sample_std(left);
            let (mean_right, std_right) = mean_and_sample_std(right);
            let (n_left, n_right) = (count(left.len()), count(right.len()));
            let freedom = n_left + n_right - 2.0;
            let pooled = ((n_left - 1.0) * std_left * std_left
                + (n_right - 1.0) * std_right * std_right)
                / freedom;
            let tstatistic =
                (mean_left - mean_right) / (pooled * (1.0 / n_left + 1.0 / n_right)).sqrt();
            for (field, actual, expected) in [
                ("mean_before", point.mean_before, mean_left),
                ("mean_after", point.mean_after, mean_right),
                ("stddev_before", point.stddev_before, std_left),
                ("stddev_after", point.stddev_after, std_right),
                ("tstatistic", point.tstatistic, tstatistic),
            ] {
                assert!(
                    (actual - expected).abs() <= 1e-9 * expected.abs(),
                    "p {max_pvalue} magnitude {min_magnitude} index {}: {field} {actual} != {expected}",
                    point.index
                );
            }
            assert_eq!(point.degrees_of_freedom, left.len() + right.len() - 2);
            assert!(point.pvalue <= max_pvalue, "{point:?}");
            assert!(point.magnitude() >= min_magnitude, "{point:?}");
        }
    }
}

#[test]
fn merge_refreshes_adjacent_statistics() {
    for (series, expected_indices) in [
        (MERGE_NEIGHBORS, &[4][..]),
        (MERGE_NEIGHBORS_TAIL, &[4, 12][..]),
    ] {
        let parameters = Parameters {
            window_len: 50,
            max_pvalue: 0.001,
            min_magnitude: 0.1,
        };
        let points = detect(series, parameters).unwrap();
        let indices: Vec<usize> = points.iter().map(|point| point.index).collect();
        assert_eq!(indices, expected_indices);
        let (mean, std) = (2.025, 0.02672612419124244);
        let first = &points[0];
        assert!((first.mean_after - mean).abs() <= 1e-9 * mean, "{first:?}");
        assert!((first.stddev_after - std).abs() <= 1e-9 * std, "{first:?}");
        if let Some(second) = points.get(1) {
            assert!(
                (second.mean_before - mean).abs() <= 1e-9 * mean,
                "{second:?}"
            );
            assert!(
                (second.stddev_before - std).abs() <= 1e-9 * std,
                "{second:?}"
            );
        }
    }
}

#[test]
fn change_point_detection() {
    assert_eq!(defaults(SERIES_1, 0.0), [6]);
    assert_eq!(defaults(SERIES_2, 0.0), [4]);
}

#[test]
fn change_point_min_magnitude() {
    let parameters = Parameters {
        window_len: DEFAULT_WINDOW_LEN,
        max_pvalue: DEFAULT_MAX_PVALUE,
        min_magnitude: 0.2,
    };
    let series_1 = detect(SERIES_1, parameters).unwrap();
    let series_2 = detect(SERIES_2, parameters).unwrap();
    assert_eq!(series_1.iter().map(|cp| cp.index).collect::<Vec<_>>(), [6]);
    assert_eq!(series_2, []);
    for cp in series_1 {
        assert!(cp.magnitude() >= 0.2, "{cp:?}");
    }
}

#[test]
fn div_by_zero() {
    let indices = defaults(DIV_BY_ZERO, 0.0);
    assert_eq!(indices.len(), 2);
    assert_eq!(indices.first(), Some(&3));
}

#[test]
fn change_point_detection_many() {
    assert_eq!(
        indices(PLATEAUS, DEFAULT_WINDOW_LEN, 0.05, 0.0),
        [5, 13, 21]
    );
}

#[test]
fn csv_example() {
    assert_eq!(defaults(CSV_METRIC_1, 0.0), NONE);
    let parameters = Parameters {
        window_len: DEFAULT_WINDOW_LEN,
        max_pvalue: DEFAULT_MAX_PVALUE,
        min_magnitude: 0.0,
    };
    let metric_2 = detect(CSV_METRIC_2, parameters).unwrap();
    assert_eq!(metric_2.len(), 1);
    // Otava prints the change as -12.9%.
    assert!(
        metric_2
            .iter()
            .all(|cp| cp.index == 6 && format!("{:.1}", cp.forward_change_percent()) == "-12.9"),
        "{metric_2:?}"
    );
}

#[test]
fn fewer_than_three_points() {
    for len in 0..=2 {
        assert_eq!(defaults(&TWO_POINTS[..len], 0.0), NONE, "{len}");
    }
}

#[test]
fn constant_series() {
    assert_eq!(defaults(CONSTANT, 0.0), NONE);
}

#[test]
fn one_outlier() {
    assert_eq!(defaults(ONE_OUTLIER, 0.0), NONE);
}

#[test]
fn step_at_the_last_point() {
    assert_eq!(defaults(STEP_AT_LAST_POINT, 0.0), [59]);
}

#[test]
fn two_steps_in_one_window() {
    assert_eq!(defaults(TWO_STEPS_IN_ONE_WINDOW, 0.0), [20, 30]);
}

#[test]
fn non_finite_value() {
    for value in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        let mut series = CONSTANT.to_vec();
        series[3] = value;
        let parameters = Parameters {
            window_len: DEFAULT_WINDOW_LEN,
            max_pvalue: DEFAULT_MAX_PVALUE,
            min_magnitude: 0.0,
        };
        let error = detect(&series, parameters).unwrap_err();
        assert!(
            matches!(error, ChangepointError::NonFinite { index: 3, .. }),
            "{error}"
        );
    }
}

#[test]
fn window_shorter_than_two() {
    for window_len in [0, 1] {
        let parameters = Parameters {
            window_len,
            max_pvalue: DEFAULT_MAX_PVALUE,
            min_magnitude: 0.0,
        };
        let error = detect(CONSTANT, parameters).unwrap_err();
        assert!(
            matches!(error, ChangepointError::WindowLength(len) if len == window_len),
            "{error}"
        );
    }
}

fn defaults(series: &[f64], min_magnitude: f64) -> Vec<usize> {
    indices(
        series,
        DEFAULT_WINDOW_LEN,
        DEFAULT_MAX_PVALUE,
        min_magnitude,
    )
}

fn indices(series: &[f64], window_len: usize, max_pvalue: f64, min_magnitude: f64) -> Vec<usize> {
    let parameters = Parameters {
        window_len,
        max_pvalue,
        min_magnitude,
    };
    detect(series, parameters)
        .unwrap()
        .iter()
        .map(|cp| cp.index)
        .collect()
}

/// The first statistic that is not within a relative 1e-9 of Otava's.
fn differs(actual: &Changepoint, expected: &Expected) -> Option<String> {
    let Expected {
        index: _,
        mean_before,
        mean_after,
        stddev_before,
        stddev_after,
        pvalue,
        tstatistic,
        degrees_of_freedom,
    } = *expected;
    if actual.degrees_of_freedom != degrees_of_freedom {
        return Some(format!(
            "degrees_of_freedom {} != {degrees_of_freedom}",
            actual.degrees_of_freedom
        ));
    }
    [
        ("mean_before", actual.mean_before, mean_before),
        ("mean_after", actual.mean_after, mean_after),
        ("stddev_before", actual.stddev_before, stddev_before),
        ("stddev_after", actual.stddev_after, stddev_after),
        ("pvalue", actual.pvalue, pvalue),
        ("tstatistic", actual.tstatistic, tstatistic),
    ]
    .into_iter()
    .find(|&(_, actual, expected)| {
        if expected.is_nan() {
            !actual.is_nan()
        } else if expected.is_infinite() {
            !actual.is_infinite() || actual.is_sign_negative() != expected.is_sign_negative()
        } else {
            actual.is_nan() || (actual - expected).abs() > 1e-9 * expected.abs()
        }
    })
    .map(|(field, actual, expected)| format!("{field} {actual:e} != {expected:e}"))
}

fn mean_and_sample_std(values: &[f64]) -> (f64, f64) {
    let n = count(values.len());
    let mean = values.iter().sum::<f64>() / n;
    if values.len() < 2 {
        return (mean, 0.0);
    }
    let squares: f64 = values.iter().map(|value| (value - mean).powi(2)).sum();
    (mean, (squares / (n - 1.0)).sqrt())
}

fn count(n: usize) -> f64 {
    f64::from(u32::try_from(n).unwrap())
}
