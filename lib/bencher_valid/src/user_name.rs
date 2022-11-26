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

#[cfg(test)]
mod test {
    use super::is_valid_user_name;
    use pretty_assertions::assert_eq;

    #[test]
    fn test_is_valid_user_name() {
        assert_eq!(true, is_valid_user_name("muriel"));
        assert_eq!(true, is_valid_user_name("Muriel"));
        assert_eq!(true, is_valid_user_name("Muriel Bagge"));
        assert_eq!(true, is_valid_user_name("Muriel    Bagge"));
        assert_eq!(true, is_valid_user_name("Muriel Linda Bagge"));
        assert_eq!(true, is_valid_user_name("Bagge, Muriel"));
        assert_eq!(true, is_valid_user_name("Mrs. Muriel Bagge"));
        assert_eq!(true, is_valid_user_name("Muriel Linda-Bagge"));
        assert_eq!(true, is_valid_user_name("Muriel De'Bagge"));
        assert_eq!(true, is_valid_user_name("Mrs. Muriel Linda-De'Bagge"));

        assert_eq!(false, is_valid_user_name(" Muriel Bagge"));
        assert_eq!(false, is_valid_user_name("Muriel Bagge "));
        assert_eq!(false, is_valid_user_name(" Muriel Bagge "));
        assert_eq!(false, is_valid_user_name("Muriel!"));
        assert_eq!(false, is_valid_user_name("Muriel! Bagge"));
        assert_eq!(true, is_valid_user_name("Dumb"));
        assert_eq!(false, is_valid_user_name("Dog"));
        assert_eq!(
            true,
            is_valid_user_name("01234567890123456789012345678901234567890123456789")
        );
        assert_eq!(
            false,
            is_valid_user_name("012345678901234567890123456789012345678901234567890")
        );
        assert_eq!(false, is_valid_user_name(""));
    }
}
