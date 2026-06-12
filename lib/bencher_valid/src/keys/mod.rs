pub mod bencher_key;
pub mod project_key;
pub mod project_key_hash;
pub mod user_key;
pub mod user_key_hash;

pub use bencher_key::BencherKey;
pub use project_key::ProjectKey;
pub use project_key_hash::ProjectKeyHash;
pub use user_key::UserKey;
pub use user_key_hash::UserKeyHash;

/// ~178 bits of entropy.
/// <https://github.blog/engineering/platform-security/behind-githubs-new-authentication-token-formats/>
pub(crate) const KEY_RANDOM_LEN: usize = 30;

pub(crate) const SHA256_HEX_LEN: usize = 64;

/// Alphanumeric charset for key generation (0-9, A-Z, a-z = 62 characters)
#[cfg(feature = "server")]
pub(crate) const KEY_CHARSET: &[u8] =
    b"0123456789ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz";

#[cfg(feature = "server")]
#[expect(
    clippy::indexing_slicing,
    reason = "index is bounded by KEY_CHARSET.len()"
)]
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

#[cfg(feature = "server")]
pub(crate) fn sha256_hex(input: &[u8]) -> String {
    use sha2::Digest as _;
    hex::encode(sha2::Sha256::digest(input))
}

/// Emit the impl blocks (`PREFIX` / `SANITIZED` / `LENGTH` associated consts,
/// `generate`, `Debug`/`Display`/`Sanitize`/`TryFrom`/`FromStr`/`AsRef`/`From`,
/// and the private `is_valid` validator) for a `bencher_*_*` API key newtype.
///
/// The `pub struct $name(String);` declaration stays explicit in the caller so
/// `typeshare-cli` and serde/schema derives apply at the syntactic level —
/// declarative macros aren't expanded by typeshare, which would otherwise drop
/// these types from the generated `bencher.ts`.
macro_rules! api_key_impl {
    (
        $name:ident,
        prefix = $prefix:literal,
        error = $err:ident $(,)?
    ) => {
        impl $name {
            pub const PREFIX: &'static str = $prefix;
            const SANITIZED: &'static str = concat!($prefix, "******************************");
            const LENGTH: usize = $prefix.len() + $crate::keys::KEY_RANDOM_LEN;

            #[cfg(feature = "server")]
            pub fn generate() -> Self {
                Self(format!(
                    "{}{}",
                    Self::PREFIX,
                    $crate::keys::generate_random_body()
                ))
            }

            fn is_valid(key: &str) -> bool {
                key.len() == Self::LENGTH
                    && key
                        .strip_prefix(Self::PREFIX)
                        .is_some_and($crate::keys::is_valid_alphanumeric_body)
            }
        }

        impl ::core::fmt::Debug for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                ::core::fmt::Display::fmt(self, f)
            }
        }

        impl ::core::fmt::Display for $name {
            fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                if cfg!(debug_assertions) {
                    write!(f, "{}", self.0)
                } else {
                    write!(f, "{}", Self::SANITIZED)
                }
            }
        }

        impl $crate::Sanitize for $name {
            fn sanitize(&mut self) {
                self.0 = Self::SANITIZED.into();
            }
        }

        impl ::core::convert::TryFrom<::std::string::String> for $name {
            type Error = $crate::ValidError;

            fn try_from(key: ::std::string::String) -> ::core::result::Result<Self, Self::Error> {
                if Self::is_valid(&key) {
                    ::core::result::Result::Ok(Self(key))
                } else {
                    ::core::result::Result::Err($crate::ValidError::$err(key))
                }
            }
        }

        impl ::core::str::FromStr for $name {
            type Err = $crate::ValidError;

            fn from_str(key: &str) -> ::core::result::Result<Self, Self::Err> {
                <Self as ::core::convert::TryFrom<::std::string::String>>::try_from(key.to_owned())
            }
        }

        impl ::core::convert::AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }

        impl ::core::convert::From<$name> for ::std::string::String {
            fn from(key: $name) -> Self {
                key.0
            }
        }
    };
}

/// Emit the standard `mod tests` block for a key type defined via
/// [`api_key_impl!`]. `sample` is a fully-formed valid key whose body the
/// negative tests also derive themselves.
macro_rules! api_key_tests {
    (
        $name:ident,
        sample = $sample:literal $(,)?
    ) => {
        #[cfg(test)]
        #[expect(clippy::string_slice, reason = "test strings have known ASCII content")]
        mod tests {
            use super::*;
            use pretty_assertions::assert_eq;

            #[test]
            fn is_valid_true() {
                let padded = format!(
                    "{}{}",
                    $name::PREFIX,
                    "A".repeat($crate::keys::KEY_RANDOM_LEN)
                );
                assert!($name::is_valid(&padded));
                assert!($name::is_valid($sample));
            }

            #[test]
            fn is_valid_false() {
                assert!(!$name::is_valid(""));
                assert!(!$name::is_valid(
                    "wrong_prefix_aB3xY9mN2pQ7rS4tU8vW1zK5jL0fGh"
                ));
                let short = format!("{}short", $name::PREFIX);
                assert!(!$name::is_valid(&short));
                let with_special = format!("{}aB3xY9mN2pQ7rS4tU8vW1zK5jL0f!", $name::PREFIX);
                assert!(!$name::is_valid(&with_special));
                let too_long = format!("{}{}", $name::PREFIX, "A".repeat(31));
                assert!(!$name::is_valid(&too_long));
            }

            #[test]
            fn serde_roundtrip() {
                let json = format!("\"{}\"", $sample);
                let key: $name = serde_json::from_str(&json).unwrap();
                assert_eq!(key.as_ref(), $sample);
                let serialized = serde_json::to_string(&key).unwrap();
                assert_eq!(serialized, json);

                serde_json::from_str::<$name>("\"invalid\"").unwrap_err();
            }

            #[cfg(feature = "server")]
            #[test]
            fn generate_valid() {
                let key = $name::generate();
                assert!(key.as_ref().starts_with($name::PREFIX));
                assert_eq!(key.as_ref().len(), $name::LENGTH);
                let random_part = &key.as_ref()[$name::PREFIX.len()..];
                assert!(random_part.chars().all(|c| c.is_ascii_alphanumeric()));
            }

            #[cfg(feature = "server")]
            #[test]
            fn generate_unique() {
                let k1 = $name::generate();
                let k2 = $name::generate();
                assert_ne!(k1, k2);
            }

            #[test]
            fn sanitize_output() {
                let mut key: $name = $sample.parse().unwrap();
                $crate::Sanitize::sanitize(&mut key);
                assert_eq!(key.as_ref(), $name::SANITIZED);
            }
        }
    };
}

/// Emit the impl blocks for a key-hash newtype (`From<&KeyType>`, `FromStr`,
/// `AsRef<str>`, plus the diesel `typed_string!` impls when the `db` feature is
/// on). The `pub struct $name(String);` declaration stays explicit in the
/// caller so the conditional diesel derives apply at the AST level.
macro_rules! api_key_hash_impl {
    (
        $name:ident,
        $key_ty:ident,
        error = $err:ident $(,)?
    ) => {
        #[cfg(feature = "db")]
        $crate::typed_string!($name);

        #[cfg(feature = "server")]
        impl ::core::convert::From<&$crate::$key_ty> for $name {
            fn from(key: &$crate::$key_ty) -> Self {
                Self($crate::keys::sha256_hex(
                    <$crate::$key_ty as ::core::convert::AsRef<str>>::as_ref(key).as_bytes(),
                ))
            }
        }

        impl ::core::str::FromStr for $name {
            type Err = $crate::ValidError;

            fn from_str(hash: &str) -> ::core::result::Result<Self, Self::Err> {
                if $crate::keys::is_valid_sha256_hex(hash) {
                    ::core::result::Result::Ok(Self(hash.to_owned()))
                } else {
                    ::core::result::Result::Err($crate::ValidError::$err(hash.to_owned()))
                }
            }
        }

        impl ::core::convert::AsRef<str> for $name {
            fn as_ref(&self) -> &str {
                &self.0
            }
        }
    };
}

/// Emit the standard `mod tests` block for a hash type defined via
/// [`api_key_hash_impl!`]. `key_sample` / `other_sample` are two distinct
/// valid keys for the corresponding `$key_ty`; the test asserts they hash to
/// different values.
macro_rules! api_key_hash_tests {
    (
        $name:ident,
        $key_ty:ident,
        key_sample = $key_sample:literal,
        other_sample = $other_sample:literal $(,)?
    ) => {
        #[cfg(test)]
        mod tests {
            use super::*;
            use pretty_assertions::assert_eq;

            const VALID_HEX: &str =
                "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

            #[test]
            fn valid() {
                VALID_HEX.parse::<$name>().unwrap();
                "a".repeat($crate::keys::SHA256_HEX_LEN)
                    .parse::<$name>()
                    .unwrap();
            }

            #[test]
            fn invalid() {
                "".parse::<$name>().unwrap_err();
                "abc123".parse::<$name>().unwrap_err();
                "g".repeat($crate::keys::SHA256_HEX_LEN)
                    .parse::<$name>()
                    .unwrap_err();
            }

            #[test]
            fn roundtrip() {
                let hash: $name = VALID_HEX.parse().unwrap();
                assert_eq!(hash.to_string(), VALID_HEX);
            }

            #[cfg(feature = "server")]
            #[test]
            fn from_key() {
                let key: $crate::$key_ty = $key_sample.parse().unwrap();
                let hash = $name::from(&key);
                assert_eq!(hash.as_ref().len(), $crate::keys::SHA256_HEX_LEN);
                assert_eq!(hash, $name::from(&key));

                let other: $crate::$key_ty = $other_sample.parse().unwrap();
                assert_ne!(hash, $name::from(&other));
            }
        }
    };
}

pub(crate) use api_key_hash_impl;
pub(crate) use api_key_hash_tests;
pub(crate) use api_key_impl;
pub(crate) use api_key_tests;
