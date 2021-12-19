use url::Url;

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
    account_type: AccountType,
    investments: Vec<Investment>,
}

struct Investment {
    fund: Fund,
    shares: u64,
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
