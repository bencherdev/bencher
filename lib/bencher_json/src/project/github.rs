use bencher_valid::{
    DateTime, GitHubInstallationId, GitHubInstallationPermission, GitHubRepositoryName, Jwt,
};
#[cfg(feature = "schema")]
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::{JsonPubUser, ProjectSlug};

const CONNECTED_INT: i32 = 0;
const DISCONNECTED_INT: i32 = 1;

const UNINSTALLED_INT: i32 = 0;
const SUSPENDED_INT: i32 = 1;
const REMOVED_INT: i32 = 2;

/// Where to send the user to link a repository, or to install the GitHub App first.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonGitHubLinkUrls {
    /// GitHub's authorize page.
    pub authorize: Url,
    /// GitHub's install page for the app.
    pub install: Url,
}

/// The repositories the user may link to the project, from a GitHub code exchange.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonGitHubRepositories {
    /// The project the link was started for.
    pub project: ProjectSlug,
    /// Every repository the user can push to, in every installation of the app the user can see.
    pub repositories: Vec<JsonGitHubRepository>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonGitHubRepository {
    pub full_name: GitHubRepositoryName,
    pub installation: GitHubInstallationId,
    /// The installation's page on GitHub, where its owner approves a new permission.
    pub installation_url: Url,
    pub permission: GitHubInstallationPermission,
    /// The grant to link this repository, only if its installation has granted the permission.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub grant: Option<Jwt>,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonNewLinkedRepository {
    /// A grant from the GitHub code exchange.
    pub grant: Jwt,
}

/// A project's link to GitHub.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonProjectGitHub {
    /// The project's linked repository, if it has one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub linked_repository: Option<JsonLinkedRepository>,
}

/// A project's linked repository, which its dispatches go to.
#[typeshare::typeshare]
#[derive(Debug, Clone, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
pub struct JsonLinkedRepository {
    pub full_name: GitHubRepositoryName,
    pub status: LinkedRepositoryStatus,
    /// Why the link is disconnected, if it is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<LinkedRepositoryReason>,
    /// The user who linked it.
    pub user: JsonPubUser,
    pub linked: DateTime,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum LinkedRepositoryStatus {
    Connected = CONNECTED_INT,
    Disconnected = DISCONNECTED_INT,
}

#[typeshare::typeshare]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "schema", derive(JsonSchema))]
#[cfg_attr(feature = "db", derive(diesel::FromSqlRow, diesel::AsExpression))]
#[cfg_attr(feature = "db", diesel(sql_type = diesel::sql_types::Integer))]
#[serde(rename_all = "snake_case")]
#[repr(i32)]
pub enum LinkedRepositoryReason {
    /// The app was uninstalled.
    Uninstalled = UNINSTALLED_INT,
    /// The app's installation was suspended.
    Suspended = SUSPENDED_INT,
    /// The repository was removed from the app's installation.
    Removed = REMOVED_INT,
}

#[cfg(feature = "db")]
mod linked_repository_db {
    use super::{
        CONNECTED_INT, DISCONNECTED_INT, LinkedRepositoryReason, LinkedRepositoryStatus,
        REMOVED_INT, SUSPENDED_INT, UNINSTALLED_INT,
    };

    #[derive(Debug, thiserror::Error)]
    pub enum LinkedRepositoryError {
        #[error("Invalid linked repository status value: {0}")]
        Status(i32),
        #[error("Invalid linked repository reason value: {0}")]
        Reason(i32),
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for LinkedRepositoryStatus
    where
        DB: diesel::backend::Backend,
        i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            match self {
                Self::Connected => CONNECTED_INT.to_sql(out),
                Self::Disconnected => DISCONNECTED_INT.to_sql(out),
            }
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for LinkedRepositoryStatus
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            match i32::from_sql(bytes)? {
                CONNECTED_INT => Ok(Self::Connected),
                DISCONNECTED_INT => Ok(Self::Disconnected),
                value => Err(Box::new(LinkedRepositoryError::Status(value))),
            }
        }
    }

    impl<DB> diesel::serialize::ToSql<diesel::sql_types::Integer, DB> for LinkedRepositoryReason
    where
        DB: diesel::backend::Backend,
        i32: diesel::serialize::ToSql<diesel::sql_types::Integer, DB>,
    {
        fn to_sql<'b>(
            &'b self,
            out: &mut diesel::serialize::Output<'b, '_, DB>,
        ) -> diesel::serialize::Result {
            match self {
                Self::Uninstalled => UNINSTALLED_INT.to_sql(out),
                Self::Suspended => SUSPENDED_INT.to_sql(out),
                Self::Removed => REMOVED_INT.to_sql(out),
            }
        }
    }

    impl<DB> diesel::deserialize::FromSql<diesel::sql_types::Integer, DB> for LinkedRepositoryReason
    where
        DB: diesel::backend::Backend,
        i32: diesel::deserialize::FromSql<diesel::sql_types::Integer, DB>,
    {
        fn from_sql(bytes: DB::RawValue<'_>) -> diesel::deserialize::Result<Self> {
            match i32::from_sql(bytes)? {
                UNINSTALLED_INT => Ok(Self::Uninstalled),
                SUSPENDED_INT => Ok(Self::Suspended),
                REMOVED_INT => Ok(Self::Removed),
                value => Err(Box::new(LinkedRepositoryError::Reason(value))),
            }
        }
    }
}
