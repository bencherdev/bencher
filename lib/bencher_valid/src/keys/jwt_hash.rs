use derive_more::Display;

/// The stored hash of a deprecated user API token's [`crate::Jwt`].
#[derive(Display, Debug, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(
    feature = "db",
    derive(diesel::FromSqlRow, diesel::AsExpression),
    diesel(sql_type = diesel::sql_types::Text)
)]
pub struct JwtHash(String);

crate::keys::api_key_hash_impl!(JwtHash, Jwt, error = JwtHash);
