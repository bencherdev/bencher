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
    pub fn new(conn: &mut DbConnection, is_bencher_cloud: bool) -> Result<Self, HttpError> {
        let organizations = if is_bencher_cloud {
            None
        } else {
            Some(get_organizations(conn)?)
        };
        Ok(Self { organizations })
    }
}

// Intentionally includes soft-deleted organizations for server admin stats
fn get_organizations(conn: &mut DbConnection) -> Result<JsonOrganizations, HttpError> {
    Ok(schema::organization::table
        .load::<QueryOrganization>(conn)
        .map_err(resource_not_found_err!(Organization))?
        .into_iter()
        .map(|org| org.into_json(conn))
        .collect())
}
