use std::{fmt, str::FromStr};

use crate::ValidError;

/// The length of an unpadded base64url SHA-256 digest.
const LEN: usize = 43;

/// A PKCE `S256` code challenge: the unpadded base64url SHA-256 of the client's code verifier.
#[derive(Debug, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct OAuthCodeChallenge(String);

#[cfg(feature = "db")]
crate::typed_string!(OAuthCodeChallenge);

impl FromStr for OAuthCodeChallenge {
    type Err = ValidError;

    fn from_str(challenge: &str) -> Result<Self, Self::Err> {
        if challenge.len() == LEN
            && challenge
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_')
        {
            Ok(Self(challenge.to_owned()))
        } else {
            Err(ValidError::OAuthCodeChallenge)
        }
    }
}

impl fmt::Display for OAuthCodeChallenge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl AsRef<str> for OAuthCodeChallenge {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::OAuthCodeChallenge;

    #[test]
    fn challenge_is_an_unpadded_base64url_sha256() {
        "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
            .parse::<OAuthCodeChallenge>()
            .unwrap();
        "_-0123456789abcdefghijklmnopqrstuvwxyzABCDE"
            .parse::<OAuthCodeChallenge>()
            .unwrap();
        for challenge in [
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-c",
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cMA",
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw+cM",
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw/cM",
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-c=",
        ] {
            challenge.parse::<OAuthCodeChallenge>().unwrap_err();
        }
    }
}
