use bencher_github_client::GitHubInstallationChange;
use bencher_json::{
    DateTime, GitHubInstallationId, GitHubRepositoryId, GitHubRepositoryName,
    project::github::{JsonLinkedRepository, LinkedRepositoryReason, LinkedRepositoryStatus},
};
use diesel::{
    ExpressionMethods as _, OptionalExtension as _, QueryDsl as _, RunQueryDsl as _,
    SelectableHelper as _, dsl::Eq, result::QueryResult,
};
use dropshot::HttpError;

use super::ProjectId;
use crate::{
    context::DbConnection,
    model::user::{QueryUser, UserId},
    schema::{self, linked_repository as linked_repository_table},
};

/// A delivery can name every repository an organization has, so each write binds at most this
/// many, well under SQLite's limit on bound parameters.
const REPOSITORY_BATCH: usize = 1024;

/// The one GitHub repository a project's dispatches go to.
#[derive(Debug, Clone, diesel::Queryable, diesel::Selectable, diesel::Insertable)]
#[diesel(table_name = linked_repository_table)]
pub struct QueryLinkedRepository {
    pub project_id: ProjectId,
    pub installation_id: GitHubInstallationId,
    pub repository_id: GitHubRepositoryId,
    pub full_name: GitHubRepositoryName,
    pub status: LinkedRepositoryStatus,
    pub reason: Option<LinkedRepositoryReason>,
    pub user_id: UserId,
    pub linked: DateTime,
}

impl QueryLinkedRepository {
    /// A connected link that `user` made to the repository just now.
    pub fn connected(
        project_id: ProjectId,
        installation_id: GitHubInstallationId,
        repository_id: GitHubRepositoryId,
        full_name: GitHubRepositoryName,
        user_id: UserId,
        now: DateTime,
    ) -> Self {
        Self {
            project_id,
            installation_id,
            repository_id,
            full_name,
            status: LinkedRepositoryStatus::Connected,
            reason: None,
            user_id,
            linked: now,
        }
    }

    pub fn get(conn: &mut DbConnection, project_id: ProjectId) -> QueryResult<Option<Self>> {
        schema::linked_repository::table
            .filter(schema::linked_repository::project_id.eq(project_id))
            .select(Self::as_select())
            .first(conn)
            .optional()
    }

    /// The project's link, if it is connected and so can mint a dispatch token.
    pub fn get_connected(
        conn: &mut DbConnection,
        project_id: ProjectId,
    ) -> QueryResult<Option<Self>> {
        schema::linked_repository::table
            .filter(schema::linked_repository::project_id.eq(project_id))
            .filter(schema::linked_repository::status.eq(LinkedRepositoryStatus::Connected))
            .select(Self::as_select())
            .first(conn)
            .optional()
    }

    /// Link the project, in place of any link it had.
    pub fn replace(&self, conn: &mut DbConnection) -> QueryResult<usize> {
        diesel::replace_into(schema::linked_repository::table)
            .values(self)
            .execute(conn)
    }

    pub fn delete(conn: &mut DbConnection, project_id: ProjectId) -> QueryResult<usize> {
        diesel::delete(
            schema::linked_repository::table
                .filter(schema::linked_repository::project_id.eq(project_id)),
        )
        .execute(conn)
    }

    /// Keep the repository's current full name, which a mint for this link returned. A link
    /// replaced since is left alone.
    pub fn write_full_name(
        &self,
        conn: &mut DbConnection,
        full_name: &GitHubRepositoryName,
    ) -> QueryResult<bool> {
        diesel::update(
            schema::linked_repository::table
                .filter(schema::linked_repository::project_id.eq(self.project_id))
                .filter(schema::linked_repository::installation_id.eq(self.installation_id))
                .filter(schema::linked_repository::repository_id.eq(self.repository_id)),
        )
        .set(schema::linked_repository::full_name.eq(full_name))
        .execute(conn)
        .map(|updated| updated > 0)
    }

    /// Disconnect this link for a mint that GitHub refused. A link replaced or disconnected
    /// since is left alone.
    pub fn disconnect(
        &self,
        conn: &mut DbConnection,
        reason: LinkedRepositoryReason,
    ) -> QueryResult<bool> {
        diesel::update(
            schema::linked_repository::table
                .filter(schema::linked_repository::project_id.eq(self.project_id))
                .filter(schema::linked_repository::installation_id.eq(self.installation_id))
                .filter(schema::linked_repository::repository_id.eq(self.repository_id))
                .filter(schema::linked_repository::status.eq(LinkedRepositoryStatus::Connected)),
        )
        .set(disconnected(reason))
        .execute(conn)
        .map(|updated| updated > 0)
    }

    /// Bring every link a webhook delivery names up to date. Each write sets an end state, so a
    /// duplicate of the latest delivery changes nothing.
    pub fn apply(conn: &mut DbConnection, change: &GitHubInstallationChange) -> QueryResult<()> {
        match change {
            GitHubInstallationChange::Uninstalled(installation) => {
                // Uninstalling ends any other reason too, so a later unsuspend reconnects nothing.
                diesel::update(
                    schema::linked_repository::table
                        .filter(schema::linked_repository::installation_id.eq(installation)),
                )
                .set(disconnected(LinkedRepositoryReason::Uninstalled))
                .execute(conn)?;
            },
            GitHubInstallationChange::Suspended(installation) => {
                // A link already disconnected keeps its reason, so unsuspending does not reconnect it.
                diesel::update(
                    schema::linked_repository::table
                        .filter(schema::linked_repository::installation_id.eq(installation))
                        .filter(
                            schema::linked_repository::status.eq(LinkedRepositoryStatus::Connected),
                        ),
                )
                .set(disconnected(LinkedRepositoryReason::Suspended))
                .execute(conn)?;
            },
            GitHubInstallationChange::Unsuspended {
                installation,
                repositories,
            } => {
                // GitHub's delivery may name no repositories, so the links the suspension
                // disconnected reconnect by installation.
                diesel::update(
                    schema::linked_repository::table
                        .filter(schema::linked_repository::installation_id.eq(installation))
                        .filter(
                            schema::linked_repository::reason.eq(LinkedRepositoryReason::Suspended),
                        ),
                )
                .set(connected(*installation))
                .execute(conn)?;
                reconnect(conn, *installation, repositories)?;
            },
            GitHubInstallationChange::Removed {
                installation,
                repositories,
            } => {
                for batch in repositories.chunks(REPOSITORY_BATCH) {
                    diesel::update(
                        schema::linked_repository::table
                            .filter(schema::linked_repository::installation_id.eq(installation))
                            .filter(schema::linked_repository::repository_id.eq_any(batch)),
                    )
                    .set(disconnected(LinkedRepositoryReason::Removed))
                    .execute(conn)?;
                }
            },
            GitHubInstallationChange::Added {
                installation,
                repositories,
            } => reconnect(conn, *installation, repositories)?,
        }
        Ok(())
    }

    pub fn into_json(self, conn: &mut DbConnection) -> Result<JsonLinkedRepository, HttpError> {
        let Self {
            project_id: _,
            installation_id: _,
            repository_id: _,
            full_name,
            status,
            reason,
            user_id,
            linked,
        } = self;
        Ok(JsonLinkedRepository {
            full_name,
            status,
            reason,
            user: QueryUser::get(conn, user_id)?.into_pub_json(),
            linked,
        })
    }
}

/// Connect every link to one of the repositories through the installation that now holds it,
/// whatever its status, since a repository is in one installation of the app at a time.
fn reconnect(
    conn: &mut DbConnection,
    installation: GitHubInstallationId,
    repositories: &[GitHubRepositoryId],
) -> QueryResult<()> {
    for batch in repositories.chunks(REPOSITORY_BATCH) {
        diesel::update(
            schema::linked_repository::table
                .filter(schema::linked_repository::repository_id.eq_any(batch)),
        )
        .set(connected(installation))
        .execute(conn)?;
    }
    Ok(())
}

fn connected(
    installation: GitHubInstallationId,
) -> (
    SetStatus,
    Eq<schema::linked_repository::installation_id, GitHubInstallationId>,
) {
    (
        (
            schema::linked_repository::status.eq(LinkedRepositoryStatus::Connected),
            schema::linked_repository::reason.eq(None),
        ),
        schema::linked_repository::installation_id.eq(installation),
    )
}

fn disconnected(reason: LinkedRepositoryReason) -> SetStatus {
    (
        schema::linked_repository::status.eq(LinkedRepositoryStatus::Disconnected),
        schema::linked_repository::reason.eq(Some(reason)),
    )
}

type SetStatus = (
    Eq<schema::linked_repository::status, LinkedRepositoryStatus>,
    Eq<schema::linked_repository::reason, Option<LinkedRepositoryReason>>,
);
