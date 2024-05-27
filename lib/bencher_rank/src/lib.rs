pub mod db;
mod generator;

pub use generator::RankGenerator;

#[allow(clippy::integer_division)]
const MID_RANK: i64 = i64::MAX / 2;

#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    PartialOrd,
    Ord,
    Hash,
    diesel::FromSqlRow,
    diesel::AsExpression,
)]
#[diesel(sql_type = diesel::sql_types::BigInt)]
pub struct Rank(i64);

pub trait Ranked {
    fn rank(&self) -> Rank;
}

impl From<u8> for Rank {
    fn from(rank: u8) -> Self {
        Self(i64::from(rank))
    }
}

impl Rank {
    /// Check if the ranks are sorted.
    pub fn is_sorted<R>(ranks: &[R]) -> bool
    where
        R: Ranked,
    {
        // The window size is 2.
        #[allow(clippy::indexing_slicing)]
        ranks.windows(2).all(|w| {
            assert!(w.len() == 2, "window size is not 2");
            w[0].rank() <= w[1].rank()
        })
    }

    /// Calculate the rank that would fit in between the surrounding `ranks` at `index`.
    pub fn calculate<R>(ranks: &[R], index: usize) -> Option<Rank>
    where
        R: Ranked,
    {
        if ranks.is_empty() {
            return Some(Rank(MID_RANK));
        }

        match index {
            0 => {
                let first = ranks.first()?.rank().0;
                // This is okay because we make sure that the new rank is less than the first rank.
                #[allow(clippy::integer_division)]
                let new_first = first / 2;
                if new_first < first {
                    return Some(Rank(new_first));
                }
            },
            _ if index >= ranks.len() => {
                let last = ranks.last()?.rank().0;
                // This is okay because we make sure that the new rank is greater than the last rank.
                #[allow(clippy::integer_division)]
                let new_last = last + ((i64::MAX - last) / 2);
                if new_last > last {
                    return Some(Rank(new_last));
                }
            },
            _ => {
                let prev_rank = ranks.get(index - 1)?.rank().0;
                let next_rank = ranks.get(index)?.rank().0;
                // This is okay because we make sure that the new rank is between the previous and next rank.
                #[allow(clippy::integer_division)]
                let new_rank = prev_rank + ((next_rank - prev_rank) / 2);
                if new_rank > prev_rank && new_rank < next_rank {
                    return Some(Rank(new_rank));
                }
            },
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use crate::{Rank, Ranked};

    struct TestRank {
        rank: Rank,
    }

    impl TestRank {
        fn new(rank: i64) -> Self {
            Self { rank: Rank(rank) }
        }
    }

    impl Ranked for TestRank {
        fn rank(&self) -> Rank {
            self.rank
        }
    }

    #[test]
    fn test_rank_is_sorted() {
        let ranks = vec![TestRank::new(1), TestRank::new(2), TestRank::new(3)];
        assert!(Rank::is_sorted(&ranks));

        let ranks = vec![TestRank::new(1), TestRank::new(3), TestRank::new(2)];
        assert!(!Rank::is_sorted(&ranks));
    }

    #[test]
    #[allow(clippy::decimal_literal_representation)]
    fn test_rank_calculate() {
        let ranks = vec![
            TestRank::new(0),
            TestRank::new(1),
            TestRank::new(2),
            TestRank::new(i64::MAX),
        ];
        assert_eq!(Rank::calculate(&ranks, 0), None);
        assert_eq!(Rank::calculate(&ranks, 1), None);
        assert_eq!(Rank::calculate(&ranks, 2), None);
        assert_eq!(
            Rank::calculate(&ranks, 3),
            Some(Rank(4_611_686_018_427_387_904))
        );
        assert_eq!(Rank::calculate(&ranks, 4), None);

        let ranks = vec![
            TestRank::new(1),
            TestRank::new(3),
            TestRank::new(5),
            TestRank::new(6),
        ];
        assert_eq!(Rank::calculate(&ranks, 0), Some(Rank(0)));
        assert_eq!(Rank::calculate(&ranks, 1), Some(Rank(2)));
        assert_eq!(Rank::calculate(&ranks, 2), Some(Rank(4)));
        assert_eq!(Rank::calculate(&ranks, 3), None);
        assert_eq!(
            Rank::calculate(&ranks, 4),
            Some(Rank(4_611_686_018_427_387_906))
        );

        let ranks = vec![TestRank::new(4)];
        assert_eq!(Rank::calculate(&ranks, 0), Some(Rank(2)));
        assert_eq!(
            Rank::calculate(&ranks, 1),
            Some(Rank(4_611_686_018_427_387_905))
        );
        assert_eq!(
            Rank::calculate(&ranks, 2),
            Some(Rank(4_611_686_018_427_387_905))
        );
    }
}
