use std::fmt;

use bencher_valid::{BranchName, DateTime, GitHash, NameId, Slug};
use once_cell::sync::Lazy;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ProjectUuid;

crate::typed_uuid::typed_uuid!(BranchUuid);
crate::typed_uuid::typed_uuid!(VersionUuid);

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
    pub start_point: Option<JsonStartPoint>,
    pub created: DateTime,
    pub modified: DateTime,
}

impl fmt::Display for JsonBranch {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonStartPoint {
    pub branch: BranchUuid,
    pub version: JsonVersion,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonBranchVersion {
    pub uuid: BranchUuid,
    pub project: ProjectUuid,
    pub name: BranchName,
    pub slug: Slug,
    pub version: JsonVersion,
    pub start_point: Option<JsonStartPoint>,
    pub created: DateTime,
    pub modified: DateTime,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Deserialize, Serialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonVersion {
    pub number: VersionNumber,
    pub hash: Option<GitHash>,
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
    /// The full `git` commit hash for the next version of the branch.
    /// This is useful in cases where there may be a race condition
    /// when a start point branch has been updated but its benchmarks have not yet finished.
    pub hash: Option<GitHash>,
}

impl JsonUpdateBranch {
    // Check whether only the `git` hash is being updated.
    pub fn is_hash_only(&self) -> bool {
        self.name.is_none() && self.slug.is_none() && self.hash.is_some()
    }
}

#[typeshare::typeshare]
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, Hash, derive_more::Display, Serialize, Deserialize,
)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
pub struct VersionNumber(pub u32);

#[cfg(feature = "db")]
mod version_number {
    use super::VersionNumber;

    impl VersionNumber {
        #[must_use]
        pub fn increment(self) -> Self {
            Self(self.0.checked_add(1).unwrap_or_default())
        }

        #[must_use]
        pub fn decrement(self) -> Self {
            Self(self.0.checked_sub(1).unwrap_or_default())
        }
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for VersionNumber
    where
        DB: diesel::backend::Backend,
        for<'a> i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>
            + Into<<DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            out.set_value(i32::try_from(self.0)?);
            Ok(diesel::serialize::IsNull::No)
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for VersionNumber
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            Ok(Self(u32::try_from(i32::from_sql(bytes)?)?))
        }
    }
}
