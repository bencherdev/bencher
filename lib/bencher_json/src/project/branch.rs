use std::fmt;

use bencher_valid::{BranchName, DateTime, GitHash, NameId, Slug};
use once_cell::sync::Lazy;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::{JsonReference, ProjectUuid};

crate::typed_uuid::typed_uuid!(BranchUuid);

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
    /// The name of the branch.
    /// Maximum length is 256 characters.
    pub name: BranchName,
    /// The preferred slug for the branch.
    /// If not provided, the slug will be generated from the name.
    /// If the provided or generated slug is already in use, a unique slug will be generated.
    /// Maximum length is 64 characters.
    pub slug: Option<Slug>,
    /// The start point for the new branch.
    /// All branch versions from the start point branch will be shallow copied over to the new branch.
    /// That is, all historical metrics data for the start point branch will appear in queries for the new branch.
    /// For example, pull request branches often use their base branch as their start point branch.
    /// After the new branch is created, it is not kept in sync with the start point branch.
    /// If not provided, the new branch will have no historical data.
    pub start_point: Option<JsonNewStartPoint>,
}

impl JsonNewBranch {
    pub fn main() -> Self {
        Self {
            name: BRANCH_MAIN.clone(),
            slug: BRANCH_MAIN_SLUG.clone(),
            start_point: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewStartPoint {
    /// The UUID, slug, or name of the branch to use as the start point.
    pub branch: NameId,
    /// The full `git` hash of the branch to use as the start point.
    pub hash: Option<GitHash>,
    /// If set to `true`, the thresholds from the start point branch will be deep copied to the new branch.
    /// This can be useful for pull request branches that should have the same thresholds as their target branch.
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
    pub uuid: BranchUuid,
    pub project: ProjectUuid,
    pub name: BranchName,
    pub slug: Slug,
    pub head: JsonReference,
    pub created: DateTime,
    pub modified: DateTime,
    pub archived: Option<DateTime>,
}

impl fmt::Display for JsonBranch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonUpdateBranch {
    /// The new name of the branch.
    /// Maximum length is 256 characters.
    pub name: Option<BranchName>,
    /// The preferred new slug for the branch.
    /// Maximum length is 64 characters.
    pub slug: Option<Slug>,
    /// Set whether the branch is archived.
    pub archived: Option<bool>,
}
