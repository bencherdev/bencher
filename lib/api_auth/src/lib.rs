mod accept;
mod confirm;
mod github;
mod google;
mod login;
mod signup;

// TODO Custom max TTL
// 30 minutes * 60 seconds / minute
const AUTH_TOKEN_TTL: u32 = 30 * 60;
// TODO Custom max TTL
// 30 days * 24 hours / day * 60 minutes / hour * 60 seconds / minute
const CLIENT_TOKEN_TTL: u32 = 30 * 24 * 60 * 60;

#[cfg(feature = "plus")]
const PLAN_ARG: &str = "plan";
const TOKEN_ARG: &str = "token";

pub struct Api;

impl bencher_endpoint::Registrar for Api {
    fn register(
        api_description: &mut dropshot::ApiDescription<bencher_schema::ApiContext>,
        http_options: bool,
        #[cfg(feature = "plus")] _is_bencher_cloud: bool,
    ) -> Result<(), dropshot::ApiDescriptionRegisterError> {
        // Auth
        if http_options {
            api_description.register(signup::auth_signup_options)?;
            api_description.register(login::auth_login_options)?;
            api_description.register(confirm::auth_confirm_options)?;
            api_description.register(accept::auth_accept_options)?;
        }
        api_description.register(signup::auth_signup_post)?;
        api_description.register(login::auth_login_post)?;
        api_description.register(confirm::auth_confirm_post)?;
        api_description.register(accept::auth_accept_post)?;

        #[cfg(feature = "plus")]
        {
            // GitHub OAuth
            if http_options {
                api_description.register(github::auth_github_options)?;
            }
            api_description.register(github::auth_github_post)?;

            // Google OAuth
            if http_options {
                api_description.register(google::auth_google_options)?;
            }
            api_description.register(google::auth_google_post)?;
        }

        Ok(())
    }
}

#[cfg(feature = "plus")]
async fn is_allowed_oauth2(
    context: &bencher_schema::context::ApiContext,
) -> Result<(), dropshot::HttpError> {
    use bencher_schema::{error::payment_required_error, model::organization::plan::LicenseUsage};

    // Either the server is Bencher Cloud, or at least one organization must have a valid Bencher Plus license
    let is_allowed = context.is_bencher_cloud
        || !LicenseUsage::get_for_server(
            &context.database.connection,
            &context.licensor,
            Some(bencher_json::PlanLevel::Enterprise),
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
