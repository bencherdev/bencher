use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::{fmt, str::FromStr};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize,
};

use crate::ValidError;

#[derive(Debug, Display, Clone, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct Url(String);

impl FromStr for Url {
    type Err = ValidError;

    fn from_str(url: &str) -> Result<Self, Self::Err> {
        if is_valid_url(url) {
            Ok(Self(url.into()))
        } else {
            Err(ValidError::Url(url.into()))
        }
    }
}

impl AsRef<str> for Url {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<Url> for String {
    fn from(url: Url) -> Self {
        url.0
    }
}

impl<'de> Deserialize<'de> for Url {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(UrlVisitor)
    }
}

struct UrlVisitor;

impl<'de> Visitor<'de> for UrlVisitor {
    type Value = Url;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid URL")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_url(url: &str) -> bool {
    ::url::Url::from_str(url).is_ok()
}

#[cfg(test)]
mod test {
    use super::is_valid_url;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_email() {
        assert_eq!(true, is_valid_url("http://example.com"));
        assert_eq!(true, is_valid_url("https://example.com"));

        assert_eq!(false, is_valid_url(""));
        assert_eq!(false, is_valid_url("bad"));
    }
}
