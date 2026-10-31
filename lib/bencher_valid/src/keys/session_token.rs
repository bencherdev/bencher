use std::{fmt, str::FromStr};

use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

use crate::ValidError;

const TOKEN_BYTES: usize = 32;
/// The unpadded base64url length of [`TOKEN_BYTES`].
const TOKEN_LEN: usize = 43;

/// A browser session's secret, which only its cookie carries: 32 bytes from the operating
/// system's random number generator, base64url without padding.
#[derive(Clone, Eq, PartialEq)]
pub struct SessionToken(String);

impl SessionToken {
    #[cfg(feature = "server")]
    pub fn generate() -> Result<Self, rand::rngs::SysError> {
        use rand::TryRng as _;

        let mut bytes = [0; TOKEN_BYTES];
        rand::rngs::SysRng.try_fill_bytes(&mut bytes)?;
        Ok(Self(URL_SAFE_NO_PAD.encode(bytes)))
    }
}

impl fmt::Debug for SessionToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SessionToken(************)")
    }
}

impl FromStr for SessionToken {
    type Err = ValidError;

    fn from_str(token: &str) -> Result<Self, Self::Err> {
        if token.len() == TOKEN_LEN
            && URL_SAFE_NO_PAD
                .decode(token)
                .is_ok_and(|bytes| bytes.len() == TOKEN_BYTES)
        {
            Ok(Self(token.to_owned()))
        } else {
            Err(ValidError::SessionToken)
        }
    }
}

impl AsRef<str> for SessionToken {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
#[expect(clippy::string_slice, reason = "test tokens are ASCII")]
mod tests {
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

    use super::SessionToken;

    #[test]
    fn token_is_32_bytes_of_unpadded_base64url() {
        let valid = URL_SAFE_NO_PAD.encode([0xfb; 32]);
        valid.parse::<SessionToken>().unwrap();
        for token in [
            URL_SAFE_NO_PAD.encode([0xfb; 31]),
            URL_SAFE_NO_PAD.encode([0xfb; 33]),
            format!("{valid}="),
            valid.replace('-', "+").replace('_', "/"),
            // The last character's unused low bits must be zero.
            format!("{}B", &valid[..42]),
            format!(" {}", &valid[1..]),
        ] {
            assert!(token.parse::<SessionToken>().is_err(), "{token}");
        }
    }

    #[cfg(feature = "server")]
    #[test]
    fn generated_tokens_are_valid_and_distinct() {
        let token = SessionToken::generate().unwrap();
        assert_eq!(
            token.as_ref().parse::<SessionToken>().unwrap(),
            token,
            "a generated token parses back"
        );
        assert_ne!(token, SessionToken::generate().unwrap());
    }
}
