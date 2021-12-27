use std::fmt;

use crate::ticker::TickerSymbol;

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct Fund {
    ticker_symbol: TickerSymbol,
    price: u64,
    expense_ratio: u64,
}

impl fmt::Display for Fund {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.ticker_symbol.fmt(f)
    }
}

impl Fund {
    // TODO reach out and get these prices
    pub fn new(ticker_symbol: TickerSymbol) -> Self {
        Self {
            ticker_symbol,
            price: 5555,
            expense_ratio: 5,
        }
    }

    pub fn ticker_symbol(&self) -> &TickerSymbol {
        &self.ticker_symbol
    }

    pub fn price(&self) -> u64 {
        self.price
    }

    pub fn expense_ratio(&self) -> u64 {
        self.expense_ratio
    }
}
