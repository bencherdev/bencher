use bencher_json::PlanLevel;
use bencher_schema::{error::payment_required_error, model::organization::plan::LicenseUsage};

pub mod github;
pub mod google;

async fn is_allowed_oauth2(
    context: &bencher_schema::context::ApiContext,
) -> Result<(), dropshot::HttpError> {
    // Either the server is Bencher Cloud, or at least one organization must have a valid Bencher Plus license
    let is_allowed = context.is_bencher_cloud
        || !LicenseUsage::get_for_server(
            &context.database.connection,
            &context.licensor,
            Some(PlanLevel::Enterprise),
        )
        .await?
        .is_empty();

    if is_allowed {
        Ok(())
    } else {
        Err(payment_required_error(
            "You must have a valid Bencher Plus Enterprise license for at least one organization on the server to use OAuth2",
        ))
    }
}
