use std::collections::BTreeMap;
use std::fmt;

use crate::holding::Holding;
use crate::ticker::TickerSymbol;
use crate::total::Total;

/// Accounts, stored by `AccountId`
pub type Accounts = BTreeMap<AccountId, Account>;

impl Total for Accounts {
    fn total(&self) -> u64 {
        self.values().fold(0, |acc, account| acc + account.total())
    }
}

/// An account with holdings stored by `TickerSymbol`
#[derive(Clone, Hash, Eq, PartialEq)]
pub struct Account {
    id: AccountId,
    kind: AccountKind,
    holdings: BTreeMap<TickerSymbol, Holding>,
}

impl Account {
    pub fn new(id: String, kind: AccountKind) -> Self {
        Self {
            id,
            kind,
            holdings: BTreeMap::new(),
        }
    }

    pub fn id(&self) -> &AccountId {
        &self.id
    }

    pub fn kind(&self) -> &AccountKind {
        &self.kind
    }

    pub fn holdings(&self) -> &BTreeMap<TickerSymbol, Holding> {
        &self.holdings
    }

    pub fn update_kind(&mut self, kind: AccountKind) {
        self.kind = kind;
    }

    pub fn add_holding(&mut self, ticker_symbol: TickerSymbol, shares: u64) {
        let holding = Holding::new(ticker_symbol.clone(), shares);
        self.holdings.insert(ticker_symbol, holding);
    }

    pub fn update_holding(&mut self, ticker_symbol: &TickerSymbol, shares: u64) -> Option<u64> {
        if let Some(holding) = self.holdings.get_mut(&ticker_symbol) {
            holding.set_shares(shares);
            Some(holding.shares())
        } else {
            None
        }
    }

    pub fn remove_holding(&mut self, ticker_symbol: &TickerSymbol) -> Option<Holding> {
        self.holdings.remove(ticker_symbol)
    }
}

impl fmt::Display for Account {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.kind)
    }
}

impl Total for Account {
    fn total(&self) -> u64 {
        self.holdings
            .iter()
            .fold(0, |acc, (_, inv)| acc + inv.total())
    }
}

pub type AccountId = String;

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum AccountKind {
    Brokerage,
    IRA(IraKind),
    DefinedContribution(DefinedContributionPlan),
}

impl fmt::Display for AccountKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AccountKind::Brokerage => write!(f, "Brokerage"),
            AccountKind::IRA(ira_kind) => ira_kind.fmt(f),
            AccountKind::DefinedContribution(dc_plan) => dc_plan.fmt(f),
        }
    }
}

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum IraKind {
    Traditional,
    Roth,
    SEP,
    SIMPLE,
    Conduit,
}

impl fmt::Display for IraKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} IRA",
            match self {
                IraKind::Traditional => "Traditional",
                IraKind::Roth => "Roth",
                IraKind::SEP => "SEP",
                IraKind::SIMPLE => "SIMPLE",
                IraKind::Conduit => "Conduit",
            }
        )
    }
}

#[derive(Clone, Hash, Eq, PartialEq)]
pub struct DefinedContributionPlan {
    company: String,
    kind: DcKind,
}

impl fmt::Display for DefinedContributionPlan {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.company, self.kind)
    }
}

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum DcKind {
    Dc401k(Dc401kKind),
    Dc403b(BucketKind),
    Dc457b(BucketKind),
    ProfitSharing,
    MoneyPurchase,
    DefinedBenefit,
}

impl fmt::Display for DcKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DcKind::Dc401k(dc401k_kind) => dc401k_kind.fmt(f),
            DcKind::Dc403b(bucket_kind) => write!(f, "{} 403(b)", bucket_kind),
            DcKind::Dc457b(bucket_kind) => write!(f, "{} 457(b)", bucket_kind),
            DcKind::ProfitSharing => write!(f, "Profit Sharing"),
            DcKind::MoneyPurchase => write!(f, "Money Purchase"),
            DcKind::DefinedBenefit => write!(f, "Defined Benefit"),
        }
    }
}

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum Dc401kKind {
    Traditional(BucketKind),
    SafeHarbor(BucketKind),
    SIMPLE(BucketKind),
}

impl fmt::Display for Dc401kKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (dc401k_kind, bucket_kind) = match self {
            Dc401kKind::Traditional(bucket_kind) => ("", bucket_kind),
            Dc401kKind::SafeHarbor(bucket_kind) => ("Safe Harbor ", bucket_kind),
            Dc401kKind::SIMPLE(bucket_kind) => ("SIMPLE ", bucket_kind),
        };
        write!(f, "{}{} 401(k)", dc401k_kind, bucket_kind)
    }
}

#[derive(Clone, Hash, Eq, PartialEq)]
pub enum BucketKind {
    PreTax,
    Roth,
    AfterTax,
}

impl fmt::Display for BucketKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                BucketKind::PreTax => "Pre-Tax",
                BucketKind::Roth => "Roth",
                BucketKind::AfterTax => "After-Tax",
            }
        )
    }
}
