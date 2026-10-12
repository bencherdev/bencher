use std::{fmt, str::FromStr};

use crate::ValidError;

const MAX_LEN: usize = 1 << 8;

/// The `nonce` an OAuth client sends with an authorization request, for its ID token.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct OAuthNonce(String);

#[cfg(feature = "db")]
crate::typed_string!(OAuthNonce);

impl FromStr for OAuthNonce {
    type Err = ValidError;

    fn from_str(nonce: &str) -> Result<Self, Self::Err> {
        if nonce.len() <= MAX_LEN {
            Ok(Self(nonce.to_owned()))
        } else {
            Err(ValidError::OAuthNonce)
        }
    }
}

impl fmt::Display for OAuthNonce {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for OAuthNonce {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_LEN, OAuthNonce};

    #[test]
    fn nonce_is_at_most_256_bytes() {
        "a".repeat(MAX_LEN).parse::<OAuthNonce>().unwrap();
        "a".repeat(MAX_LEN + 1).parse::<OAuthNonce>().unwrap_err();
    }
}
