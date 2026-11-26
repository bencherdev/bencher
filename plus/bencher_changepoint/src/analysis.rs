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
// d4f62a1c38dea6053ce31d1f1fba7e53105d2e66: `compute_change_points`, `split`,
// and `merge` of `otava/analysis.py`. Changes: the incremental path
// (`new_points` and `old_cp`) is left out, only the final change points are
// returned, and the merge removes the weakest change point by its position in
// place of the first one at its index, which is the same change point since
// the split keeps one change point per index.

use crate::{Changepoint, ChangepointError, Parameters, detector, tester::change_point};

/// Otava's `compute_change_points`: the split step with a relaxed p value finds
/// the weak change points, and the merge step prunes them.
pub fn compute_change_points(
    series: &[f64],
    parameters: Parameters,
) -> Result<Vec<Changepoint>, ChangepointError> {
    let Parameters {
        window_len,
        max_pvalue,
        min_magnitude,
    } = parameters;
    let weak_change_points = split(series, window_len, first_pass_pvalue(max_pvalue))?;
    merge(weak_change_points, series, max_pvalue, min_magnitude)
}

fn first_pass_pvalue(max_pvalue: f64) -> f64 {
    if max_pvalue < 0.05 {
        max_pvalue * 10.0
    } else if max_pvalue < 0.5 {
        max_pvalue * 2.0
    } else {
        max_pvalue
    }
}

/// Run E-divisive on windows of `window_len` points that start half a window
/// apart, or at the last change point found, and keep the first change point
/// found at each index.
fn split(
    series: &[f64],
    window_len: usize,
    max_pvalue: f64,
) -> Result<Vec<Changepoint>, ChangepointError> {
    let step = window_len.div_euclid(2);
    let mut start = 0;
    let mut change_points: Vec<Changepoint> = Vec::new();
    while start < series.len() {
        let end = (start + window_len).min(series.len());
        let window: Vec<f64> = series
            .iter()
            .skip(start)
            .take(end - start)
            .copied()
            .collect();
        let new_change_points = detector::change_points(&window, max_pvalue)?;
        let last_new_index = new_change_points.last().map_or(0, |cp| cp.index + start);
        for cp in new_change_points {
            let index = cp.index + start;
            if !change_points.iter().any(|existing| existing.index == index) {
                change_points.push(Changepoint { index, ..cp });
            }
        }
        start = last_new_index.max(start + step);
    }
    change_points.sort_by_key(|cp| cp.index);
    change_points
        .iter()
        .enumerate()
        .map(|(position, cp)| recompute(series, &change_points, position, cp.index))
        .collect()
}

/// Remove the weakest change point while one has a p value of at least
/// `max_pvalue`, or else while the smallest magnitude is not above
/// `min_magnitude`, and recompute the two change points next to each removal.
fn merge(
    mut change_points: Vec<Changepoint>,
    series: &[f64],
    max_pvalue: f64,
    min_magnitude: f64,
) -> Result<Vec<Changepoint>, ChangepointError> {
    while let Some(mut weakest) = first_by(&change_points, |cp| cp.pvalue, |a, b| a > b) {
        if change_points
            .get(weakest)
            .is_some_and(|cp| cp.pvalue < max_pvalue)
        {
            let Some(smallest) = first_by(&change_points, Changepoint::magnitude, |a, b| a < b)
            else {
                break;
            };
            if change_points
                .get(smallest)
                .is_some_and(|cp| cp.magnitude() > min_magnitude)
            {
                break;
            }
            weakest = smallest;
        }
        change_points.remove(weakest);
        for position in [weakest.checked_sub(1), Some(weakest)]
            .into_iter()
            .flatten()
        {
            if let Some(index) = change_points.get(position).map(|cp| cp.index) {
                let cp = recompute(series, &change_points, position, index)?;
                if let Some(slot) = change_points.get_mut(position) {
                    *slot = cp;
                }
            }
        }
    }
    Ok(change_points)
}

/// The statistics of the change point at `position` and `index`, with the
/// change points next to it, or the ends of the series, as the ends of its two sides.
fn recompute(
    series: &[f64],
    change_points: &[Changepoint],
    position: usize,
    index: usize,
) -> Result<Changepoint, ChangepointError> {
    let before = position
        .checked_sub(1)
        .and_then(|before| change_points.get(before))
        .map_or(0, |cp| cp.index);
    let after = change_points
        .get(position + 1)
        .map_or(series.len(), |cp| cp.index);
    change_point(series, before..index, index..after)
}

/// The position of the first best change point, as Python's `max` and `min` find it:
/// a later one replaces the best only if `replaces` holds for their keys.
fn first_by<K, R>(change_points: &[Changepoint], key: K, replaces: R) -> Option<usize>
where
    K: Fn(&Changepoint) -> f64,
    R: Fn(f64, f64) -> bool,
{
    change_points
        .iter()
        .enumerate()
        .fold(None, |best: Option<(usize, f64)>, (position, cp)| {
            let value = key(cp);
            if best.is_none_or(|(_, best_value)| replaces(value, best_value)) {
                Some((position, value))
            } else {
                best
            }
        })
        .map(|(position, _)| position)
}

#[cfg(test)]
mod tests {
    use super::first_pass_pvalue;
    use crate::tests::assert_relative_eq;

    #[test]
    fn first_pass_relaxes_the_pvalue() {
        assert_relative_eq(first_pass_pvalue(0.001), 0.01);
        assert_relative_eq(first_pass_pvalue(0.049), 0.49);
        assert_relative_eq(first_pass_pvalue(0.05), 0.1);
        assert_relative_eq(first_pass_pvalue(0.49), 0.98);
        assert_relative_eq(first_pass_pvalue(0.5), 0.5);
        assert_relative_eq(first_pass_pvalue(0.9), 0.9);
    }
}
