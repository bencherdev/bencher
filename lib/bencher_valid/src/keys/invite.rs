//! The secret an invitation's email carries in its link's fragment.

#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// The secret an invitation's link carries in its fragment, which names the invitation.
#[typeshare::typeshare]
#[derive(Clone, Eq, PartialEq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
pub struct InviteSecret(String);

crate::keys::secret_impl!(InviteSecret, error = InviteSecret);

/// The stored hash of an [`InviteSecret`].
#[derive(derive_more::Display, Debug, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(
    feature = "db",
    derive(diesel::FromSqlRow, diesel::AsExpression),
    diesel(sql_type = diesel::sql_types::Text)
)]
pub struct InviteSecretHash(String);

crate::keys::api_key_hash_impl!(InviteSecretHash, InviteSecret, error = InviteSecretHash);
