pub mod project_key;
pub mod project_key_hash;

pub use project_key::{PROJECT_KEY_PREFIX, ProjectKey};
pub use project_key_hash::ProjectKeyHash;

/// ~178 bits of entropy.
/// <https://github.blog/engineering/platform-security/behind-githubs-new-authentication-token-formats/>
pub(crate) const KEY_RANDOM_LEN: usize = 30;

pub(crate) const SHA256_HEX_LEN: usize = 64;

/// Alphanumeric charset for key generation (0-9, A-Z, a-z = 62 characters)
#[cfg(feature = "server")]
pub(crate) const KEY_CHARSET: &[u8] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

#[cfg(feature = "server")]
#[expect(clippy::indexing_slicing)]
pub(crate) fn generate_random_body() -> String {
    use rand::RngExt as _;
    let mut rng = rand::rng();
    std::iter::repeat_with(|| {
        let idx = rng.random_range(0..KEY_CHARSET.len());
        KEY_CHARSET[idx] as char
    })
    .take(KEY_RANDOM_LEN)
    .collect()
}

pub(crate) fn is_valid_alphanumeric_body(body: &str) -> bool {
    body.len() == KEY_RANDOM_LEN && body.bytes().all(|b| b.is_ascii_alphanumeric())
}

pub(crate) fn is_valid_sha256_hex(s: &str) -> bool {
    s.len() == SHA256_HEX_LEN && s.bytes().all(|b| b.is_ascii_hexdigit())
}

pub(crate) fn sha256_hex(input: &[u8]) -> String {
    use sha2::Digest as _;
    hex::encode(sha2::Sha256::digest(input))
}
