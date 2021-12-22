use crate::investments::ticker::TickerSymbol;

pub struct Fund {
    ticker_symbol: TickerSymbol,
    price: u64,
    expense_ratio: u64,
}

impl Fund {
    pub fn new(ticker_symbol: TickerSymbol) -> Self {
        Self {
            ticker_symbol,
            price: 0,
            expense_ratio: 0,
        }
    }

    pub fn tickersymbol(&self) -> &TickerSymbol {
        &self.ticker_symbol
    }

    pub fn price(&self) -> u64 {
        self.price
    }

    pub fn expense_ratio(&self) -> u64 {
        self.expense_ratio
    }
}
