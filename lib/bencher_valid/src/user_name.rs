use derive_more::Display;
use icu_properties::{
    CodePointMapData,
    props::{GeneralCategory, GeneralCategoryGroup},
};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::str::FromStr;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::wasm_bindgen;

use serde::{Deserialize, Serialize};

use crate::{Email, Slug, ValidError, is_valid_len};

/// A letter (`\p{L}`), a mark (`\p{M}`), and a number (`\p{N}`) of any script.
const NAME_CATEGORIES: GeneralCategoryGroup = GeneralCategoryGroup::Letter
    .union(GeneralCategoryGroup::Mark)
    .union(GeneralCategoryGroup::Number);

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct UserName(String);

#[cfg(feature = "db")]
crate::typed_string!(UserName);

impl TryFrom<String> for UserName {
    type Error = ValidError;

    fn try_from(user_name: String) -> Result<Self, Self::Error> {
        if is_valid_user_name(&user_name) {
            Ok(Self(user_name))
        } else {
            Err(ValidError::UserName(user_name))
        }
    }
}

impl FromStr for UserName {
    type Err = ValidError;

    fn from_str(user_name: &str) -> Result<Self, Self::Err> {
        Self::try_from(user_name.to_owned())
    }
}

impl AsRef<str> for UserName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<Slug> for UserName {
    fn from(slug: Slug) -> Self {
        Self(slug.to_string())
    }
}

impl From<UserName> for String {
    fn from(user_name: UserName) -> Self {
        user_name.0
    }
}

impl UserName {
    pub const MAX_LEN: usize = crate::MAX_LEN;
    /// The name of an account whose address gives no name.
    const FALLBACK: &str = "User";

    /// A name from the local part of `email`, cut to the characters a name allows, or the fallback
    /// when no letter or number is left.
    pub fn from_email(email: &Email) -> Self {
        let mut name = String::new();
        for name_char in email
            .local_part()
            .chars()
            .filter(|c| is_valid_user_name_char(*c))
        {
            if name.len().saturating_add(name_char.len_utf8()) > Self::MAX_LEN {
                break;
            }
            name.push(name_char);
        }
        let name = name.trim();
        name.chars()
            .any(char::is_alphanumeric)
            .then(|| name.parse().ok())
            .flatten()
            .unwrap_or_else(|| Self(Self::FALLBACK.to_owned()))
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_user_name(name: &str) -> bool {
    is_valid_len(name) && name.chars().all(is_valid_user_name_char)
}

/// A letter (`\p{L}`), a mark (`\p{M}`), or a number (`\p{N}`) of any script, and the punctuation
/// a name is written with. A symbol, an emoji, and a control character are none of those.
fn is_valid_user_name_char(name_char: char) -> bool {
    if matches!(name_char, ' ' | '_' | ',' | '.' | '-' | '\'') {
        return true;
    }

    NAME_CATEGORIES.contains(CodePointMapData::<GeneralCategory>::new().get(name_char))
}

#[cfg(test)]
mod tests {
    use crate::tests::{LEN_0_STR, LEN_64_STR, LEN_65_STR};

    use super::is_valid_user_name;
    use pretty_assertions::assert_eq;

    #[test]
    fn is_valid_user_name_true() {
        for name in [
            "muriel",
            "Muriel",
            "Muriel Bagge",
            "Muriel    Bagge",
            "Muriel Linda Bagge",
            "Bagge, Muriel",
            "Mrs. Muriel Bagge",
            "Muriel Linda-Bagge",
            "Muriel De'Bagge",
            "Mrs. Muriel Linda-De'Bagge",
            // A GitHub Enterprise Managed User login always holds an underscore.
            "muriel_bagge",
            "José",
            "Zoë",
            // The same name with the diaeresis as a combining mark instead of one codepoint.
            "Zoe\u{0308}",
            "Жанна",
            "张伟",
            "مُرئيل",
            LEN_64_STR,
        ] {
            assert_eq!(true, is_valid_user_name(name), "{name}");
        }
    }

    #[test]
    fn is_valid_user_name_false() {
        for name in [
            LEN_0_STR,
            LEN_65_STR,
            " Muriel Bagge",
            "Muriel Bagge ",
            " Muriel Bagge ",
            "Muriel!",
            "Muriel! Bagge",
            "Muriel 🐄",
            // A circled letter is a symbol, however much it reads as a letter.
            "Ⓜuriel",
            "Muriel\u{0}Bagge",
            "Muriel\tBagge",
        ] {
            assert_eq!(false, is_valid_user_name(name), "{name}");
        }
    }

    #[test]
    fn user_name_from_email() {
        use super::UserName;

        for (email, name) in [
            ("ada@example.com", "ada"),
            ("Ada.Lovelace@example.com", "ada.lovelace"),
            ("o'brien-smith@example.com", "o'brien-smith"),
            ("ada+bencher_dev@example.com", "adabencher_dev"),
            ("ada_lovelace@example.com", "ada_lovelace"),
            ("zoë@example.com", "zoë"),
            ("+@example.com", "User"),
            ("___@example.com", "User"),
        ] {
            let email = email.parse().unwrap();
            assert_eq!(UserName::from_email(&email).as_ref(), name, "{email}");
        }
    }

    #[test]
    fn user_name_serde_roundtrip() {
        use super::UserName;

        let name: UserName = serde_json::from_str("\"Muriel Bagge\"").unwrap();
        assert_eq!(name.as_ref(), "Muriel Bagge");
        let json = serde_json::to_string(&name).unwrap();
        assert_eq!(json, "\"Muriel Bagge\"");

        serde_json::from_str::<UserName>("\"Muriel!\"").unwrap_err();
    }
}
