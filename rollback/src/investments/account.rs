use crate::investments::investment::Investment;
use crate::investments::ticker::TickerSymbol;
use crate::investments::total::Total;

pub struct Account {
    id: AccountId,
    kind: AccountKind,
    investments: Vec<Investment>,
}

impl Account {
    pub fn new(id: String, kind: AccountKind) -> Self {
        Self {
            id,
            kind,
            investments: Vec::new(),
        }
    }

    pub fn add_fund(&mut self, ticker_symbol: TickerSymbol) {
        let inv = Investment::new(ticker_symbol, 0);
        self.investments.push(inv);
    }
}

impl Total for Account {
    fn total(&self) -> u64 {
        self.investments
            .iter()
            .fold(0, |acc, inv| acc + inv.total())
    }
}

pub type AccountId = String;

pub enum AccountKind {
    Brokerage,
    IRA(IraKind),
    DefinedContribution(DcKind),
}

pub enum IraKind {
    Traditional,
    Roth,
    SEP,
    SIMPLE,
    Conduit,
}

pub enum DcKind {
    Dc401k(BucketKind),
    Dc403b(BucketKind),
    Dc457b(BucketKind),
    ProfitSharing,
    MoneyPurchase,
    DefinedBenefit,
}

pub enum BucketKind {
    PreTax,
    Roth,
    AfterTax,
}
