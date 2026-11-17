#![cfg(feature = "plus")]

use std::time::Duration;

use bencher_json::{DateTime, JsonNewSso, JsonSso, NonEmpty, SsoUuid};
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
        organization::{OrganizationId, QueryOrganization},
        user::QueryUser,
    },
    resource_not_found_err,
    schema::{self, sso as sso_table},
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

    /// Join the user to the organization whose SSO domain is their email's domain, when they are
    /// not already a member and the organization is Enterprise. Whether it is Enterprise is read
    /// only for an organization that matches, and a read that fails or takes longer than
    /// [`ENTERPRISE_READ_TIMEOUT`] skips the join and logs, so it never fails or holds up a
    /// sign-in.
    pub async fn join_all(
        log: &Logger,
        context: &ApiContext,
        query_user: &QueryUser,
        #[cfg(feature = "otel")] auth_method: bencher_otel::AuthMethod,
    ) -> Result<(), HttpError> {
        let email_domain = query_user.email.domain();

        // Get all organizations that match the domain of the user's email, where they are not already a member
        let organizations = schema::sso::table
            .filter(schema::sso::domain.eq(&email_domain))
            .inner_join(schema::organization::table)
            .filter(schema::organization::deleted.is_null())
            .left_join(
                schema::organization_role::table.on(schema::organization::id
                    .eq(schema::organization_role::organization_id)
                    .and(schema::organization_role::user_id.eq(query_user.id))),
            )
            .filter(schema::organization_role::id.is_null())
            .select(schema::organization::all_columns)
            .load::<QueryOrganization>(auth_conn!(context))
            .map_err(resource_not_found_err!(Sso, &email_domain))?;

        if organizations.len() > 1 {
            return Err(issue_error(
                "Multiple SSO organizations found for user",
                &format!(
                    "Multiple SSO organizations found for user with email ({email}) at domain ({email_domain})",
                    email = query_user.email,
                ),
                format!("{}", organizations.len()),
            ));
        }

        let Some(query_organization) = organizations.first() else {
            return Ok(());
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
                return Ok(());
            },
            Err(e) => {
                slog::error!(
                    log,
                    "User ({user}) did not join organization ({organization}) by SSO, since whether it is Enterprise could not be read: {e}"
                );
                #[cfg(feature = "sentry")]
                sentry::capture_error(&e);
                return Ok(());
            },
        }

        QueryOrganization::join(context, organization, query_user).await?;

        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserSsoJoin(auth_method));

        Ok(())
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
