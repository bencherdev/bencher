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
// d4f62a1c38dea6053ce31d1f1fba7e53105d2e66: `PairDistanceCalculator`
// of `otava/change_point_divisive/calculator.py` and `Calculator` of
// `otava/change_point_divisive/base.py`, with the power fixed at Otava's
// default of 1. Its tests translate `compute_Q_and_candidate_slow` and
// `test_calculator_candidate` of `tests/change_point_divisive_test.py`.
// Changes: the matrices are built row by row in the order numpy sums them, and
// the tests add sub-intervals and seeded series.

use std::ops::Range;

use crate::count;

/// The E-divisive statistic over one window of a series.
///
/// Every sum runs in the order numpy runs it in Otava, so that the Q values,
/// and so the ties among them, are the same to the bit.
pub struct Calculator {
    /// The distance between every pair of points, `D(i, j) = |x_i - x_j|`.
    distances: Vec<Vec<f64>>,
    /// Otava's `V`: `V[j] = D(0, j + 1) + D(1, j + 1) + ... + D(j, j + 1)`.
    vertical: Vec<f64>,
}

pub struct Candidate {
    /// The first point after the candidate change.
    pub index: usize,
    pub qhat: f64,
}

impl Calculator {
    pub fn new(series: &[f64]) -> Self {
        let distances: Vec<Vec<f64>> = series
            .iter()
            .map(|x| series.iter().map(|y| (x - y).abs()).collect())
            .collect();
        // `D` is symmetric, so column `j + 1` above the diagonal is the start of row `j + 1`.
        let vertical = distances
            .iter()
            .skip(1)
            .enumerate()
            .map(|(j, row)| sequential_sum(row.iter().take(j + 1).copied()))
            .collect();
        Self {
            distances,
            vertical,
        }
    }

    /// The best candidate over every interval of more than 1 point, and the interval it splits.
    /// Ties go to the first interval, as Python's `max` keeps the first maximum.
    pub fn next_candidate(&self, intervals: &[Range<usize>]) -> Option<(Range<usize>, Candidate)> {
        intervals
            .iter()
            .filter(|interval| interval.len() > 1)
            .filter_map(|interval| Some((interval.clone(), self.candidate(interval.clone())?)))
            .fold(None, |best, (interval, candidate)| {
                if best
                    .as_ref()
                    .is_none_or(|(_, best): &(Range<usize>, Candidate)| candidate.qhat > best.qhat)
                {
                    Some((interval, candidate))
                } else {
                    best
                }
            })
    }

    /// The split of `interval` with the largest Q value, as `numpy.argmax` picks it:
    /// the first maximum in row major order, or the first NaN.
    pub fn candidate(&self, interval: Range<usize>) -> Option<Candidate> {
        let start = interval.start;
        let mut best: Option<(usize, f64)> = None;
        for (i, row) in self.q_values(interval).iter().enumerate() {
            for &q in row {
                if q.is_nan() {
                    return Some(Candidate {
                        index: start + i + 1,
                        qhat: q,
                    });
                }
                if best.is_none_or(|(_, best_q)| q > best_q) {
                    best = Some((i, q));
                }
            }
        }
        best.map(|(i, qhat)| Candidate {
            index: start + i + 1,
            qhat,
        })
    }

    /// Otava's `_get_Q_vals`: the square matrix of `Q(tau, kappa)` for every `start < tau < kappa <= end`,
    /// with row `tau - start - 1`, column `kappa - start - 2`, and 0 below the diagonal.
    pub fn q_values(&self, interval: Range<usize>) -> Vec<Vec<f64>> {
        let Range { start, end } = interval;
        let len = end - 1 - start;

        // `V` within the interval: Otava subtracts the distances from the points before `start`.
        let vertical: Vec<f64> = self
            .vertical
            .iter()
            .skip(start)
            .zip(self.distances.iter().skip(start + 1))
            .take(len)
            .map(|(v, row)| v - sequential_sum(row.iter().take(start).copied()))
            .collect();
        let cumulative_vertical = cumulative_sum(vertical.iter().copied());

        // Otava's `H` within the interval: row `i` is the running sum of the distances
        // from point `start + i` to the points after it, and 0 before the diagonal.
        let horizontal: Vec<Vec<f64>> = self
            .distances
            .iter()
            .skip(start)
            .take(len)
            .enumerate()
            .map(|(i, row)| {
                let mut row_h = vec![0.0; i];
                row_h.extend(cumulative_sum(
                    row.iter().skip(start + i + 1).take(len - i).copied(),
                ));
                row_h
            })
            .collect();

        // For each row, the sum of `H` over the rows below it, added from the bottom row up,
        // as numpy's flipped `cumsum` adds them.
        let mut below_rev = Vec::with_capacity(len);
        let mut below_sum = vec![0.0; len];
        for row_h in horizontal.iter().rev() {
            below_rev.push(below_sum.clone());
            for (sum, h) in below_sum.iter_mut().zip(row_h) {
                *sum += h;
            }
        }

        // For each row, the sum of `H` over the rows down to it, added from the top row down.
        let mut above_sum = vec![0.0; len];
        let mut left_vertical = 0.0;
        let mut matrix = Vec::with_capacity(len);
        for (i, ((row_h, below), &next_vertical)) in horizontal
            .iter()
            .zip(below_rev.iter().rev())
            .zip(&cumulative_vertical)
            .enumerate()
        {
            for (sum, h) in above_sum.iter_mut().zip(row_h) {
                *sum += h;
            }
            let row = above_sum
                .iter()
                .zip(below)
                .enumerate()
                .map(|(j, (above, below))| {
                    if j < i {
                        return 0.0;
                    }
                    let a_term = 2.0 / count(j + 2) * (above - left_vertical);
                    let b_term = if i == 0 {
                        0.0
                    } else {
                        count(2 * (j - i + 1)) / count((j + 2) * i) * left_vertical
                    };
                    let c_term = if j > i {
                        count(2 * (i + 1)) / count((j + 2) * (j - i)) * below
                    } else {
                        0.0
                    };
                    a_term - b_term - c_term
                })
                .collect();
            matrix.push(row);
            left_vertical = next_vertical;
        }
        matrix
    }
}

fn sequential_sum<I: Iterator<Item = f64>>(values: I) -> f64 {
    values.fold(0.0, |sum, value| sum + value)
}

fn cumulative_sum<I: Iterator<Item = f64>>(values: I) -> Vec<f64> {
    values
        .scan(0.0, |sum, value| {
            *sum += value;
            Some(*sum)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use rand::{RngExt as _, SeedableRng as _, rngs::StdRng};

    use super::Calculator;
    use crate::tests::SEQUENCE;

    #[test]
    fn calculator_candidate() {
        let calculator = Calculator::new(&SEQUENCE);
        let q = calculator.q_values(0..SEQUENCE.len());
        let (slow_q, slow_q_max, slow_index) = q_slow(&SEQUENCE);
        assert_all_close(&q, &slow_q, &SEQUENCE);

        let candidate = calculator.candidate(0..SEQUENCE.len()).unwrap();
        crate::tests::assert_relative_eq(candidate.qhat, slow_q_max);
        assert_eq!(Some(candidate.index), slow_index);
    }

    #[test]
    fn q_values_match_slow_reference() {
        let mut rng = StdRng::seed_from_u64(0x0007_a7a0);
        for _ in 0..300 {
            let len = rng.random_range(2..=18);
            let integers = rng.random_bool(0.3);
            let step: f64 = rng.random_range(0.0..10.0);
            let series: Vec<f64> = (0..len)
                .map(|i| {
                    let level = if i * 2 < len { 0.0 } else { step };
                    if integers {
                        level.round() + f64::from(rng.random_range(0..4u8))
                    } else {
                        level + rng.random_range(-1.0..1.0)
                    }
                })
                .collect();
            let calculator = Calculator::new(&series);
            let start = rng.random_range(0..len - 1);
            let end = rng.random_range(start + 2..=len);
            for interval in [0..len, start..end] {
                let window = &series[interval.clone()];
                let (slow_q, slow_q_max, slow_index) = q_slow(window);
                assert_all_close(&calculator.q_values(interval.clone()), &slow_q, window);
                // Integer series tie, and a tie may break apart in the slow sums.
                if !integers && slow_q_max > 0.0 {
                    let candidate = calculator.candidate(interval.clone()).unwrap();
                    assert_eq!(
                        Some(candidate.index - interval.start),
                        slow_index,
                        "{series:?} {interval:?}"
                    );
                }
            }
        }
    }

    /// Otava's slow reference for the statistic: every Q value from its definition, and the first maximum above 0.
    fn q_slow(sequence: &[f64]) -> (Vec<Vec<f64>>, f64, Option<usize>) {
        let len = sequence.len();
        let mut q_matrix = vec![vec![0.0; len - 1]; len - 1];
        let mut q_max = 0.0;
        let mut candidate = None;
        for tau in 1..len {
            for kappa in tau + 1..=len {
                let before = &sequence[..tau];
                let after = &sequence[tau..kappa];
                let (n_before, n_after) = (count(before.len()), count(after.len()));

                let mut between = 0.0;
                for x_i in before {
                    for y_j in after {
                        between += (x_i - y_j).abs();
                    }
                }
                between *= 2.0 / (n_after + n_before);

                let mut within_before = 0.0;
                for i in 0..before.len().saturating_sub(1) {
                    for k in i + 1..before.len() {
                        within_before += (before[i] - before[k]).abs();
                    }
                }
                within_before *= if before.len() > 1 {
                    2.0 * n_after / (n_after + n_before) / (n_before - 1.0)
                } else {
                    0.0
                };

                let mut within_after = 0.0;
                for j in 0..after.len().saturating_sub(1) {
                    for k in j..after.len() {
                        within_after += (after[j] - after[k]).abs();
                    }
                }
                within_after *= if after.len() > 1 {
                    2.0 * n_before / (n_after + n_before) / (n_after - 1.0)
                } else {
                    0.0
                };

                let value = between - within_before - within_after;
                q_matrix[tau - 1][kappa - 2] = value;
                if value > q_max {
                    q_max = value;
                    candidate = Some(tau);
                }
            }
        }
        (q_matrix, q_max, candidate)
    }

    #[track_caller]
    fn assert_all_close(actual: &[Vec<f64>], expected: &[Vec<f64>], series: &[f64]) {
        let scale = series.iter().fold(0.0f64, |max, x| max.max(x.abs())) * count(series.len());
        assert_eq!(actual.len(), expected.len(), "{series:?}");
        for (actual_row, expected_row) in actual.iter().zip(expected) {
            assert_eq!(actual_row.len(), expected_row.len(), "{series:?}");
            for (a, e) in actual_row.iter().zip(expected_row) {
                assert!(
                    (a - e).abs() <= 1e-12 * scale.max(1.0),
                    "{a} != {e} in {series:?}"
                );
            }
        }
    }

    fn count(n: usize) -> f64 {
        f64::from(u32::try_from(n).unwrap())
    }
}
