#![cfg(feature = "plus")]

use std::time::Duration;

use bencher_json::{
    DateTime, JsonNewSso, JsonSso, NonEmpty, SsoUuid, organization::member::OrganizationRole,
};
use diesel::{
    BoolExpressionMethods as _, ExpressionMethods as _, JoinOnDsl as _, QueryDsl as _,
    RunQueryDsl as _,
};
use dropshot::HttpError;
use slog::Logger;
use tokio::time::error::Elapsed;

use crate::{
    ApiContext, auth_conn,
    context::DbConnection,
    error::issue_error,
    macros::fn_get::fn_from_uuid,
    model::{
        organization::{
            OrganizationId, QueryOrganization, organization_role::InsertOrganizationRole,
        },
        user::QueryUser,
    },
    resource_conflict_err, resource_not_found_err,
    schema::{self, sso as sso_table},
    write_conn,
};

crate::macros::typed_id::typed_id!(SsoId);

/// How long a sign-in waits to learn whether the organization at its SSO domain is Enterprise
/// before it signs the user in without joining it.
pub const ENTERPRISE_READ_TIMEOUT: Duration = Duration::from_secs(5);

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = sso_table)]
#[diesel(belongs_to(QueryOrganization, foreign_key = organization_id))]
pub struct QuerySso {
    pub id: SsoId,
    pub uuid: SsoUuid,
    pub organization_id: OrganizationId,
    pub domain: NonEmpty,
    pub created: DateTime,
}

impl QuerySso {
    fn_from_uuid!(sso, SsoUuid, Sso);

    /// [`Self::join_domain`] at the domain of the user's email, counting a join by `auth_method`.
    pub async fn join_all(
        log: &Logger,
        context: &ApiContext,
        query_user: &QueryUser,
        #[cfg(feature = "otel")] auth_method: bencher_otel::AuthMethod,
    ) -> Result<(), HttpError> {
        let joined =
            Self::join_domain(log, context, query_user, &query_user.email.domain()).await?;
        #[cfg(feature = "otel")]
        if joined {
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserSsoJoin(auth_method));
        }
        #[cfg(not(feature = "otel"))]
        let _ = joined;
        Ok(())
    }

    /// Join the user to the organization whose SSO domain is `domain`, when they are not already a
    /// member and the organization is Enterprise, returning whether they joined it. Whether it is
    /// Enterprise is read only for an organization that matches, and a read that fails or takes
    /// longer than [`ENTERPRISE_READ_TIMEOUT`] skips the join, logs, and goes to Sentry, so the
    /// read holds up a sign-in for at most that limit and never fails it. Any other error, from a
    /// connection, a query, or the insert, is returned: a sign-in runs this once its write
    /// transaction is in, so it logs the error and goes on without the join, and no error here
    /// fails a sign-in.
    pub async fn join_domain(
        log: &Logger,
        context: &ApiContext,
        query_user: &QueryUser,
        domain: &str,
    ) -> Result<bool, HttpError> {
        let Some(query_organization) = Self::unjoined(auth_conn!(context), query_user, domain)?
        else {
            return Ok(false);
        };
        let (user, organization) = (query_user.uuid, query_organization.uuid);
        let enterprise = tokio::time::timeout(
            ENTERPRISE_READ_TIMEOUT,
            query_organization.is_enterprise(context),
        )
        .await
        .map_err(EnterpriseReadError::Timeout)
        .and_then(|read| read.map_err(EnterpriseReadError::Read));
        match enterprise {
            Ok(true) => {},
            Ok(false) => {
                slog::info!(
                    log,
                    "User ({user}) did not join organization ({organization}) by SSO, since it is not Enterprise"
                );
                return Ok(false);
            },
            Err(e) => {
                slog::error!(
                    log,
                    "User ({user}) did not join organization ({organization}) by SSO, since whether it is Enterprise could not be read: {e}"
                );
                #[cfg(feature = "sentry")]
                sentry::capture_error(&e);
                return Ok(false);
            },
        }
        Self::join_with(
            write_conn!(context),
            query_user,
            domain,
            &query_organization,
            context.clock.now(),
        )
    }

    /// Join the user to `query_organization`, whose SSO domain is `domain`, on one connection,
    /// unless the user is a member of it by now or `domain` names another organization, returning
    /// whether the user joined it.
    fn join_with(
        conn: &mut DbConnection,
        query_user: &QueryUser,
        domain: &str,
        query_organization: &QueryOrganization,
        now: DateTime,
    ) -> Result<bool, HttpError> {
        let unjoined = Self::unjoined(conn, query_user, domain)?;
        if unjoined.is_none_or(|unjoined| unjoined.id != query_organization.id) {
            return Ok(false);
        }
        let insert_org_role = InsertOrganizationRole {
            user_id: query_user.id,
            organization_id: query_organization.id,
            role: OrganizationRole::Leader,
            created: now,
            modified: now,
        };
        diesel::insert_into(schema::organization_role::table)
            .values(&insert_org_role)
            .execute(conn)
            .map_err(resource_conflict_err!(OrganizationRole, insert_org_role))?;
        Ok(true)
    }

    /// The organization whose SSO domain is `domain`, unless the user is a member.
    fn unjoined(
        conn: &mut DbConnection,
        query_user: &QueryUser,
        domain: &str,
    ) -> Result<Option<QueryOrganization>, HttpError> {
        // Get all organizations that match the domain, where they are not already a member
        let organizations = schema::sso::table
            .filter(schema::sso::domain.eq(domain))
            .inner_join(schema::organization::table)
            .filter(schema::organization::deleted.is_null())
            .left_join(
                schema::organization_role::table.on(schema::organization::id
                    .eq(schema::organization_role::organization_id)
                    .and(schema::organization_role::user_id.eq(query_user.id))),
            )
            .filter(schema::organization_role::id.is_null())
            .select(schema::organization::all_columns)
            .load::<QueryOrganization>(conn)
            .map_err(resource_not_found_err!(Sso, domain))?;

        if organizations.len() > 1 {
            return Err(issue_error(
                "Multiple SSO organizations found for user",
                &format!(
                    "Multiple SSO organizations found for user with email ({email}) at domain ({domain})",
                    email = query_user.email,
                ),
                format!("{}", organizations.len()),
            ));
        }

        Ok(organizations.into_iter().next())
    }

    pub fn into_json(self) -> JsonSso {
        let Self {
            uuid,
            domain,
            created,
            ..
        } = self;
        JsonSso {
            uuid,
            domain,
            created,
        }
    }
}

/// Why a sign-in could not learn whether the organization at its SSO domain is Enterprise.
#[derive(Debug, thiserror::Error)]
enum EnterpriseReadError {
    #[error("the read took longer than {ENTERPRISE_READ_TIMEOUT:?}")]
    Timeout(Elapsed),
    #[error("{0}")]
    Read(HttpError),
}

#[derive(Debug, diesel::Insertable)]
#[diesel(table_name = sso_table)]
pub struct InsertSso {
    pub uuid: SsoUuid,
    pub organization_id: OrganizationId,
    pub domain: NonEmpty,
    pub created: DateTime,
}

impl InsertSso {
    pub fn from_json(organization_id: OrganizationId, json_new_sso: JsonNewSso) -> Self {
        let JsonNewSso { domain } = json_new_sso;
        Self {
            uuid: SsoUuid::new(),
            organization_id,
            domain,
            created: DateTime::now(),
        }
    }
}
