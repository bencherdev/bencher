use crate::fund::Fund;
use crate::ticker::TickerSymbol;
use crate::total::Total;

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct Investment {
    fund: Fund,
    shares: u64,
}

impl Investment {
    pub fn new(ticker_symbol: TickerSymbol, shares: u64) -> Self {
        let fund = Fund::new(ticker_symbol);
        Self { fund, shares }
    }

    pub fn shares(&self) -> u64 {
        self.shares
    }

    pub fn set_shares(&mut self, shares: u64) {
        self.shares = shares;
    }
}

impl Total for Investment {
    fn total(&self) -> u64 {
        self.fund.price() * self.shares
    }
}
