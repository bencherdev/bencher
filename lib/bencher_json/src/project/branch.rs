use bencher_valid::Slug;
use once_cell::sync::Lazy;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const BRANCH_MAIN: &str = "main";
static BRANCH_MAIN_SLUG: Lazy<Option<Slug>> = Lazy::new(|| BRANCH_MAIN.parse().ok());

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewBranch {
    pub name: String,
    pub slug: Option<Slug>,
}

impl JsonNewBranch {
    pub fn main() -> Self {
        Self {
            name: BRANCH_MAIN.into(),
            slug: BRANCH_MAIN_SLUG.clone(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBranch {
    pub uuid: Uuid,
    pub project: Uuid,
    pub name: String,
    pub slug: Slug,
}
