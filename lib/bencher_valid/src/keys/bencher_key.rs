use std::{fmt, str::FromStr};

use crate::{PROJECT_KEY_PREFIX, ProjectKey, Sanitize, USER_KEY_PREFIX, UserKey, ValidError};

/// A Bencher API key. Either a project-scoped key (`bencher_run_*`) or a
/// user-scoped key (`bencher_user_*`). The prefix disambiguates the two so the
/// CLI / SDK can accept a single `--key` / `BENCHER_API_KEY` slot for both.
#[derive(Clone, Eq, PartialEq, Hash)]
pub enum BencherKey {
    Project(ProjectKey),
    User(UserKey),
}

impl BencherKey {
    pub fn is_project(&self) -> bool {
        matches!(self, Self::Project(_))
    }

    pub fn is_user(&self) -> bool {
        matches!(self, Self::User(_))
    }
}

impl fmt::Debug for BencherKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(self, f)
    }
}

impl fmt::Display for BencherKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Project(key) => key.fmt(f),
            Self::User(key) => key.fmt(f),
        }
    }
}

impl Sanitize for BencherKey {
    fn sanitize(&mut self) {
        match self {
            Self::Project(key) => key.sanitize(),
            Self::User(key) => key.sanitize(),
        }
    }
}

impl AsRef<str> for BencherKey {
    fn as_ref(&self) -> &str {
        match self {
            Self::Project(key) => key.as_ref(),
            Self::User(key) => key.as_ref(),
        }
    }
}

impl From<ProjectKey> for BencherKey {
    fn from(key: ProjectKey) -> Self {
        Self::Project(key)
    }
}

impl From<UserKey> for BencherKey {
    fn from(key: UserKey) -> Self {
        Self::User(key)
    }
}

impl FromStr for BencherKey {
    type Err = ValidError;

    fn from_str(key: &str) -> Result<Self, Self::Err> {
        if key.starts_with(USER_KEY_PREFIX) {
            key.parse::<UserKey>().map(Self::User)
        } else if key.starts_with(PROJECT_KEY_PREFIX) {
            key.parse::<ProjectKey>().map(Self::Project)
        } else {
            Err(ValidError::BencherKey(key.to_owned()))
        }
    }
}

impl TryFrom<String> for BencherKey {
    type Error = ValidError;

    fn try_from(key: String) -> Result<Self, Self::Error> {
        key.parse()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    const VALID_PROJECT: &str = "bencher_run_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh";
    const VALID_USER: &str = "bencher_user_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh";

    #[test]
    fn parses_project() {
        let key: BencherKey = VALID_PROJECT.parse().unwrap();
        assert!(key.is_project());
        assert_eq!(key.as_ref(), VALID_PROJECT);
    }

    #[test]
    fn parses_user() {
        let key: BencherKey = VALID_USER.parse().unwrap();
        assert!(key.is_user());
        assert_eq!(key.as_ref(), VALID_USER);
    }

    #[test]
    fn rejects_unknown_prefix() {
        "bencher_other_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh"
            .parse::<BencherKey>()
            .unwrap_err();
        "".parse::<BencherKey>().unwrap_err();
    }

    #[test]
    fn rejects_malformed_known_prefix() {
        "bencher_user_short".parse::<BencherKey>().unwrap_err();
        "bencher_run_short".parse::<BencherKey>().unwrap_err();
    }
}
