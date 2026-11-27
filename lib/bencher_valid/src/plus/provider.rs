//! What a sign-in with GitHub or Google keeps between its start and its finish: the state its
//! round trip carries, the PKCE verifier behind its challenge, and Google's nonce; and the account
//! the provider names.

use std::str::FromStr;

use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ValidError;

/// The longest account ID a provider names: Google's `sub` is at most 255 ASCII characters.
const ACCOUNT_ID_MAX_LEN: usize = 255;

/// The diesel impls of a secret kept in a column, which never prints it.
#[cfg(feature = "db")]
macro_rules! secret_column {
    ($name:ident) => {
        impl<DB> diesel::serialize::ToSql<diesel::sql_types::Text, DB> for $name
        where
            DB: diesel::backend::Backend,
            for<'a> String: diesel::serialize::ToSql<diesel::sql_types::Text, DB>
                + Into<
                    <DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer,
                >,
        {
            fn to_sql<'b>(
                &'b self,
                out: &mut diesel::serialize::Output<'b, '_, DB>,
            ) -> diesel::serialize::Result {
                out.set_value(self.0.clone());
                Ok(diesel::serialize::IsNull::No)
            }
        }

        impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Text, DB> for $name
        where
            DB: diesel::backend::Backend,
            String: diesel::deserialize::FromSql<diesel::sql_types::Text, DB>,
        {
            fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
                String::from_sql(bytes)?
                    .as_str()
                    .parse()
                    .map_err(Into::into)
            }
        }
    };
}

/// The state a provider's round trip carries, which finds the sign-in's row.
#[typeshare::typeshare]
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
pub struct ProviderState(String);

crate::keys::secret_impl!(ProviderState, error = ProviderState);

/// The stored hash of a [`ProviderState`].
#[derive(Display, Debug, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(
    feature = "db",
    derive(diesel::FromSqlRow, diesel::AsExpression),
    diesel(sql_type = diesel::sql_types::Text)
)]
pub struct ProviderStateHash(String);

crate::keys::api_key_hash_impl!(ProviderStateHash, ProviderState, error = ProviderStateHash);

/// The PKCE verifier behind a sign-in's `S256` challenge, which only the server holds until the
/// exchange it proves.
#[derive(Clone, Eq, PartialEq)]
#[cfg_attr(
    feature = "db",
    derive(diesel::FromSqlRow, diesel::AsExpression),
    diesel(sql_type = diesel::sql_types::Text)
)]
pub struct PkceVerifier(String);

crate::keys::secret_impl!(PkceVerifier, error = PkceVerifier);
#[cfg(feature = "db")]
secret_column!(PkceVerifier);

/// The nonce a Google sign-in asks Google to put in its ID token.
#[derive(Clone, Eq, PartialEq)]
#[cfg_attr(
    feature = "db",
    derive(diesel::FromSqlRow, diesel::AsExpression),
    diesel(sql_type = diesel::sql_types::Text)
)]
pub struct ProviderNonce(String);

crate::keys::secret_impl!(ProviderNonce, error = ProviderNonce);
#[cfg(feature = "db")]
secret_column!(ProviderNonce);

/// The ID a provider gives an account, which never changes: GitHub's user ID, or Google's `sub`.
#[derive(Debug, Display, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct ProviderAccountId(String);

#[cfg(feature = "db")]
crate::typed_string!(ProviderAccountId);

impl FromStr for ProviderAccountId {
    type Err = ValidError;

    fn from_str(id: &str) -> Result<Self, Self::Err> {
        if (1..=ACCOUNT_ID_MAX_LEN).contains(&id.len()) && id.bytes().all(|b| b.is_ascii_graphic())
        {
            Ok(Self(id.to_owned()))
        } else {
            Err(ValidError::ProviderAccountId(id.to_owned()))
        }
    }
}

impl From<u64> for ProviderAccountId {
    fn from(id: u64) -> Self {
        Self(id.to_string())
    }
}

impl AsRef<str> for ProviderAccountId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

#[cfg(test)]
mod tests {
    use super::ProviderAccountId;

    #[test]
    fn account_id_is_1_to_255_visible_ascii_characters() {
        for id in ["1", "110169484474386276334", &"a".repeat(255)] {
            id.parse::<ProviderAccountId>().unwrap();
        }
        for id in ["", &"a".repeat(256), "1 2", "1\n", "é"] {
            assert!(id.parse::<ProviderAccountId>().is_err(), "{id:?}");
        }
    }
}
