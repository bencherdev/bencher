use url::Url;

use crate::investments::investment::Investment;
use crate::investments::total::Total;

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
