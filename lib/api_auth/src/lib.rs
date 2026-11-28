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

#[cfg(not(feature = "plus"))]
use schemars as _;
#[cfg(not(feature = "plus"))]
use serde as _;

mod claim;
mod email_change;
mod invite;
pub mod oci;
mod provider;
mod session;
mod sign_in;

pub struct Api;

impl bencher_endpoint::Registrar for Api {
    fn register(
        api_description: &mut dropshot::ApiDescription<bencher_schema::ApiContext>,
        http_options: bool,
        #[cfg(feature = "plus")] _is_bencher_cloud: bool,
    ) -> Result<(), dropshot::ApiDescriptionRegisterError> {
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

        // Email changes
        if http_options {
            api_description.register(email_change::auth_email_change_options)?;
            api_description.register(email_change::auth_email_change_confirm_options)?;
        }
        api_description.register(email_change::auth_email_change_post)?;
        api_description.register(email_change::auth_email_change_confirm_post)?;

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

/// The app's request to connect a sign-in returns to, dropped unless it still waits for an answer.
#[cfg(feature = "plus")]
async fn pending_authorization(
    context: &bencher_schema::ApiContext,
    authorization: Option<bencher_json::OAuthAuthorizationUuid>,
) -> Result<Option<bencher_json::OAuthAuthorizationUuid>, dropshot::HttpError> {
    use bencher_schema::model::user::oauth_authorization::QueryOAuthAuthorization;

    let Some(uuid) = authorization else {
        return Ok(None);
    };
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
