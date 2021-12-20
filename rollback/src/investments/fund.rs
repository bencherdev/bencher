use crate::investments::ticker::TickerSymbol;

pub struct Fund {
    kind: FundKind,
    ticker_symbol: TickerSymbol,
    price: u64,
    expense_ratio: u64,
}

impl Fund {
    pub fn price(&self) -> u64 {
        self.price
    }
}

pub enum FundKind {
    Mutual,
    Index,
    TargetDate,
}
