//! The secret an email change's link carries in its fragment.

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The secret an email change's link carries in its fragment, which names the change.
#[typeshare::typeshare]
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
pub struct EmailChangeSecret(String);

crate::keys::secret_impl!(EmailChangeSecret, error = EmailChangeSecret);

/// The stored hash of an [`EmailChangeSecret`].
#[derive(derive_more::Display, Debug, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(
    feature = "db",
    derive(diesel::FromSqlRow, diesel::AsExpression),
    diesel(sql_type = diesel::sql_types::Text)
)]
pub struct EmailChangeSecretHash(String);

crate::keys::api_key_hash_impl!(
    EmailChangeSecretHash,
    EmailChangeSecret,
    error = EmailChangeSecretHash
);
