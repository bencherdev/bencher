#![cfg(feature = "plus")]

use bencher_json::{DateTime, JsonSso, NonEmpty};

use crate::{
    model::organization::{OrganizationId, QueryOrganization},
    schema::sso as sso_table,
};

crate::macros::typed_id::typed_id!(SsoId);

#[derive(
    Debug, Clone, diesel::Queryable, diesel::Identifiable, diesel::Associations, diesel::Selectable,
)]
#[diesel(table_name = sso_table)]
#[diesel(belongs_to(QueryOrganization, foreign_key = organization_id))]
pub struct QuerySso {
    pub id: SsoId,
    pub organization_id: OrganizationId,
    pub domain: NonEmpty,
    pub created: DateTime,
}

impl QuerySso {
    pub fn into_json(self) -> JsonSso {
        let Self {
            domain, created, ..
        } = self;
        JsonSso { domain, created }
    }
}
