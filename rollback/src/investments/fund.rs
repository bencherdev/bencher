pub struct Fund {
    ticker_symbol: TickerSymbol,
    price: u64,
    expense_ratio: u64,
    fund_type: FundType,
}

impl Fund {
    pub fn price(&self) -> u64 {
        self.price
    }
}

pub enum StockExchange {
    NASDAQ,
    NYSE,
}

pub struct TickerSymbol {
    stock_exchange: StockExchange,
    symbol: String,
}

pub enum FundType {
    Mutual,
    Index,
    TargetDate,
}
