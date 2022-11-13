#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const BRANCH_MAIN: &str = "main";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewBranch {
    pub name: String,
    pub slug: Option<String>,
}

impl JsonNewBranch {
    pub fn main() -> Self {
        Self {
            name: BRANCH_MAIN.into(),
            slug: Some(BRANCH_MAIN.into()),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBranch {
    pub uuid: Uuid,
    pub project: Uuid,
    pub name: String,
    pub slug: String,
}
