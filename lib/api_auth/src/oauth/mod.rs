use bencher_json::{Email, JsonAuthUser, JsonSignup, PlanLevel, UserName};
use bencher_schema::{
    ApiContext, conn_lock,
    error::{issue_error, payment_required_error},
    model::{
        organization::{QueryOrganization, plan::LicenseUsage},
        user::{InsertUser, QueryUser},
    },
};
use dropshot::HttpError;
use slog::Logger;

use crate::CLIENT_TOKEN_TTL;

pub mod github;
pub mod google;
mod oauth_state;

use oauth_state::OAuthState;

async fn is_allowed_oauth(context: &ApiContext) -> Result<(), HttpError> {
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

async fn handle_oauth_user(
    log: &Logger,
    context: &ApiContext,
    oauth_state: OAuthState,
    name: UserName,
    email: Email,
    method: &str,
) -> Result<JsonAuthUser, HttpError> {
    // If the user already exists, then we just need to check if they are locked and possible accept an invite
    // Otherwise, we need to create a new user and notify the admins
    let query_user = QueryUser::get_with_email(conn_lock!(context), &email);
    let user = if let Ok(query_user) = query_user {
        query_user.check_is_locked()?;
        if let Some(invite) = oauth_state.invite() {
            query_user.accept_invite(conn_lock!(context), &context.token_key, invite)?;
        } else if let Some(organization_uuid) = oauth_state.claim() {
            let query_organization =
                QueryOrganization::from_uuid(conn_lock!(context), organization_uuid)?;
            query_organization.claim(context, &query_user).await?;
        }
        query_user
    } else {
        let json_signup = JsonSignup {
            name,
            slug: None,
            email: email.clone(),
            plan: oauth_state.plan(),
            invite: oauth_state.invite().cloned(),
            claim: oauth_state.claim(),
            i_agree: true,
        };

        let invited = json_signup.invite.is_some();
        let insert_user =
            InsertUser::from_json(conn_lock!(context), &context.token_key, &json_signup)?;

        insert_user.notify(
            log,
            conn_lock!(context),
            &context.messenger,
            &context.console_url,
            invited,
            method,
        )?;

        QueryUser::get_with_email(conn_lock!(context), &email)?
    }
    .into_json();

    let token = context
        .token_key
        .new_client(email.clone(), CLIENT_TOKEN_TTL)
        .map_err(|e| {
            issue_error(
                "Failed to create client JWT for OAuth2",
                &format!("Failed to create client JWT for {method} ({email} | {CLIENT_TOKEN_TTL})"),
                e,
            )
        })?;

    let claims = context.token_key.validate_client(&token).map_err(|e| {
        issue_error(
            "Failed to validate new client JWT for OAuth2",
            &format!("Failed to validate new client JWT for {method}: {token}"),
            e,
        )
    })?;

    Ok(JsonAuthUser {
        user,
        token,
        creation: claims.issued_at(),
        expiration: claims.expiration(),
    })
}
