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

impl Rank {
    pub fn is_sorted(ranks: &[Rank]) -> bool {
        #[allow(clippy::indexing_slicing)]
        ranks.windows(2).all(|w| {
            assert!(w.len() == 2, "window size is not 2");
            w[0] <= w[1]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_sorted() {
        let ranks = vec![Rank(1), Rank(2), Rank(3)];
        assert!(Rank::is_sorted(&ranks));

        let ranks = vec![Rank(1), Rank(3), Rank(2)];
        assert!(!Rank::is_sorted(&ranks));
    }
}
