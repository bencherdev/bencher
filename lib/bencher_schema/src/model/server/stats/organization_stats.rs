use bencher_json::JsonOrganizations;
use diesel::RunQueryDsl as _;
use dropshot::HttpError;
use tokio::sync::Mutex;

use crate::{
    context::DbConnection, error::resource_not_found_err, model::organization::QueryOrganization,
    schema, yield_connection_lock,
};

pub(super) struct OrganizationStats {
    pub organizations: Option<JsonOrganizations>,
}

impl OrganizationStats {
    pub async fn new(
        db_connection: &Mutex<DbConnection>,
        is_bencher_cloud: bool,
    ) -> Result<Self, HttpError> {
        let organizations = get_organizations(db_connection, is_bencher_cloud).await?;
        Ok(Self { organizations })
    }
}

async fn get_organizations(
    db_connection: &Mutex<DbConnection>,
    is_bencher_cloud: bool,
) -> Result<Option<JsonOrganizations>, HttpError> {
    Ok(if is_bencher_cloud {
        None
    } else {
        Some(yield_connection_lock!(db_connection, |conn| {
            schema::organization::table
                .load::<QueryOrganization>(conn)
                .map_err(resource_not_found_err!(Organization))?
                .into_iter()
                .map(|org| org.into_json(conn))
                .collect()
        }))
    })
}
