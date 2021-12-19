use crate::investments::fund::Fund;
use crate::investments::total::Total;

pub struct Investment {
    fund: Fund,
    shares: u64,
}

impl Total for Investment {
    fn total(&self) -> u64 {
        self.fund.price() * self.shares
    }
}
