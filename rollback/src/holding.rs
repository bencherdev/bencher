use std::fmt;

use crate::fund::Fund;
use crate::ticker::TickerSymbol;
use crate::total::Total;

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct Holding {
    fund: Fund,
    // TODO shares should be derived/adjudicated by a Transactions type
    shares: u64,
}

impl fmt::Display for Holding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fund.fmt(f)
    }
}

impl Holding {
    pub fn new(ticker_symbol: TickerSymbol, shares: u64) -> Self {
        let fund = Fund::new(ticker_symbol);
        Self { fund, shares }
    }

    pub fn fund(&self) -> &Fund {
        &self.fund
    }

    pub fn shares(&self) -> u64 {
        self.shares
    }

    pub fn set_shares(&mut self, shares: u64) {
        self.shares = shares;
    }
}

impl Total for Holding {
    fn total(&self) -> u64 {
        self.fund.price() * self.shares
    }
}
