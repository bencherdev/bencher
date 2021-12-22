use crate::investments::account::Account;
use crate::investments::total::Total;
use url::Url;

pub struct Institutions {
    inst_accs: Vec<InstitutionAccounts>,
}

impl Institutions {
    pub fn new() -> Self {
        Self {
            inst_accs: Vec::new(),
        }
    }
}

impl Total for Institutions {
    fn total(&self) -> u64 {
        self.inst_accs
            .iter()
            .fold(0, |acc, account| acc + account.total())
    }
}

pub struct Institution {
    name: String,
    url: Url,
}

impl Institution {
    pub fn new(name: String, url: Url) -> Self {
        Self { name, url }
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn url(&self) -> &Url {
        &self.url
    }
}

pub struct InstitutionAccounts {
    institution: Institution,
    accounts: Vec<Account>,
}

impl InstitutionAccounts {
    pub fn new(institution: Institution) -> Self {
        Self {
            institution,
            accounts: Vec::new(),
        }
    }
}

impl Total for InstitutionAccounts {
    fn total(&self) -> u64 {
        self.accounts
            .iter()
            .fold(0, |acc, account| acc + account.total())
    }
}
