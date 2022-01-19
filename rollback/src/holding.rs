use std::collections::BTreeMap;
use std::fmt;

use crate::fund::Fund;
use crate::ticker::TickerSymbol;
use crate::total::Total;
use crate::transaction::Transactions;

pub type Holdings = BTreeMap<TickerSymbol, Holding>;

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct Holding {
    fund: Fund,
    transactions: Transactions,
}

impl fmt::Display for Holding {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fund.fmt(f)
    }
}

impl Holding {
    pub fn new(ticker_symbol: TickerSymbol) -> Self {
        let fund = Fund::new(ticker_symbol);
        let transactions = Transactions::new();
        Self { fund, transactions }
    }

    pub fn fund(&self) -> &Fund {
        &self.fund
    }

    pub fn transactions(&self) -> &Transactions {
        &self.transactions
    }

    pub fn transactions_mut(&mut self) -> &mut Transactions {
        &mut self.transactions
    }
}

impl Total for Holding {
    fn total(&self) -> u64 {
        self.fund.price() * self.transactions.quantity()
    }
}
