use std::collections::BTreeMap;
use std::fmt;

use crate::account::Accounts;
use crate::total::Total;
use url::Url;

/// An iterable map of all institutions
pub type Institutions = BTreeMap<Institution, Accounts>;

impl Total for Institutions {
    fn total(&self) -> u64 {
        self.iter()
            .fold(0, |acc, (_, inst_accs)| acc + inst_accs.total())
    }
}

/// An invesmtment institution
#[derive(Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct Institution {
    name: String,
    url: Url,
}

impl fmt::Display for Institution {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
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
