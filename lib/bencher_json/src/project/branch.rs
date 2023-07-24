use std::fmt;

use bencher_valid::{BranchName, GitHash, ResourceId, Slug};
use chrono::{DateTime, Utc};
use once_cell::sync::Lazy;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const BRANCH_MAIN_STR: &str = "main";
#[allow(clippy::expect_used)]
static BRANCH_MAIN: Lazy<BranchName> = Lazy::new(|| {
    BRANCH_MAIN_STR
        .parse()
        .expect("Failed to parse branch name.")
});
#[allow(clippy::expect_used)]
static BRANCH_MAIN_SLUG: Lazy<Option<Slug>> = Lazy::new(|| {
    Some(
        BRANCH_MAIN_STR
            .parse()
            .expect("Failed to parse branch slug."),
    )
});

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewBranch {
    pub name: BranchName,
    pub slug: Option<Slug>,
    pub soft: Option<bool>,
    pub start_point: Option<JsonStartPoint>,
}

impl JsonNewBranch {
    pub fn main() -> Self {
        Self {
            name: BRANCH_MAIN.clone(),
            slug: BRANCH_MAIN_SLUG.clone(),
            soft: None,
            start_point: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonStartPoint {
    pub branch: ResourceId,
    pub thresholds: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBranches(pub Vec<JsonBranch>);

crate::from_vec!(JsonBranches[JsonBranch]);

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBranch {
    pub uuid: Uuid,
    pub project: Uuid,
    pub name: BranchName,
    pub slug: Slug,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
}

impl fmt::Display for JsonBranch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBranchVersion {
    pub uuid: Uuid,
    pub project: Uuid,
    pub name: BranchName,
    pub slug: Slug,
    pub version: JsonVersion,
    pub created: DateTime<Utc>,
    pub modified: DateTime<Utc>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonVersion {
    pub number: u32,
    pub hash: Option<GitHash>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateBranch {
    pub name: Option<BranchName>,
    pub slug: Option<Slug>,
}
