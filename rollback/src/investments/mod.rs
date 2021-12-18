use url::Url;

struct Institution {
    name: String,
    url: Url,
}

enum AccountType {
    Brokerage,
    IRA,
    DefinedContribution,
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
