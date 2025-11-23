#[cfg(feature = "plus")]
use bencher_json::NonEmpty;
#[cfg(not(feature = "plus"))]
use schemars as _;
#[cfg(not(feature = "plus"))]
use serde as _;

mod accept;
mod confirm;
mod login;
#[cfg(feature = "plus")]
mod oauth;
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
                api_description.register(oauth::github::auth_github_options)?;
            }
            api_description.register(oauth::github::auth_github_get)?;
            api_description.register(oauth::github::auth_github_post)?;

            // Google OAuth
            if http_options {
                api_description.register(oauth::google::auth_google_options)?;
            }
            api_description.register(oauth::google::auth_google_get)?;
            api_description.register(oauth::google::auth_google_post)?;
        }

        Ok(())
    }
}

#[cfg(feature = "plus")]
async fn verify_recaptcha(
    log: &slog::Logger,
    headers: &http::HeaderMap,
    context: &bencher_schema::ApiContext,
    recaptcha_token: Option<&NonEmpty>,
    recaptcha_action: bencher_json::RecaptchaAction,
) -> Result<(), dropshot::HttpError> {
    // If the recaptcha client is not configured, skip token verification
    let Some(recaptcha_client) = &context.recaptcha_client else {
        return Ok(());
    };

    // todo(epompeii): Add a way to signup with the CLI again
    let Some(recaptcha_token) = recaptcha_token.cloned() else {
        return Err(bencher_schema::error::forbidden_error(
            "Missing reCAPTCHA token",
        ));
    };

    let remote_ip = bencher_endpoint::remote_ip(headers);
    slog::info!(log, "Verifying reCAPTCHA from remote IP address"; "remote_ip" => ?remote_ip);

    recaptcha_client
        .verify(log, recaptcha_token, recaptcha_action, remote_ip)
        .await
        .map_err(|_error| bencher_schema::error::forbidden_error("reCAPTCHA verification failed"))
}
