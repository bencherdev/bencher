use std::collections::BTreeMap;
use std::fmt;

use crate::institution::Institutions;
use crate::total::Total;

/// A discrete user
#[derive(Clone, Hash, Eq, PartialEq, Ord, PartialOrd)]
pub struct User {
    first: String,
    last: String,
}

impl fmt::Display for User {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} {}", self.first, self.last)
    }
}

/// An iterable map of all portfolios
pub type Portfolios = BTreeMap<User, Portfolio>;

impl Total for Portfolios {
    fn total(&self) -> u64 {
        self.iter()
            .fold(0, |acc, (_, portfolio)| acc + portfolio.total())
    }
}

/// An invesmtment institution
#[derive(Clone)]
pub struct Portfolio {
    user: User,
    institutions: Institutions,
}

impl fmt::Display for Portfolio {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} Portfolio", self.user)
    }
}

impl Total for Portfolio {
    fn total(&self) -> u64 {
        self.institutions.total()
    }
}

impl Portfolio {
    pub fn new(user: User) -> Self {
        Self {
            user,
            institutions: Institutions::new(),
        }
    }

    pub fn user(&self) -> &User {
        &self.user
    }

    pub fn institutions(&self) -> &Institutions {
        &self.institutions
    }

    pub fn institutions_mut(&mut self) -> &mut Institutions {
        &mut self.institutions
    }
}
