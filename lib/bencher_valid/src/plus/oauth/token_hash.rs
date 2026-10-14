use derive_more::Display;

/// The stored hash of an OAuth access or refresh token.
#[derive(Display, Debug, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(
    feature = "db",
    derive(diesel::FromSqlRow, diesel::AsExpression),
    diesel(sql_type = diesel::sql_types::Text)
)]
pub struct OAuthTokenHash(String);

crate::keys::api_key_hash_impl!(OAuthTokenHash, OAuthAccessToken, error = OAuthTokenHash);

#[cfg(feature = "server")]
impl From<&crate::OAuthRefreshToken> for OAuthTokenHash {
    fn from(token: &crate::OAuthRefreshToken) -> Self {
        Self(crate::keys::sha256_hex(token.as_ref().as_bytes()))
    }
}

crate::keys::api_key_hash_tests!(
    OAuthTokenHash,
    OAuthAccessToken,
    key_sample = "bencher_oauth_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh",
    other_sample = "bencher_oauth_xY9mN2pQ7rS4tU8vW1zK5jL0fGhaB3",
);

#[cfg(all(test, feature = "server"))]
mod kind_tests {
    use crate::{OAuthAccessToken, OAuthRefreshToken};

    use super::OAuthTokenHash;

    // Both kinds share one column, so a token's prefix is part of what is hashed.
    #[test]
    fn same_body_hashes_differ_by_kind() {
        let access: OAuthAccessToken = "bencher_oauth_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh"
            .parse()
            .unwrap();
        let refresh: OAuthRefreshToken = "bencher_refresh_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh"
            .parse()
            .unwrap();
        assert_ne!(
            OAuthTokenHash::from(&access),
            OAuthTokenHash::from(&refresh)
        );
    }
}
