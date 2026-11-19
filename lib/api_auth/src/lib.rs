// Dev dependencies used by integration tests
#[cfg(test)]
use async_trait as _;
#[cfg(test)]
use bencher_api_tests as _;
#[cfg(test)]
use bencher_turnstile as _;
#[cfg(test)]
use reqwest as _;
#[cfg(test)]
use serde_json as _;
#[cfg(test)]
use tokio as _;

#[cfg(feature = "plus")]
use bencher_json::NonEmpty;
use bencher_schema::{AUTH_TOKEN_TTL, CLIENT_TOKEN_TTL, model::user::QueryUser};
#[cfg(not(feature = "plus"))]
use schemars as _;
#[cfg(not(feature = "plus"))]
use serde as _;

mod claim;
mod confirm;
mod invite;
mod login;
pub mod oci;
mod provider;
mod session;
mod sign_in;
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
        }
        api_description.register(signup::auth_signup_post)?;
        api_description.register(login::auth_login_post)?;
        api_description.register(confirm::auth_confirm_post)?;

        // Session
        if http_options {
            api_description.register(session::auth_session_options)?;
        }
        api_description.register(session::auth_session_get)?;
        api_description.register(session::auth_session_delete)?;

        // Sign in by email
        if http_options {
            api_description.register(sign_in::email::auth_email_options)?;
            api_description.register(sign_in::email::auth_email_resend_options)?;
            api_description.register(sign_in::code::auth_code_options)?;
            api_description.register(sign_in::link::auth_link_options)?;
        }
        api_description.register(sign_in::email::auth_email_post)?;
        api_description.register(sign_in::email::auth_email_delete)?;
        api_description.register(sign_in::email::auth_email_resend_post)?;
        api_description.register(sign_in::code::auth_code_post)?;
        api_description.register(sign_in::link::auth_link_post)?;

        // Invitations
        if http_options {
            api_description.register(invite::auth_invite_options)?;
            api_description.register(invite::auth_invite_accept_options)?;
        }
        api_description.register(invite::auth_invite_post)?;
        api_description.register(invite::auth_invite_accept_post)?;

        // Claims
        if http_options {
            api_description.register(claim::auth_claim_options)?;
        }
        api_description.register(claim::auth_claim_get)?;

        // Sign in again
        if http_options {
            api_description.register(sign_in::fresh::auth_fresh_options)?;
            api_description.register(sign_in::fresh::auth_fresh_email_options)?;
            api_description.register(sign_in::fresh::auth_fresh_email_resend_options)?;
            api_description.register(sign_in::fresh::auth_fresh_code_options)?;
        }
        api_description.register(sign_in::fresh::auth_fresh_get)?;
        api_description.register(sign_in::fresh::auth_fresh_email_post)?;
        api_description.register(sign_in::fresh::auth_fresh_email_resend_post)?;
        api_description.register(sign_in::fresh::auth_fresh_code_post)?;

        #[cfg(feature = "plus")]
        {
            // Sign in with GitHub or Google
            if http_options {
                api_description.register(provider::auth_provider_start_options)?;
                api_description.register(provider::auth_provider_finish_options)?;
                api_description.register(provider::auth_fresh_provider_start_options)?;
            }
            api_description.register(provider::auth_provider_start_post)?;
            api_description.register(provider::auth_provider_finish_post)?;
            api_description.register(provider::auth_fresh_provider_start_post)?;

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

    let remote_ip = context.remote_ip(log, headers);
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

/// Record that the user proved they hold `email`.
async fn verify_email(
    context: &bencher_schema::ApiContext,
    query_user: &QueryUser,
    email: &bencher_json::Email,
) -> Result<(), dropshot::HttpError> {
    QueryUser::verify_email(
        bencher_schema::write_conn!(context),
        query_user.id,
        email,
        context.clock.now(),
    )
    .map(|_updated| ())
    .map_err(|e| {
        bencher_schema::error::issue_error(
            "Failed to record a verified email",
            &format!(
                "Failed to record a verified email for user ({})",
                query_user.uuid
            ),
            e,
        )
    })
}
