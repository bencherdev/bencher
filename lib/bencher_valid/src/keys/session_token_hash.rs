use derive_more::Display;

/// The stored hash of a [`crate::SessionToken`].
#[derive(Display, Debug, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(
    feature = "db",
    derive(diesel::FromSqlRow, diesel::AsExpression),
    diesel(sql_type = diesel::sql_types::Text)
)]
pub struct SessionTokenHash(String);

crate::keys::api_key_hash_impl!(SessionTokenHash, SessionToken, error = SessionTokenHash);
