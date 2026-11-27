#![cfg(feature = "plus")]

use std::fmt;

use bencher_endpoint::{Post, ResponseAccepted};
use bencher_json::{
    Email, JsonSignedIn, JsonSignup, OAuthAuthorizationUuid, PlanLevel, SessionMethod,
    SessionToken, UserName,
};
use bencher_schema::{
    ApiContext, auth_conn,
    error::{issue_error, payment_required_error},
    model::{
        organization::{QueryOrganization, plan::LicenseUsage, sso::QuerySso},
        user::{
            InsertUser, QueryUser,
            session::{InsertSession, QuerySession, cookie::SessionCookie},
        },
    },
    public_conn, write_conn, write_transaction,
};
use dropshot::{HttpError, RequestInfo};
use http::header::SET_COOKIE;
use slog::Logger;

use crate::{sign_in::Presented, verify_email};

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
            public_conn!(context),
            &context.licensor,
            Some(PlanLevel::Enterprise),
        )?
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
) -> Result<ProviderSignIn, HttpError> {
    // If the user already exists, then we just need to check if they are locked and possible accept an invite
    // Otherwise, we need to create a new user and notify the admins
    let query_user = QueryUser::get_with_email(public_conn!(context), &email);
    let (query_user, auth_action) = if let Ok(query_user) = query_user {
        query_user.check_is_locked()?;
        query_user.rate_limit_auth(context)?;

        if let Some(invite) = oauth_state.invite() {
            query_user
                .accept_invite(context, &context.token_key, invite)
                .await?;

            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::UserAccept(Some(
                bencher_otel::AuthMethod::OAuth(provider.into()),
            )));
        } else if let Some(organization_uuid) = oauth_state.claim() {
            let query_organization =
                QueryOrganization::from_uuid(auth_conn!(context), organization_uuid)?;
            query_organization.claim(log, context, &query_user).await?;
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
            authorization: None,
        };

        let invited = json_signup.invite.is_some();
        let insert_user =
            InsertUser::from_json(write_conn!(context), &context.token_key, &json_signup)?;
        insert_user.rate_limit_auth(context)?;

        insert_user.notify(
            log,
            public_conn!(context),
            &context.messenger,
            &context.console_url,
            invited,
            provider.as_ref(),
        )?;

        let query_user = QueryUser::get_with_email(public_conn!(context), &email)?;

        (query_user, AuthAction::Signup)
    };

    // Both providers vouch only for an address they verified.
    verify_email(context, &query_user, &email).await?;

    #[cfg(feature = "otel")]
    let auth_method = bencher_otel::AuthMethod::OAuth(provider.into());

    // Only use SSO data for Google OAuth2
    // GitHub OAuth2 won't check that an email is still active for every authentication
    #[cfg(feature = "plus")]
    if let OAuthProvider::Google = provider {
        QuerySso::join_all(
            context,
            &query_user,
            #[cfg(feature = "otel")]
            auth_method,
        )
        .await?;
    }

    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(match auth_action {
        AuthAction::Signup => bencher_otel::ApiCounter::UserSignup(auth_method),
        AuthAction::Login => bencher_otel::ApiCounter::UserLogin(auth_method),
    });

    Ok(ProviderSignIn {
        user: query_user,
        new: matches!(auth_action, AuthAction::Signup),
        plan: oauth_state.plan(),
        authorization: oauth_state.authorization(),
    })
}

/// Who a provider signed in, and what the sign-in carries to where it lands.
struct ProviderSignIn {
    user: QueryUser,
    /// Whether the sign-in made the account.
    new: bool,
    plan: Option<PlanLevel>,
    authorization: Option<OAuthAuthorizationUuid>,
}

/// Begin a session for a provider's sign-in, in place of any the browser presented, and answer
/// with its cookie and no token.
async fn sign_in(
    context: &ApiContext,
    request: &RequestInfo,
    provider: OAuthProvider,
    signed_in: ProviderSignIn,
) -> Result<ResponseAccepted<JsonSignedIn>, HttpError> {
    let ProviderSignIn {
        user,
        new,
        plan,
        authorization,
    } = signed_in;
    let token = begin_session(context, &Presented::new(context, request), provider, &user).await?;
    let cookie = SessionCookie::new(&context.console_url)
        .set(&token)
        .map_err(|e| {
            issue_error(
                "Failed to set the session cookie",
                "Failed to set the session cookie:",
                e,
            )
        })?;
    let mut response = Post::pub_response_accepted(JsonSignedIn {
        user: user.into_json(),
        new,
        plan,
        authorization,
    });
    response.headers_mut().append(SET_COOKIE, cookie);
    Ok(response)
}

async fn begin_session(
    context: &ApiContext,
    presented: &Presented<'_>,
    provider: OAuthProvider,
    user: &QueryUser,
) -> Result<SessionToken, HttpError> {
    let now = context.clock.now();
    let (session, token) = InsertSession::new(user.id, provider.into(), presented.user_agent, now)?;
    write_transaction!(context, |conn| {
        if let Some(presented) = &presented.session {
            QuerySession::delete_by_token(conn, presented)?;
        }
        session.insert(conn, now)
    })
    .map_err(|e| {
        issue_error(
            "Failed to sign in",
            "Failed to begin a provider's session:",
            e,
        )
    })?;
    Ok(token)
}

impl From<OAuthProvider> for SessionMethod {
    fn from(provider: OAuthProvider) -> Self {
        match provider {
            OAuthProvider::GitHub => Self::Github,
            OAuthProvider::Google => Self::Google,
        }
    }
}

enum AuthAction {
    Signup,
    Login,
}

#[cfg(feature = "otel")]
impl From<OAuthProvider> for bencher_otel::OAuthProvider {
    fn from(provider: OAuthProvider) -> Self {
        match provider {
            OAuthProvider::GitHub => Self::GitHub,
            OAuthProvider::Google => Self::Google,
        }
    }
}

#[cfg(test)]
mod tests {
    use bencher_api_tests::{TestServer, helpers::user_organizations};
    use bencher_json::{Email, OAuthAuthorizationUuid, OrganizationUuid, SessionMethod};
    use bencher_schema::{
        model::user::{QueryUser, session::QuerySession},
        schema,
    };
    use diesel::{ExpressionMethods as _, RunQueryDsl as _};

    use super::{OAuthProvider, OAuthState, Presented, begin_session, handle_oauth_user};

    // Kills a provider sign-in that loses the app's request to connect between the state it sent
    // the provider and the signed-in user the console reads.
    #[tokio::test]
    async fn provider_sign_in_returns_to_the_authorization_in_its_state() {
        let server = TestServer::new().await;
        let log = slog::Logger::root(slog::Discard, slog::o!());
        let authorization = OAuthAuthorizationUuid::new();
        let state = OAuthState {
            invite: None,
            claim: None,
            plan: None,
            authorization: Some(authorization),
        }
        .encode(&log, server.token_key())
        .unwrap();

        let oauth_state = OAuthState::decode(&log, server.token_key(), &state).unwrap();
        let user = handle_oauth_user(
            &log,
            server.context(),
            oauth_state,
            "Provider User".parse().unwrap(),
            "provider@example.com".parse().unwrap(),
            OAuthProvider::GitHub,
        )
        .await
        .unwrap();
        assert_eq!(user.authorization, Some(authorization));
    }

    // Kills a provider sign in that is not recorded as proof of the address, whether it signs a
    // new user up or an existing one in.
    #[tokio::test]
    async fn provider_sign_in_verifies_the_email() {
        let server = TestServer::new().await;
        let log = slog::Logger::root(slog::Discard, slog::o!());
        let email: Email = "provider@example.com".parse().unwrap();
        for provider in [OAuthProvider::GitHub, OAuthProvider::Google] {
            diesel::update(schema::user::table)
                .set(schema::user::email_verified.eq(None::<bencher_json::DateTime>))
                .execute(&mut server.db_conn())
                .unwrap();
            handle_oauth_user(
                &log,
                server.context(),
                OAuthState::new(None, None, None),
                "Provider User".parse().unwrap(),
                email.clone(),
                provider,
            )
            .await
            .unwrap();
            let query_user = QueryUser::get_with_email(&mut server.db_conn(), &email).unwrap();
            assert!(query_user.email_verified.is_some(), "{provider}");
        }
    }

    // Kills a provider's session recorded as another way of signing in, or as the email's.
    #[tokio::test]
    async fn a_provider_session_names_its_provider() {
        let server = TestServer::new().await;
        let user = server.signup("Provider User", "provider@example.com").await;
        let query_user = QueryUser::get_with_email(&mut server.db_conn(), &user.email).unwrap();
        let presented = Presented {
            user_agent: None,
            session: None,
        };
        for (provider, method) in [
            (OAuthProvider::GitHub, SessionMethod::Github),
            (OAuthProvider::Google, SessionMethod::Google),
        ] {
            let token = begin_session(server.context(), &presented, provider, &query_user)
                .await
                .unwrap();
            let (_, session) = QuerySession::from_token(&mut server.db_conn(), &token).unwrap();
            assert_eq!(session.method, method, "{provider}");
        }
    }

    // Kills a provider signup that makes a new user a member of an organization that already has
    // members.
    #[tokio::test]
    async fn provider_signup_claim_claimed_organization() {
        let server = TestServer::new().await;
        let log = slog::Logger::root(slog::Discard, slog::o!());
        let leader = server.signup("Leader", "leader@example.com").await;
        let organization = server.create_org(&leader, "Claimed").await;

        let user = handle_oauth_user(
            &log,
            server.context(),
            OAuthState::new(None, Some(organization.uuid), None),
            "Provider User".parse().unwrap(),
            "provider@example.com".parse().unwrap(),
            OAuthProvider::GitHub,
        )
        .await
        .unwrap()
        .user
        .uuid;
        assert_eq!(
            user_organizations(&server, user),
            [OrganizationUuid::from(user)]
        );
    }
}
