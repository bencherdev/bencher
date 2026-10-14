use std::{fmt, str::FromStr};

use crate::ValidError;

const MAX_LEN: usize = 1 << 10;

/// The `state` an OAuth client sends with an authorization request, returned to it unchanged.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct OAuthRequestState(String);

#[cfg(feature = "db")]
crate::typed_string!(OAuthRequestState);

impl FromStr for OAuthRequestState {
    type Err = ValidError;

    fn from_str(state: &str) -> Result<Self, Self::Err> {
        if state.len() <= MAX_LEN {
            Ok(Self(state.to_owned()))
        } else {
            Err(ValidError::OAuthRequestState)
        }
    }
}

impl fmt::Display for OAuthRequestState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for OAuthRequestState {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_LEN, OAuthRequestState};

    #[test]
    fn state_is_at_most_one_kibibyte() {
        "a".repeat(MAX_LEN).parse::<OAuthRequestState>().unwrap();
        "a".repeat(MAX_LEN + 1)
            .parse::<OAuthRequestState>()
            .unwrap_err();
    }
}
