#![cfg(feature = "plus")]

use bencher_json::{DateTime, JsonSso, NonEmpty};

use crate::model::organization::OrganizationId;

crate::macros::typed_id::typed_id!(SsoId);

#[derive(diesel::Queryable)]
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
