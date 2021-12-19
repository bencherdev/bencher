use url::Url;

trait Total {
    fn total(&self) -> u64;
}

type Accounts = Vec<Account>;

impl Total for Accounts {
    fn total(&self) -> u64 {
        self.iter().fold(0, |acc, account| acc + account.total())
    }
}

struct Institution {
    name: String,
    url: Url,
}

enum AccountKind {
    Brokerage,
    IRA(IraKind),
    DefinedContribution(DcKind),
}

enum IraKind {
    Traditional,
    Roth,
    SEP,
    SIMPLE,
    Conduit,
}

enum DcKind {
    Dc401k(BucketKind),
    Dc403b(BucketKind),
    Dc457b(BucketKind),
    ProfitSharing,
    MoneyPurchase,
    DefinedBenefit,
}

enum BucketKind {
    PreTax,
    Roth,
    AfterTax,
}

struct Account {
    account_type: AccountKind,
    investments: Vec<Investment>,
}

impl Total for Account {
    fn total(&self) -> u64 {
        self.investments
            .iter()
            .fold(0, |acc, inv| acc + inv.total())
    }
}

struct Investment {
    fund: Fund,
    shares: u64,
}

impl Total for Investment {
    fn total(&self) -> u64 {
        self.fund.price * self.shares
    }
}

struct Fund {
    ticker_symbol: TickerSymbol,
    price: u64,
    expense_ratio: u64,
    fund_type: FundType,
}

enum StockExchange {
    NASDAQ,
    NYSE,
}

struct TickerSymbol {
    stock_exchange: StockExchange,
    symbol: String,
}

enum FundType {
    Mutual,
    Index,
    TargetDate,
}
