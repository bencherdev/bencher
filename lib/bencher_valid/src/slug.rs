use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::{fmt, str::FromStr};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::wasm_bindgen;

use serde::{
    Deserialize, Deserializer, Serialize,
    de::{self, Visitor},
};

use crate::{MAX_LEN, ValidError, is_valid_len};

pub const BASE_36: &str = "0123456789abcdefghijklmnopqrstuvwxyz";

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct Slug(String);

#[cfg(feature = "db")]
crate::typed_string!(Slug);

impl From<uuid::Uuid> for Slug {
    fn from(uuid: uuid::Uuid) -> Self {
        Self(uuid.to_string())
    }
}

impl FromStr for Slug {
    type Err = ValidError;

    fn from_str(slug: &str) -> Result<Self, Self::Err> {
        if is_valid_slug(slug) {
            Ok(Self(slug.into()))
        } else {
            Err(ValidError::Slug(slug.into()))
        }
    }
}

impl AsRef<str> for Slug {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<Slug> for String {
    fn from(slug: Slug) -> Self {
        slug.0
    }
}

impl<'de> Deserialize<'de> for Slug {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(SlugVisitor)
    }
}

struct SlugVisitor;

impl Visitor<'_> for SlugVisitor {
    type Value = Slug;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid slug")
    }

    fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        v.parse().map_err(E::custom)
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_slug(slug: &str) -> bool {
    is_valid_len(slug) && slug == slug::slugify(slug)
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
#[cfg_attr(not(feature = "wasm"), expect(dead_code))]
pub fn new_slug(slug: &str) -> Option<String> {
    Slug::new(slug).map(Into::into)
}

impl Slug {
    pub const MAX_LEN: usize = MAX_LEN;

    #[cfg(feature = "server")]
    const RAND_LEN: usize = 8;

    pub fn new<S>(input: S) -> Option<Self>
    where
        S: AsRef<str>,
    {
        let new_slug = slug::slugify(&input);
        if new_slug.len() > Self::MAX_LEN {
            Some(Self(slug::slugify(
                new_slug.chars().take(Self::MAX_LEN).collect::<String>(),
            )))
        } else if new_slug.is_empty() {
            None
        } else {
            Some(Self(new_slug))
        }
    }

    #[cfg(feature = "server")]
    pub fn unwrap_or_new<N>(name: N, slug: Option<Self>) -> Self
    where
        N: AsRef<str>,
    {
        if let Some(slug) = slug {
            slug
        } else if let Some(slug) = Self::new(name) {
            slug
        } else {
            Self(Self::rand_suffix())
        }
    }

    #[cfg(feature = "server")]
    fn rand_suffix() -> String {
        use chrono::Utc;
        use rand::Rng as _;

        const BASE: u64 = 36;
        const CHARSET: &[u8] = BASE_36.as_bytes();

        let now = Utc::now();
        let mut timestamp = u64::try_from(now.timestamp()).unwrap_or_default();
        let mut base36 = String::new();

        while timestamp > 0 {
            let remainder = timestamp % BASE;
            #[expect(clippy::cast_possible_truncation)]
            if let Some(c) = std::char::from_digit(remainder as u32, BASE as u32) {
                base36.push(c);
            }
            timestamp /= BASE;
        }
        let mut base36 = base36.chars().rev().collect::<String>();

        let mut rng = rand::rng();
        let Some(remainder) = Self::RAND_LEN.checked_sub(base36.len()) else {
            debug_assert!(false, "RAND_LEN is too small");
            return base36;
        };

        for _ in 0..remainder {
            let index = rng.random_range(0..CHARSET.len());
            if let Some(c) = CHARSET.get(index).copied() {
                base36.push(c as char);
            }
        }
        debug_assert!(
            base36.len() == Self::RAND_LEN,
            "Slug length ({}) is not equal to RAND_LEN ({})",
            base36.len(),
            Self::RAND_LEN
        );

        base36
    }

    #[cfg(feature = "server")]
    #[must_use]
    pub fn with_rand_suffix(self) -> Self {
        let truncated = if self.as_ref().len() + 1 + Self::RAND_LEN > Self::MAX_LEN {
            let mid = Self::MAX_LEN - (1 + Self::RAND_LEN);
            slug::slugify(self.as_ref().chars().take(mid).collect::<String>())
        } else {
            self.0
        };
        let rand_suffix = Self::rand_suffix();
        Self(format!("{truncated}-{rand_suffix}"))
    }
}

#[cfg(test)]
mod test {
    use crate::test::{LEN_64_STR, LEN_65_STR};

    use super::{Slug, is_valid_slug};
    use pretty_assertions::assert_eq;

    #[test]
    fn test_slug() {
        assert_eq!(true, is_valid_slug("a-valid-slug"));
        assert_eq!(true, is_valid_slug("2nd-valid-slug"));
        assert_eq!(true, is_valid_slug(LEN_64_STR));

        assert_eq!(false, is_valid_slug(""));
        assert_eq!(false, is_valid_slug(" a-valid-slug"));
        assert_eq!(false, is_valid_slug("a- valid-slug"));
        assert_eq!(false, is_valid_slug("a-valid-slug "));
        assert_eq!(false, is_valid_slug(" a-valid-slug "));
        assert_eq!(false, is_valid_slug("-a-valid-slug"));
        assert_eq!(false, is_valid_slug("a-valid-slug-"));
        assert_eq!(false, is_valid_slug("-a-valid-slug-"));
        assert_eq!(false, is_valid_slug("a--valid-slug"));
        assert_eq!(false, is_valid_slug("a-Valid-slug"));
        assert_eq!(false, is_valid_slug(LEN_65_STR));
        assert_eq!(
            false,
            is_valid_slug(
                "client-submit-serialize-deserialize-handle-client-submit-serialize-deserialize-handle-1996529012"
            )
        );
    }

    #[test]
    fn test_benchmark_name_issue_610() {
        assert!(Slug::new("...").is_none());
    }
}
