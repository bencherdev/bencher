use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use std::{convert::Infallible, str::FromStr};

use serde::{Deserialize, Serialize};

/// A `--ci-id`, which names a GitHub check and its pull request comment section in place of the
/// project's name.
#[typeshare::typeshare]
#[derive(Debug, Display, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(from = "String")]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
pub struct CiId(String);

#[cfg(feature = "db")]
crate::typed_string!(CiId);

impl From<String> for CiId {
    fn from(ci_id: String) -> Self {
        Self(ci_id)
    }
}

impl FromStr for CiId {
    type Err = Infallible;

    fn from_str(ci_id: &str) -> Result<Self, Self::Err> {
        Ok(Self(ci_id.to_owned()))
    }
}

impl AsRef<str> for CiId {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl From<CiId> for String {
    fn from(ci_id: CiId) -> Self {
        ci_id.0
    }
}
