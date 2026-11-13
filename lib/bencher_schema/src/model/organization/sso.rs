#![cfg(feature = "plus")]

use bencher_json::{
    DateTime, JsonNewSso, JsonSso, NonEmpty, OrganizationUuid, SsoUuid,
    organization::member::OrganizationRole,
};
use diesel::{
    BoolExpressionMethods as _, ExpressionMethods as _, JoinOnDsl as _, QueryDsl as _,
    RunQueryDsl as _,
};
use dropshot::HttpError;

use crate::{
    ApiContext,
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

    pub async fn join_all(
        context: &ApiContext,
        query_user: &QueryUser,
        #[cfg(feature = "otel")] auth_method: bencher_otel::AuthMethod,
    ) -> Result<(), HttpError> {
        let joined = Self::join_all_with(write_conn!(context), query_user, context.clock.now())?;
        #[cfg(feature = "otel")]
        if joined {
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserSsoJoin(auth_method));
        }
        #[cfg(not(feature = "otel"))]
        let _ = joined;
        Ok(())
    }

    /// [`Self::join_all`] on one connection, inside a write transaction, returning whether the
    /// user joined an organization.
    pub fn join_all_with(
        conn: &mut DbConnection,
        query_user: &QueryUser,
        now: DateTime,
    ) -> Result<bool, HttpError> {
        Self::join_domain_with(conn, query_user, &query_user.email.domain(), now)
    }

    /// Join the organization whose SSO domain is `domain`, unless the user is a member, on one
    /// connection, inside a write transaction, returning whether the user joined it.
    pub fn join_domain_with(
        conn: &mut DbConnection,
        query_user: &QueryUser,
        domain: &str,
        now: DateTime,
    ) -> Result<bool, HttpError> {
        let Some(organization_uuid) = Self::unjoined(conn, query_user, domain)? else {
            return Ok(false);
        };
        let insert_org_role = InsertOrganizationRole {
            user_id: query_user.id,
            organization_id: QueryOrganization::get_id(conn, organization_uuid)?,
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
    ) -> Result<Option<OrganizationUuid>, HttpError> {
        // Get all organizations IDs that match the domain, where they are not already a member
        let organization_uuids = schema::sso::table
            .filter(schema::sso::domain.eq(domain))
            .inner_join(schema::organization::table)
            .filter(schema::organization::deleted.is_null())
            .left_join(
                schema::organization_role::table.on(schema::organization::id
                    .eq(schema::organization_role::organization_id)
                    .and(schema::organization_role::user_id.eq(query_user.id))),
            )
            .filter(schema::organization_role::id.is_null())
            .select(schema::organization::uuid)
            .load::<OrganizationUuid>(conn)
            .map_err(resource_not_found_err!(Sso, domain))?;

        if organization_uuids.len() > 1 {
            return Err(issue_error(
                "Multiple SSO organizations found for user",
                &format!(
                    "Multiple SSO organizations found for user with email ({email}) at domain ({domain})",
                    email = query_user.email,
                ),
                format!("{}", organization_uuids.len()),
            ));
        }

        Ok(organization_uuids.first().copied())
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
