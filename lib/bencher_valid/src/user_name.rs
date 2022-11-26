use once_cell::sync::Lazy;
use regex::Regex;
use std::fmt;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer,
};

use crate::REGEX_ERROR;

pub struct UserName(String);

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_user_name(name: &str) -> bool {
    static NAME_REGEX: Lazy<Regex> =
        Lazy::new(|| Regex::new(r"^[[[:alnum:]] ,\.\-']{4,50}$").expect(REGEX_ERROR));

    if name != name.trim() {
        return false;
    }

    if name.len() < 4 || name.len() > 50 {
        return false;
    };

    NAME_REGEX.is_match(name)
}

impl<'de> Deserialize<'de> for UserName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_string(UserNameVisitor)
    }
}

struct UserNameVisitor;

impl<'de> Visitor<'de> for UserNameVisitor {
    type Value = UserName;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid user name")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if is_valid_user_name(value) {
            Ok(UserName(value.into()))
        } else {
            Err(E::custom(format!("Invalid user name: {value}")))
        }
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if is_valid_user_name(&value) {
            Ok(UserName(value))
        } else {
            Err(E::custom(format!("Invalid user name: {value}")))
        }
    }
}
