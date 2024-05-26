pub mod db;

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

impl Rank {
    pub fn is_sorted<R>(ranks: &[R]) -> bool
    where
        R: Ranked,
    {
        #[allow(clippy::indexing_slicing)]
        ranks.windows(2).all(|w| {
            assert!(w.len() == 2, "window size is not 2");
            w[0].rank() <= w[1].rank()
        })
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
    fn test_is_sorted() {
        let ranks = vec![TestRank::new(1), TestRank::new(2), TestRank::new(3)];
        assert!(Rank::is_sorted(&ranks));

        let ranks = vec![TestRank::new(1), TestRank::new(3), TestRank::new(2)];
        assert!(!Rank::is_sorted(&ranks));
    }
}
