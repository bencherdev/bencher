use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::str::FromStr;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::wasm_bindgen;

use serde::{Deserialize, Serialize};

use crate::ValidError;

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct Url(String);

#[cfg(feature = "db")]
crate::typed_string!(Url);

impl TryFrom<String> for Url {
    type Error = ValidError;

    fn try_from(url: String) -> Result<Self, Self::Error> {
        if is_valid_url(&url) {
            Ok(Self(url))
        } else {
            Err(ValidError::Url(url))
        }
    }
}

impl FromStr for Url {
    type Err = ValidError;

    fn from_str(url: &str) -> Result<Self, Self::Err> {
        Self::try_from(url.to_owned())
    }
}

impl Url {
    pub fn is_web(&self) -> bool {
        is_valid_web_url(&self.0)
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

impl From<url::Url> for Url {
    fn from(url: url::Url) -> Self {
        Self(url.into())
    }
}

impl TryFrom<Url> for url::Url {
    type Error = ValidError;

    fn try_from(url: Url) -> Result<Self, Self::Error> {
        Self::from_str(url.as_ref()).map_err(|e| ValidError::UrlToUrl(url, e))
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_url(url: &str) -> bool {
    ::url::Url::from_str(url).is_ok()
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_web_url(url: &str) -> bool {
    ::url::Url::from_str(url).is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
}

#[cfg(test)]
mod tests {
    use super::{is_valid_url, is_valid_web_url};
    use pretty_assertions::assert_eq;

    #[test]
    fn is_valid_web_url_true() {
        for url in [
            "http://example.com",
            "https://example.com",
            "HTTPS://example.com",
            "https://example.com/path?query=string#fragment",
            "https://user:password@example.com",
        ] {
            assert_eq!(true, is_valid_web_url(url), "{url}");
        }
    }

    #[test]
    fn is_valid_web_url_false() {
        for url in [
            "",
            "example.com",
            "//example.com",
            "javascript:alert(document.domain)",
            "JavaScript:alert(document.domain)",
            " javascript:alert(document.domain)",
            "java\tscript:alert(document.domain)",
            "data:text/html,<script>alert(document.domain)</script>",
            "vbscript:msgbox(1)",
            "file:///etc/passwd",
            "ftp://example.com",
            "mailto:user@example.com",
            "httpx://example.com",
        ] {
            assert_eq!(false, is_valid_web_url(url), "{url}");
        }
    }

    #[test]
    fn is_valid_url_true() {
        for url in [
            "http://example.com",
            "https://example.com",
            "http://example.com/path?query=string#fragment",
            "https://user:password@example.com",
        ] {
            assert_eq!(true, is_valid_url(url), "{url}");
        }
    }

    #[test]
    fn is_valid_url_false() {
        for url in [
            "",
            "bad",
            "example.com",
            "http//missing-colon.com",
            "://missing-scheme.com",
        ] {
            assert_eq!(false, is_valid_url(url), "{url}");
        }
    }

    #[test]
    fn url_serde_roundtrip() {
        use super::Url;

        let url: Url = serde_json::from_str("\"https://example.com\"").unwrap();
        assert_eq!(url.as_ref(), "https://example.com");
        let json = serde_json::to_string(&url).unwrap();
        assert_eq!(json, "\"https://example.com\"");

        serde_json::from_str::<Url>("\"not a url\"").unwrap_err();
    }
}
