#![cfg(feature = "plus")]

use bencher_json::{DateTime, JsonNewSso, JsonSso, NonEmpty, OrganizationUuid, SsoUuid};
use diesel::{
    BoolExpressionMethods as _, ExpressionMethods as _, JoinOnDsl as _, QueryDsl as _,
    RunQueryDsl as _, SelectableHelper as _,
};
use dropshot::HttpError;

use crate::{
    ApiContext, conn_lock,
    context::DbConnection,
    model::{
        organization::{OrganizationId, QueryOrganization},
        user::{QueryUser, auth::AuthUser},
    },
    resource_not_found_err,
    schema::{self, sso as sso_table},
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
    pub fn from_uuid(conn: &mut DbConnection, uuid: SsoUuid) -> Result<Self, HttpError> {
        schema::sso::table
            .filter(schema::sso::uuid.eq(uuid))
            .first(conn)
            .map_err(resource_not_found_err!(Sso, uuid))
    }

    pub async fn join(
        context: &ApiContext,
        query_user: &QueryUser,
        #[cfg(feature = "otel")] auth_method: bencher_otel::AuthMethod,
    ) -> Result<(), HttpError> {
        let email_domain = query_user.email.domain();

        // Get all organizations IDs that match the domain of the user's email, where they are not already a member
        let organization_uuids = schema::sso::table
            .filter(schema::sso::domain.eq(&email_domain))
            .inner_join(schema::organization::table)
            .left_join(
                schema::organization_role::table.on(schema::organization::id
                    .eq(schema::organization_role::organization_id)
                    .and(schema::organization_role::user_id.eq(query_user.id))),
            )
            .filter(schema::organization_role::id.is_null())
            .select(schema::organization::uuid)
            .load::<OrganizationUuid>(conn_lock!(context))
            .map_err(resource_not_found_err!(Sso, &email_domain))?;

        for organization_uuid in organization_uuids {
            QueryOrganization::join(context, organization_uuid, query_user).await?;

            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserSsoJoin(auth_method));
        }

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
