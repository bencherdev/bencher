use crate::Rank;

pub struct RankGenerator {
    offset: i64,
    len: usize,
    count: usize,
}

impl RankGenerator {
    pub fn new(len: usize) -> Self {
        // This will provide equal spacing between ranks, including at the beginning and end.
        #[allow(clippy::cast_possible_wrap, clippy::integer_division)]
        let offset = i64::MAX / (len + 1) as i64;
        Self {
            offset,
            len,
            count: 0,
        }
    }
}

impl Iterator for RankGenerator {
    type Item = Rank;

    #[allow(clippy::cast_possible_wrap)]
    fn next(&mut self) -> Option<Self::Item> {
        if self.count >= self.len {
            return None;
        }

        let rank = self.offset * (self.count + 1) as i64;
        self.count += 1;

        Some(Rank(rank))
    }
}

#[cfg(test)]
mod test {
    use crate::Rank;

    use super::RankGenerator;

    #[test]
    #[allow(clippy::decimal_literal_representation)]
    fn test_rank_generator() {
        let mut generator = RankGenerator::new(5);
        assert_eq!(generator.next(), Some(Rank(1_537_228_672_809_129_301)));
        assert_eq!(generator.next(), Some(Rank(3_074_457_345_618_258_602)));
        assert_eq!(generator.next(), Some(Rank(4_611_686_018_427_387_903)));
        assert_eq!(generator.next(), Some(Rank(6_148_914_691_236_517_204)));
        assert_eq!(generator.next(), Some(Rank(7_686_143_364_045_646_505)));
        assert_eq!(generator.next(), None);
    }

    #[test]
    fn test_rank_generator_zero() {
        let mut generator = RankGenerator::new(0);
        assert_eq!(generator.next(), None);
    }
}
