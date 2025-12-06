#![cfg(feature = "plus")]

use bencher_json::{DateTime, JsonNewSso, JsonSso, NonEmpty, SsoUuid};
use diesel::{ExpressionMethods as _, QueryDsl as _, RunQueryDsl as _};
use dropshot::HttpError;

use crate::{
    context::DbConnection,
    model::organization::{OrganizationId, QueryOrganization},
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
