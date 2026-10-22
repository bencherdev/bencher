use std::{fmt, str::FromStr};

use derive_more::Display;
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::ValidError;

/// GitHub's IDs are positive 64-bit signed integers.
const MAX_GITHUB_ID: u64 = i64::MAX as u64;

macro_rules! github_id {
    ($name:ident) => {
        #[derive(Debug, Display, Clone, Copy, Eq, PartialEq, Hash, Serialize, Deserialize)]
        #[cfg_attr(feature = "schema", derive(JsonSchema))]
        #[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
        #[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::BigInt))]
        #[serde(try_from = "u64")]
        pub struct $name(u64);

        impl TryFrom<u64> for $name {
            type Error = ValidError;

            fn try_from(id: u64) -> Result<Self, Self::Error> {
                is_valid_github_id(id)
                    .then_some(Self(id))
                    .ok_or(ValidError::GitHubId(id))
            }
        }

        impl From<$name> for u64 {
            fn from(id: $name) -> Self {
                id.0
            }
        }

        #[cfg(feature = "db")]
        impl<DB> diesel::serialize::ToSql<diesel::sql_types::BigInt, DB> for $name
        where
            DB: diesel::backend::Backend,
            for<'a> i64: diesel::serialize::ToSql<diesel::sql_types::BigInt, DB>
                + Into<
                    <DB::BindCollector<'a> as diesel::query_builder::BindCollector<'a, DB>>::Buffer,
                >,
        {
            fn to_sql<'b>(
                &'b self,
                out: &mut diesel::serialize::Output<'b, '_, DB>,
            ) -> diesel::serialize::Result {
                out.set_value(i64::try_from(self.0)?);
                Ok(diesel::serialize::IsNull::No)
            }
        }

        #[cfg(feature = "db")]
        impl<DB> diesel::deserialize::FromSql<diesel::sql_types::BigInt, DB> for $name
        where
            DB: diesel::backend::Backend,
            i64: diesel::deserialize::FromSql<diesel::sql_types::BigInt, DB>,
        {
            fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
                Ok(u64::try_from(i64::from_sql(bytes)?)?.try_into()?)
            }
        }
    };
}

github_id!(GitHubInstallationId);
github_id!(GitHubRepositoryId);

/// A repository's owner and name, each safe to use as one URL path segment.
#[derive(Debug, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Text))]
#[serde(try_from = "String", into = "String")]
pub struct GitHubRepositoryName {
    owner: String,
    name: String,
}

impl GitHubRepositoryName {
    pub fn new(owner: &str, name: &str) -> Result<Self, ValidError> {
        if is_valid_github_path_segment(owner) && is_valid_github_path_segment(name) {
            Ok(Self {
                owner: owner.to_owned(),
                name: name.to_owned(),
            })
        } else {
            Err(ValidError::GitHubRepositoryName(format!("{owner}/{name}")))
        }
    }

    pub fn owner(&self) -> &str {
        &self.owner
    }

    pub fn name(&self) -> &str {
        &self.name
    }
}

impl fmt::Display for GitHubRepositoryName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.owner, self.name)
    }
}

impl FromStr for GitHubRepositoryName {
    type Err = ValidError;

    fn from_str(full_name: &str) -> Result<Self, Self::Err> {
        let (owner, name) = full_name
            .split_once('/')
            .ok_or_else(|| ValidError::GitHubRepositoryName(full_name.to_owned()))?;
        Self::new(owner, name)
    }
}

impl TryFrom<String> for GitHubRepositoryName {
    type Error = ValidError;

    fn try_from(full_name: String) -> Result<Self, Self::Error> {
        full_name.parse()
    }
}

impl From<GitHubRepositoryName> for String {
    fn from(full_name: GitHubRepositoryName) -> Self {
        full_name.to_string()
    }
}

#[cfg(feature = "db")]
crate::typed_string!(GitHubRepositoryName);

#[cfg(feature = "schema")]
impl JsonSchema for GitHubRepositoryName {
    fn schema_name() -> String {
        "GitHubRepositoryName".to_owned()
    }

    fn json_schema(generator: &mut schemars::r#gen::SchemaGenerator) -> schemars::schema::Schema {
        String::json_schema(generator)
    }
}

/// The app's name in its GitHub URLs, safe to use as one URL path segment.
#[derive(Debug, Display, Clone, Eq, PartialEq, Hash, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(try_from = "String")]
pub struct GitHubAppSlug(String);

impl TryFrom<String> for GitHubAppSlug {
    type Error = ValidError;

    fn try_from(slug: String) -> Result<Self, Self::Error> {
        if is_valid_github_path_segment(&slug) {
            Ok(Self(slug))
        } else {
            Err(ValidError::GitHubAppSlug(slug))
        }
    }
}

impl FromStr for GitHubAppSlug {
    type Err = ValidError;

    fn from_str(slug: &str) -> Result<Self, Self::Err> {
        Self::try_from(slug.to_owned())
    }
}

impl AsRef<str> for GitHubAppSlug {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Whether an installation of the app has approved its contents write.
#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[serde(rename_all = "snake_case")]
pub enum GitHubInstallationPermission {
    Granted,
    /// The installation must approve the new permission on GitHub before it can mint.
    ApprovalRequired,
}

fn is_valid_github_id(id: u64) -> bool {
    (1..=MAX_GITHUB_ID).contains(&id)
}

fn is_valid_github_path_segment(segment: &str) -> bool {
    // A dot segment would drop out of, or move up, a URL built from it.
    !segment.is_empty()
        && segment != "."
        && segment != ".."
        && segment
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'))
}

#[cfg(test)]
mod tests {
    use pretty_assertions::assert_eq;

    use super::GitHubRepositoryName;

    // Kills widening the character set, narrowing it for either half, and dropping the refusal of
    // a dot segment or of an empty half.
    #[test]
    fn a_repository_name_is_two_path_segments() {
        for (owner, name) in [
            ("octo-cat", "hello-world"),
            ("Octo1", "Hello.World_2"),
            ("octocat", ".github"),
            ("octocat", "..."),
            ("octo--cat", "repo"),
            ("octo-cat-", "repo"),
            ("octo_cat", "repo"),
            ("octo.cat", "repo"),
        ] {
            let full_name = GitHubRepositoryName::new(owner, name).unwrap();
            assert_eq!((full_name.owner(), full_name.name()), (owner, name));
        }
        for (owner, name) in [
            ("", "repo"),
            ("octocat", ""),
            ("..", "repo"),
            ("octo/cat", "repo"),
            ("octocat", "."),
            ("octocat", ".."),
            ("octocat", "re/po"),
            ("octocat", "re%2Fpo"),
            ("octocat", "re?po"),
            ("octocat", "re#po"),
            ("octocat", "re po"),
            ("octocat", "répo"),
        ] {
            GitHubRepositoryName::new(owner, name).unwrap_err();
        }
    }

    // Kills a `Display` or a `FromStr` that does not split the full name at its one slash.
    #[test]
    fn a_repository_name_reads_back_its_full_name() {
        let full_name: GitHubRepositoryName = "octo-cat/hello.world".parse().unwrap();

        assert_eq!(
            (full_name.owner(), full_name.name()),
            ("octo-cat", "hello.world")
        );
        assert_eq!(full_name.to_string(), "octo-cat/hello.world");
        for full_name in ["octocat", "octocat/hello/world", "/hello"] {
            full_name.parse::<GitHubRepositoryName>().unwrap_err();
        }
    }
}
