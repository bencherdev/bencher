//! The secrets of a pending email sign-in: the binding secret its browser holds in a cookie,
//! the link secret its email carries, and its 6-digit code.

use std::{fmt, str::FromStr};

use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ValidError;

const CODE_LEN: usize = 6;
#[cfg(feature = "server")]
const CODES: u32 = 1_000_000;

/// The secret that binds a pending sign-in to the browser that started it, which only that
/// browser's cookie carries.
#[derive(Clone, Eq, PartialEq)]
pub struct BindingSecret(String);

crate::keys::secret_impl!(BindingSecret, error = BindingSecret);

/// The stored hash of a [`BindingSecret`].
#[derive(Display, Debug, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(
    feature = "db",
    derive(diesel::FromSqlRow, diesel::AsExpression),
    diesel(sql_type = diesel::sql_types::Text)
)]
pub struct BindingSecretHash(String);

crate::keys::api_key_hash_impl!(BindingSecretHash, BindingSecret, error = BindingSecretHash);

/// The secret a sign-in email's link carries in its fragment.
#[typeshare::typeshare]
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
pub struct LinkSecret(String);

crate::keys::secret_impl!(LinkSecret, error = LinkSecret);

/// The stored hash of a [`LinkSecret`].
#[derive(Display, Debug, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(
    feature = "db",
    derive(diesel::FromSqlRow, diesel::AsExpression),
    diesel(sql_type = diesel::sql_types::Text)
)]
pub struct LinkSecretHash(String);

crate::keys::api_key_hash_impl!(LinkSecretHash, LinkSecret, error = LinkSecretHash);

/// The 6-digit code a sign-in email carries.
#[typeshare::typeshare]
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
pub struct SignInCode(String);

impl SignInCode {
    /// A code drawn uniformly from all 1,000,000.
    #[cfg(feature = "server")]
    pub fn generate() -> Self {
        use rand::RngExt as _;

        let code = rand::rng().random_range(0..CODES);
        Self(format!("{code:06}"))
    }
}

impl fmt::Debug for SignInCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SignInCode(******)")
    }
}

impl TryFrom<String> for SignInCode {
    type Error = ValidError;

    fn try_from(code: String) -> Result<Self, Self::Error> {
        if code.len() == CODE_LEN && code.bytes().all(|b| b.is_ascii_digit()) {
            Ok(Self(code))
        } else {
            Err(ValidError::SignInCode)
        }
    }
}

impl FromStr for SignInCode {
    type Err = ValidError;

    fn from_str(code: &str) -> Result<Self, Self::Err> {
        Self::try_from(code.to_owned())
    }
}

impl AsRef<str> for SignInCode {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// The stored keyed hash of a [`SignInCode`], which only the server's secret key can reproduce.
#[derive(Display, Debug, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(
    feature = "db",
    derive(diesel::FromSqlRow, diesel::AsExpression),
    diesel(sql_type = diesel::sql_types::Text)
)]
pub struct SignInCodeHash(String);

#[cfg(feature = "db")]
crate::typed_string!(SignInCodeHash);

impl SignInCodeHash {
    #[cfg(feature = "server")]
    pub fn from_digest(digest: &[u8]) -> Self {
        Self(hex::encode(digest))
    }
}

impl FromStr for SignInCodeHash {
    type Err = ValidError;

    fn from_str(hash: &str) -> Result<Self, Self::Err> {
        if crate::keys::is_valid_sha256_hex(hash) {
            Ok(Self(hash.to_owned()))
        } else {
            Err(ValidError::SignInCodeHash(hash.to_owned()))
        }
    }
}

impl AsRef<str> for SignInCodeHash {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::SignInCode;

    #[test]
    fn code_is_six_digits() {
        "000000".parse::<SignInCode>().unwrap();
        "123456".parse::<SignInCode>().unwrap();
        for code in [
            "",
            "12345",
            "1234567",
            "12345a",
            " 12345",
            "12 456",
            "１２３４５６",
        ] {
            assert!(code.parse::<SignInCode>().is_err(), "{code}");
        }
    }

    #[cfg(feature = "server")]
    #[test]
    fn generated_codes_are_valid() {
        for _ in 0..1000 {
            let code = SignInCode::generate();
            assert_eq!(
                code.as_ref().parse::<SignInCode>().unwrap(),
                code,
                "a generated code parses back"
            );
        }
    }
}
