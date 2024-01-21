use crate::IqrBoundary;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quartiles {
    pub q1: f64,
    pub q2: f64,
    pub q3: f64,
}

impl Quartiles {
    #[allow(clippy::indexing_slicing, clippy::integer_division)]
    pub fn new(data: &[f64]) -> Option<Self> {
        if data.is_empty() {
            return None;
        }

        let mut data = data.to_vec();
        data.sort_unstable_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));

        Some(Self {
            q1: Self::q1(&data)?,
            q2: Self::q2(&data)?,
            q3: Self::q3(&data)?,
        })
    }

    pub fn new_delta(data: &[f64]) -> Option<Self> {
        // Do not calculate delta inter-quartile range if there are less than 2 data points
        if data.len() < 2 {
            return None;
        }

        let deltas = Self::percent_changes(data);

        Some(Self {
            q1: Self::q1(&deltas)?,
            q2: Self::q2(&deltas)?,
            q3: Self::q3(&deltas)?,
        })
    }

    pub fn iqr(self, boundary: IqrBoundary) -> f64 {
        (self.q3 - self.q1) * f64::from(boundary)
    }

    fn q1(data: &[f64]) -> Option<f64> {
        Self::percentile_of_sorted(data, 0.25).and_then(|q| q.is_finite().then_some(q))
    }

    fn q2(data: &[f64]) -> Option<f64> {
        Self::percentile_of_sorted(data, 0.50).and_then(|q| q.is_finite().then_some(q))
    }

    fn q3(data: &[f64]) -> Option<f64> {
        Self::percentile_of_sorted(data, 0.75).and_then(|q| q.is_finite().then_some(q))
    }

    #[allow(
        clippy::cast_possible_truncation,
        clippy::cast_precision_loss,
        clippy::cast_sign_loss,
        clippy::indexing_slicing
    )]
    // https://doc.rust-lang.org/1.75.0/src/test/stats.rs.html#260
    fn percentile_of_sorted(sorted_data: &[f64], percentile: f64) -> Option<f64> {
        if sorted_data.is_empty() || !(0.0..=1.0).contains(&percentile) {
            None
        } else if sorted_data.len() == 1 {
            sorted_data.first().copied()
        } else if (percentile - 1.0).abs() < 0.001 {
            sorted_data.last().copied()
        } else {
            let length = (sorted_data.len() - 1) as f64;
            let rank = percentile * length;
            let floor_rank = rank.floor();
            let rank_delta = rank - floor_rank;
            let index = floor_rank as usize;
            let floor = sorted_data[index];
            let ceil = sorted_data[index + 1];
            Some(floor + (ceil - floor) * rank_delta)
        }
    }

    // The percent change of the absolute deltas between adjacent results
    // sorted from smallest delta to largest
    // https://github.com/aochagavia/rustls-bench-app/blob/c1b31a018d98547e201867b9b71df1c23e55b95c/ci-bench-runner/src/job/bench_pr.rs#L398
    // https://github.com/rust-lang/rustc-perf/blob/4f313add609f43e928e98132358e8426ed3969ae/site/src/comparison.rs#L1219
    fn percent_changes(data: &[f64]) -> Vec<f64> {
        #[allow(clippy::indexing_slicing)]
        let mut changes = data
            .windows(2)
            .map(|window| Self::abs_percent_change(window[0], window[1]))
            .collect::<Vec<_>>();
        changes.sort_unstable_by(|l, r| l.partial_cmp(r).unwrap_or(std::cmp::Ordering::Equal));
        changes
    }

    fn abs_percent_change(baseline: f64, next: f64) -> f64 {
        // Always return a positive percentage
        ((next - baseline) / baseline).abs()
    }
}

#[cfg(test)]
#[allow(clippy::float_cmp, clippy::unreadable_literal, clippy::unwrap_used)]
mod test {
    use once_cell::sync::Lazy;
    use pretty_assertions::assert_eq;

    use crate::IqrBoundary;

    use super::Quartiles;

    const DATA_ZERO: &[f64] = &[];
    const DATA_ONE: &[f64] = &[1.0];
    const DATA_TWO: &[f64] = &[1.0, 2.0];
    const DATA_THREE: &[f64] = &[1.0, 2.0, 3.0];
    const DATA_FIVE: &[f64] = &[1.0, 2.0, 3.0, 4.0, 5.0];
    const DATA_FIVE_DESC: &[f64] = &[5.0, 4.0, 3.0, 2.0, 1.0];
    const DATA_FIVE_NEG: &[f64] = &[-1.0, -2.0, -3.0, -4.0, -5.0];
    const DATA_FIVE_CONST: &[f64] = &[1.0, 1.0, 1.0, 1.0, 1.0];
    const DATA_SIX_BI: &[f64] = &[-6.0, -6.0, 1.0, 1.0, 9.0, 9.0];

    static ZERO_BOUNDARY: Lazy<IqrBoundary> = Lazy::new(|| 0.0.try_into().unwrap());
    static ONE_BOUNDARY: Lazy<IqrBoundary> = Lazy::new(|| 1.0.try_into().unwrap());
    static TWO_BOUNDARY: Lazy<IqrBoundary> = Lazy::new(|| 2.0.try_into().unwrap());
    static THREE_BOUNDARY: Lazy<IqrBoundary> = Lazy::new(|| 3.0.try_into().unwrap());
    static FIVE_BOUNDARY: Lazy<IqrBoundary> = Lazy::new(|| 5.0.try_into().unwrap());

    #[test]
    fn test_quartiles_zero() {
        let q = Quartiles::new(DATA_ZERO);
        assert_eq!(q, None);
    }

    #[test]
    fn test_quartiles_one() {
        let q = Quartiles::new(DATA_ONE).unwrap();
        assert_eq!(
            q,
            Quartiles {
                q1: 1.0,
                q2: 1.0,
                q3: 1.0
            }
        );

        assert_eq!(q.iqr(*ZERO_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*ONE_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*TWO_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*THREE_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*FIVE_BOUNDARY), 0.0);
    }

    #[test]
    fn test_quartiles_two() {
        let q = Quartiles::new(DATA_TWO).unwrap();
        assert_eq!(
            q,
            Quartiles {
                q1: 1.25,
                q2: 1.5,
                q3: 1.75
            }
        );

        assert_eq!(q.iqr(*ZERO_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*ONE_BOUNDARY), 0.5);
        assert_eq!(q.iqr(*TWO_BOUNDARY), 1.0);
        assert_eq!(q.iqr(*THREE_BOUNDARY), 1.5);
        assert_eq!(q.iqr(*FIVE_BOUNDARY), 2.5);
    }

    #[test]
    fn test_quartiles_three() {
        let q = Quartiles::new(DATA_THREE).unwrap();
        assert_eq!(
            q,
            Quartiles {
                q1: 1.5,
                q2: 2.0,
                q3: 2.5
            }
        );

        assert_eq!(q.iqr(*ZERO_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*ONE_BOUNDARY), 1.0);
        assert_eq!(q.iqr(*TWO_BOUNDARY), 2.0);
        assert_eq!(q.iqr(*THREE_BOUNDARY), 3.0);
        assert_eq!(q.iqr(*FIVE_BOUNDARY), 5.0);
    }

    #[test]
    fn test_quartiles_five() {
        let q = Quartiles::new(DATA_FIVE).unwrap();
        assert_eq!(
            q,
            Quartiles {
                q1: 2.0,
                q2: 3.0,
                q3: 4.0
            }
        );

        assert_eq!(q.iqr(*ZERO_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*ONE_BOUNDARY), 2.0);
        assert_eq!(q.iqr(*TWO_BOUNDARY), 4.0);
        assert_eq!(q.iqr(*THREE_BOUNDARY), 6.0);
        assert_eq!(q.iqr(*FIVE_BOUNDARY), 10.0);
    }

    #[test]
    fn test_quartiles_five_desc() {
        let q = Quartiles::new(DATA_FIVE_DESC).unwrap();
        assert_eq!(
            q,
            Quartiles {
                q1: 2.0,
                q2: 3.0,
                q3: 4.0
            }
        );

        assert_eq!(q.iqr(*ZERO_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*ONE_BOUNDARY), 2.0);
        assert_eq!(q.iqr(*TWO_BOUNDARY), 4.0);
        assert_eq!(q.iqr(*THREE_BOUNDARY), 6.0);
        assert_eq!(q.iqr(*FIVE_BOUNDARY), 10.0);
    }

    #[test]
    fn test_quartiles_five_neg() {
        let q = Quartiles::new(DATA_FIVE_NEG).unwrap();
        assert_eq!(
            q,
            Quartiles {
                q1: -4.0,
                q2: -3.0,
                q3: -2.0
            }
        );

        assert_eq!(q.iqr(*ZERO_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*ONE_BOUNDARY), 2.0);
        assert_eq!(q.iqr(*TWO_BOUNDARY), 4.0);
        assert_eq!(q.iqr(*THREE_BOUNDARY), 6.0);
        assert_eq!(q.iqr(*FIVE_BOUNDARY), 10.0);
    }

    #[test]
    fn test_quartiles_five_const() {
        let q = Quartiles::new(DATA_FIVE_CONST).unwrap();
        assert_eq!(
            q,
            Quartiles {
                q1: 1.0,
                q2: 1.0,
                q3: 1.0
            }
        );

        assert_eq!(q.iqr(*ZERO_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*ONE_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*TWO_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*THREE_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*FIVE_BOUNDARY), 0.0);
    }

    #[test]
    fn test_quartiles_six_bi() {
        let q = Quartiles::new(DATA_SIX_BI).unwrap();
        assert_eq!(
            q,
            Quartiles {
                q1: -4.25,
                q2: 1.0,
                q3: 7.0
            }
        );

        assert_eq!(q.iqr(*ZERO_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*ONE_BOUNDARY), 11.25);
        assert_eq!(q.iqr(*TWO_BOUNDARY), 22.5);
        assert_eq!(q.iqr(*THREE_BOUNDARY), 33.75);
        assert_eq!(q.iqr(*FIVE_BOUNDARY), 56.25);
    }

    #[test]
    fn test_delta_quartiles_zero() {
        let q = Quartiles::new_delta(DATA_ZERO);
        assert_eq!(q, None);
    }

    #[test]
    fn test_delta_quartiles_one() {
        let q = Quartiles::new_delta(DATA_ONE);
        assert_eq!(q, None);
    }

    #[test]
    fn test_delta_quartiles_two() {
        let q = Quartiles::new_delta(DATA_TWO).unwrap();
        assert_eq!(
            q,
            Quartiles {
                q1: 1.0,
                q2: 1.0,
                q3: 1.0
            }
        );

        assert_eq!(q.iqr(*ZERO_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*ONE_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*TWO_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*THREE_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*FIVE_BOUNDARY), 0.0);
    }

    #[test]
    fn test_delta_quartiles_three() {
        let q = Quartiles::new_delta(DATA_THREE).unwrap();
        assert_eq!(
            q,
            Quartiles {
                q1: 0.625,
                q2: 0.75,
                q3: 0.875
            }
        );

        assert_eq!(q.iqr(*ZERO_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*ONE_BOUNDARY), 0.25);
        assert_eq!(q.iqr(*TWO_BOUNDARY), 0.5);
        assert_eq!(q.iqr(*THREE_BOUNDARY), 0.75);
        assert_eq!(q.iqr(*FIVE_BOUNDARY), 1.25);
    }

    #[test]
    fn test_delta_quartiles_five() {
        let q = Quartiles::new_delta(DATA_FIVE).unwrap();
        assert_eq!(
            q,
            Quartiles {
                q1: 0.3125,
                q2: 0.41666666666666663,
                q3: 0.625
            }
        );

        assert_eq!(q.iqr(*ZERO_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*ONE_BOUNDARY), 0.3125);
        assert_eq!(q.iqr(*TWO_BOUNDARY), 0.625);
        assert_eq!(q.iqr(*THREE_BOUNDARY), 0.9375);
        assert_eq!(q.iqr(*FIVE_BOUNDARY), 1.5625);
    }

    #[test]
    fn test_delta_quartiles_five_desc() {
        let q = Quartiles::new_delta(DATA_FIVE_DESC).unwrap();
        assert_eq!(
            q,
            Quartiles {
                q1: 0.2375,
                q2: 0.29166666666666663,
                q3: 0.375
            }
        );

        assert_eq!(q.iqr(*ZERO_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*ONE_BOUNDARY), 0.1375);
        assert_eq!(q.iqr(*TWO_BOUNDARY), 0.275);
        assert_eq!(q.iqr(*THREE_BOUNDARY), 0.41250000000000003);
        assert_eq!(q.iqr(*FIVE_BOUNDARY), 0.6875);
    }

    #[test]
    fn test_delta_quartiles_five_neg() {
        let q = Quartiles::new_delta(DATA_FIVE_NEG).unwrap();
        assert_eq!(
            q,
            Quartiles {
                q1: 0.3125,
                q2: 0.41666666666666663,
                q3: 0.625
            }
        );

        assert_eq!(q.iqr(*ZERO_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*ONE_BOUNDARY), 0.3125);
        assert_eq!(q.iqr(*TWO_BOUNDARY), 0.625);
        assert_eq!(q.iqr(*THREE_BOUNDARY), 0.9375);
        assert_eq!(q.iqr(*FIVE_BOUNDARY), 1.5625);
    }

    #[test]
    fn test_delta_quartiles_five_const() {
        let q = Quartiles::new_delta(DATA_FIVE_CONST).unwrap();
        assert_eq!(
            q,
            Quartiles {
                q1: 0.0,
                q2: 0.0,
                q3: 0.0
            }
        );

        assert_eq!(q.iqr(*ZERO_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*ONE_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*TWO_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*THREE_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*FIVE_BOUNDARY), 0.0);
    }

    #[test]
    fn test_delta_quartiles_six_bi() {
        let q = Quartiles::new_delta(DATA_SIX_BI).unwrap();
        assert_eq!(
            q,
            Quartiles {
                q1: 0.0,
                q2: 0.0,
                q3: 1.1666666666666667
            }
        );

        assert_eq!(q.iqr(*ZERO_BOUNDARY), 0.0);
        assert_eq!(q.iqr(*ONE_BOUNDARY), 1.1666666666666667);
        assert_eq!(q.iqr(*TWO_BOUNDARY), 2.3333333333333335);
        assert_eq!(q.iqr(*THREE_BOUNDARY), 3.5);
        assert_eq!(q.iqr(*FIVE_BOUNDARY), 5.833333333333334);
    }
}
