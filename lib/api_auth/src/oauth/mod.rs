#![cfg(feature = "plus")]

use std::fmt;

use bencher_json::{Email, JsonAuthUser, JsonOAuthUser, JsonSignup, PlanLevel, UserName};
use bencher_schema::{
    ApiContext, conn_lock,
    error::{issue_error, payment_required_error},
    model::{
        organization::{QueryOrganization, plan::LicenseUsage, sso::QuerySso},
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

#[derive(Clone, Copy)]
enum OAuthProvider {
    GitHub,
    Google,
}

impl AsRef<str> for OAuthProvider {
    fn as_ref(&self) -> &str {
        match self {
            Self::GitHub => "GitHub OAuth2",
            Self::Google => "Google OAuth2",
        }
    }
}

impl fmt::Display for OAuthProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_ref())
    }
}

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
    provider: OAuthProvider,
) -> Result<JsonOAuthUser, HttpError> {
    // If the user already exists, then we just need to check if they are locked and possible accept an invite
    // Otherwise, we need to create a new user and notify the admins
    let query_user = QueryUser::get_with_email(conn_lock!(context), &email);
    let (query_user, auth_action) = if let Ok(query_user) = query_user {
        query_user.check_is_locked()?;
        query_user.rate_limit_auth(context)?;

        if let Some(invite) = oauth_state.invite() {
            query_user.accept_invite(conn_lock!(context), &context.token_key, invite)?;

            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserAccept(Some(
                bencher_otel::AuthMethod::OAuth(provider.into()),
            )));
        } else if let Some(organization_uuid) = oauth_state.claim() {
            let query_organization =
                QueryOrganization::from_uuid(conn_lock!(context), organization_uuid)?;
            query_organization.claim(context, &query_user).await?;
        }
        (query_user, AuthAction::Login)
    } else {
        let json_signup = JsonSignup {
            name,
            slug: None,
            email: email.clone(),
            plan: oauth_state.plan(),
            invite: oauth_state.invite().cloned(),
            claim: oauth_state.claim(),
            i_agree: true,
            recaptcha_token: None,
        };

        let invited = json_signup.invite.is_some();
        let insert_user =
            InsertUser::from_json(conn_lock!(context), &context.token_key, &json_signup)?;
        insert_user.rate_limit_auth(context)?;

        insert_user.notify(
            log,
            conn_lock!(context),
            &context.messenger,
            &context.console_url,
            invited,
            provider.as_ref(),
        )?;

        let query_user = QueryUser::get_with_email(conn_lock!(context), &email)?;

        (query_user, AuthAction::Signup)
    };

    #[cfg(feature = "otel")]
    let auth_method = bencher_otel::AuthMethod::OAuth(provider.into());

    #[cfg(feature = "plus")]
    QuerySso::join_all(
        context,
        &query_user,
        #[cfg(feature = "otel")]
        auth_method,
    )
    .await?;

    let token = context
        .token_key
        .new_client(email.clone(), CLIENT_TOKEN_TTL)
        .map_err(|e| {
            issue_error(
                "Failed to create client JWT for OAuth2",
                &format!(
                    "Failed to create client JWT for {provider} {auth_action} ({email} | {CLIENT_TOKEN_TTL})"
                ),
                e,
            )
        })?;

    let claims = context.token_key.validate_client(&token).map_err(|e| {
        issue_error(
            "Failed to validate new client JWT for OAuth2",
            &format!("Failed to validate new client JWT for {provider} {auth_action}: {token}"),
            e,
        )
    })?;

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(match auth_action {
        AuthAction::Signup => bencher_otel::ApiCounter::UserSignup(auth_method),
        AuthAction::Login => bencher_otel::ApiCounter::UserLogin(auth_method),
    });

    let user = JsonAuthUser {
        user: query_user.into_json(),
        token,
        creation: claims.issued_at(),
        expiration: claims.expiration(),
    };
    Ok(JsonOAuthUser {
        user,
        plan: oauth_state.plan(),
    })
}

enum AuthAction {
    Signup,
    Login,
}

impl fmt::Display for AuthAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Signup => f.write_str("signup"),
            Self::Login => f.write_str("login"),
        }
    }
}

#[cfg(feature = "otel")]
impl From<OAuthProvider> for bencher_otel::OAuthProvider {
    fn from(provider: OAuthProvider) -> Self {
        match provider {
            OAuthProvider::GitHub => bencher_otel::OAuthProvider::GitHub,
            OAuthProvider::Google => bencher_otel::OAuthProvider::Google,
        }
    }
}
