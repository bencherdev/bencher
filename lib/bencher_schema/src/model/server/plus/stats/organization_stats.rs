use bencher_json::JsonOrganizations;
use diesel::RunQueryDsl as _;
use dropshot::HttpError;

use crate::{
    context::DbConnection, error::resource_not_found_err, model::organization::QueryOrganization,
    schema,
};

pub(super) struct OrganizationStats {
    pub organizations: Option<JsonOrganizations>,
}

impl OrganizationStats {
    pub fn new(db_connection: &mut DbConnection) -> Result<Self, HttpError> {
        let organizations = Some(get_organizations(db_connection)?);

        Ok(Self { organizations })
    }
}

fn get_organizations(db_connection: &mut DbConnection) -> Result<JsonOrganizations, HttpError> {
    Ok(schema::organization::table
        .load::<QueryOrganization>(db_connection)
        .map_err(resource_not_found_err!(Organization))?
        .into_iter()
        .map(|org| org.into_json(db_connection))
        .collect())
}
