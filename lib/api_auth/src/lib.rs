// Dev dependencies used by integration tests
#[cfg(test)]
use bencher_api_tests as _;
#[cfg(test)]
use http as _;
#[cfg(test)]
use serde_json as _;
#[cfg(test)]
use tokio as _;
#[cfg(test)]
use url as _;

#[cfg(feature = "plus")]
use bencher_json::NonEmpty;
use bencher_schema::{AUTH_TOKEN_TTL, CLIENT_TOKEN_TTL};
#[cfg(not(feature = "plus"))]
use schemars as _;
#[cfg(not(feature = "plus"))]
use serde as _;

mod accept;
mod confirm;
mod login;
mod oauth;
pub mod oci;
mod signup;

#[cfg(feature = "plus")]
const PLAN_ARG: &str = "plan";
const TOKEN_ARG: &str = "token";
#[cfg(feature = "plus")]
const AUTHORIZATION_ARG: &str = "authorization";
/// Told in a sign-in email that returns to an app's request to connect, never naming the app.
#[cfg(feature = "plus")]
const CONNECT_NOTICE: &str = "An app asked to connect to your Bencher account. If you did not start this, ignore this email.";

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

            // OCI token endpoint
            if http_options {
                api_description.register(oci::auth_oci_token_options)?;
            }
            api_description.register(oci::auth_oci_token_get)?;
            api_description.register(oci::auth_oci_token_post)?;
        }

        Ok(())
    }
}

#[cfg(feature = "plus")]
async fn verify_recaptcha(
    log: &slog::Logger,
    context: &bencher_schema::ApiContext,
    headers: &bencher_schema::HeaderMap,
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

    let remote_ip = bencher_schema::RateLimiting::remote_ip(log, headers);
    slog::info!(log, "Verifying reCAPTCHA from remote IP address"; "remote_ip" => ?remote_ip);

    recaptcha_client
        .verify(log, recaptcha_token, recaptcha_action, remote_ip)
        .await
        .map_err(|_error| bencher_schema::error::forbidden_error("reCAPTCHA verification failed"))
}

/// The app's request to connect a sign-in returns to, dropped unless apps can connect and it
/// still waits for an answer.
#[cfg(feature = "plus")]
async fn pending_authorization(
    context: &bencher_schema::ApiContext,
    authorization: Option<bencher_json::OAuthAuthorizationUuid>,
) -> Result<Option<bencher_json::OAuthAuthorizationUuid>, dropshot::HttpError> {
    use bencher_schema::model::user::oauth_authorization::QueryOAuthAuthorization;

    let Some(uuid) = authorization else {
        return Ok(None);
    };
    if context.oauth.is_none() {
        return Ok(None);
    }
    let pending = QueryOAuthAuthorization::is_pending(
        bencher_schema::public_conn!(context),
        uuid,
        context.clock.now(),
    )
    .map_err(|error| {
        bencher_schema::error::issue_error(
            "Failed to find an OAuth authorization request",
            "Failed to find an OAuth authorization request to sign in to:",
            error,
        )
    })?;
    Ok(pending.then_some(uuid))
}
