#![cfg(feature = "plus")]
//! Sign in with GitHub or Google: a state row that only the browser that started it can finish,
//! once, a PKCE exchange, and the one rule that links a provider account to a user.

use bencher_endpoint::{CorsResponse, Endpoint, Post, ResponseOk};
use bencher_github_client::GitHubClient;
use bencher_google_client::GoogleClient;
use bencher_json::{
    DateTime, Email, JsonOAuthUrl, JsonProviderFinish, JsonProviderSignIn, JsonProviderStart,
    NonEmpty, PlanLevel, Secret, SignInProvider, SignInRefusal, UserName,
};
use bencher_schema::{
    auth_conn,
    context::ApiContext,
    error::{issue_error, payment_required_error, unauthorized_error},
    model::{
        organization::plan::LicenseUsage,
        user::{
            auth::AuthUser,
            provider::{InsertProviderState, ProviderPin, ProviderSecrets, QueryProviderState},
            session::{QuerySession, SessionId, cookie::ProviderCookie},
        },
    },
    public_conn, write_transaction,
};
use dropshot::{HttpError, Path, RequestContext, TypedBody, endpoint};
use http::{
    HeaderValue,
    header::{CACHE_CONTROL, SET_COOKIE},
};
use schemars::JsonSchema;
use serde::Deserialize;
use slog::Logger;
use url::Url;

use crate::sign_in::{admit, email::sign_in_context, fresh::session_of, refused};

mod link;

/// One message for every failure of a provider's sign-in, so none tells why.
const REFUSED: &str = "The sign-in failed";
/// The console's page that GitHub sends the reader back to.
const GITHUB_SIGN_IN_PATH: &str = "/auth/github";

#[derive(Deserialize, JsonSchema)]
pub struct ProviderParams {
    pub provider: SignInProvider,
}

#[endpoint {
    method = OPTIONS,
    path = "/v0/auth/providers/{provider}/start",
    unpublished = true,
}]
pub async fn auth_provider_start_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProviderParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

/// Start a sign-in with a provider
///
/// Bind a new sign-in to this browser by its cookie, and answer with the provider's authorize URL.
#[endpoint {
    method = POST,
    path = "/v0/auth/providers/{provider}/start",
    unpublished = true,
}]
pub async fn auth_provider_start_post(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProviderParams>,
    body: TypedBody<JsonProviderStart>,
) -> Result<ResponseOk<JsonOAuthUrl>, HttpError> {
    let log = &rqctx.log;
    let context = rqctx.context();
    let ProviderParams { provider } = path_params.into_inner();
    admit(log, context, &rqctx.request)?;
    let client = Client::new(log, context, provider).await?;
    let JsonProviderStart {
        invite,
        claim,
        plan,
        authorization,
    } = body.into_inner();
    let carried = sign_in_context(context, None, invite, claim, plan, authorization).await?;
    let now = context.clock.now();
    start(
        context,
        &client,
        InsertProviderState::new(provider, &carried, now)?,
        now,
    )
    .await
}

#[endpoint {
    method = OPTIONS,
    path = "/v0/auth/fresh/providers/{provider}/start",
    unpublished = true,
}]
pub async fn auth_fresh_provider_start_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProviderParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

/// Sign in again with a provider
///
/// Bind a fresh sign-in of the signed-in reader's session to this browser by its cookie, and
/// answer with the provider's authorize URL. Its finish proves the session again only for the
/// provider account pinned to the session's user.
#[endpoint {
    method = POST,
    path = "/v0/auth/fresh/providers/{provider}/start",
    unpublished = true,
}]
pub async fn auth_fresh_provider_start_post(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProviderParams>,
) -> Result<ResponseOk<JsonOAuthUrl>, HttpError> {
    let log = &rqctx.log;
    let context = rqctx.context();
    let ProviderParams { provider } = path_params.into_inner();
    admit(log, context, &rqctx.request)?;
    let auth_user = AuthUser::new(&rqctx).await?;
    let session_id = session_of(&auth_user)?;
    let client = Client::new(log, context, provider).await?;
    let now = context.clock.now();
    start(
        context,
        &client,
        InsertProviderState::fresh(provider, session_id, now)?,
        now,
    )
    .await
}

/// Record the sign-in, and answer with the provider's URL and the cookie that binds it to this
/// browser.
async fn start(
    context: &ApiContext,
    client: &Client<'_>,
    (insert, secrets): (InsertProviderState, ProviderSecrets),
    now: DateTime,
) -> Result<ResponseOk<JsonOAuthUrl>, HttpError> {
    write_transaction!(context, |conn| insert.insert(conn, now)).map_err(|e| {
        issue_error(
            "Failed to start a sign-in",
            "Failed to start a provider's sign-in:",
            e,
        )
    })?;
    let url = client.authorize_url(context, &secrets)?;
    let cookie = ProviderCookie::new(&context.console_url)
        .set(&secrets.binding)
        .map_err(|e| {
            issue_error(
                "Failed to set the provider's sign-in cookie",
                "Failed to set the provider's sign-in cookie:",
                e,
            )
        })?;
    let mut response = Post::pub_response_ok(JsonOAuthUrl { url });
    let headers = response.headers_mut();
    headers.append(SET_COOKIE, cookie);
    // Every start makes a state of its own, so no cache may answer one start with another's.
    headers.insert(CACHE_CONTROL, HeaderValue::from_static("no-store"));
    Ok(response)
}

#[endpoint {
    method = OPTIONS,
    path = "/v0/auth/providers/{provider}/finish",
    unpublished = true,
}]
pub async fn auth_provider_finish_options(
    _rqctx: RequestContext<ApiContext>,
    _path_params: Path<ProviderParams>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

/// Finish a sign-in with a provider
///
/// In the browser that started it, and only once, exchange the provider's code for the account it
/// names. An account pinned to a user signs that user in; a new address signs up, pinned; and an
/// address that has an account is emailed, to prove it before the account pins. A fresh sign-in
/// proves its session again only by the account pinned to the session's user.
#[endpoint {
    method = POST,
    path = "/v0/auth/providers/{provider}/finish",
    unpublished = true,
}]
pub async fn auth_provider_finish_post(
    rqctx: RequestContext<ApiContext>,
    path_params: Path<ProviderParams>,
    body: TypedBody<JsonProviderFinish>,
) -> Result<ResponseOk<JsonProviderSignIn>, HttpError> {
    let log = &rqctx.log;
    let context = rqctx.context();
    let ProviderParams { provider } = path_params.into_inner();
    admit(log, context, &rqctx.request)?;
    let client = Client::new(log, context, provider).await?;
    let JsonProviderFinish { code, state } = body.into_inner();
    let cookie = ProviderCookie::new(&context.console_url);
    let binding = cookie.binding(rqctx.request.headers(), &context.relay);
    let now = context.clock.now();

    let started = write_transaction!(context, |conn| {
        QueryProviderState::take(conn, provider, &state, binding.as_ref(), now)
    })
    .map_err(|e| {
        issue_error(
            "Failed to finish a sign-in",
            "Failed to find a provider's sign-in:",
            e,
        )
    })?
    .ok_or_else(provider_refused)?;

    // The sign-in has ended, so whatever it answers clears its cookie.
    let finished = match client.account(context, &started, code, now).await {
        Ok(account) => {
            if let Some(session_id) = started.session_id {
                prove_again(context, session_id, &account).await
            } else {
                link::link(log, context, &rqctx.request, account, started.context()).await
            }
        },
        Err(error) => {
            slog::warn!(log, "{} refused a sign-in: {error}", provider.name());
            Err(provider_refused())
        },
    };
    match finished {
        Ok(mut response) => {
            response.headers_mut().append(SET_COOKIE, cookie.clear());
            Ok(response)
        },
        Err(mut error) => {
            error.headers_mut().append(SET_COOKIE, cookie.clear());
            Err(error)
        },
    }
}

/// Prove the session again by the provider account, only if it is pinned to the session's user,
/// who is not locked. A fresh sign-in never pins, links, or makes anything.
async fn prove_again(
    context: &ApiContext,
    session_id: SessionId,
    account: &ProviderAccount,
) -> Result<ResponseOk<JsonProviderSignIn>, HttpError> {
    let now = context.clock.now();
    let proved = write_transaction!(context, |conn| {
        let Some(user) = account.pin.user(conn)? else {
            return Ok(false);
        };
        let Some(session) = QuerySession::find(conn, session_id)? else {
            return Ok(false);
        };
        if user.id != session.user_id || user.locked {
            return Ok(false);
        }
        QuerySession::prove(conn, session_id, now).map(|proved| proved == 1)
    })
    .map_err(|e: diesel::result::Error| {
        issue_error(
            "Failed to sign in again",
            "Failed to prove a session again by a provider:",
            e,
        )
    })?;
    if proved {
        Ok(Post::pub_response_ok(JsonProviderSignIn {
            signed_in: None,
            inbox: None,
            proved: Some(now),
        }))
    } else {
        Err(provider_refused())
    }
}

/// The providers pinned to the reader that this server signs in with.
pub(crate) async fn pinned(
    context: &ApiContext,
    auth_user: &AuthUser,
) -> Result<Vec<SignInProvider>, HttpError> {
    let pinned = ProviderPin::providers_of(auth_conn!(context), auth_user.id).map_err(|e| {
        issue_error(
            "Failed to read the pinned providers",
            "Failed to read the providers pinned to a user:",
            e,
        )
    })?;
    if pinned.is_empty() || !is_licensed(context).await? {
        return Ok(Vec::new());
    }
    Ok(pinned
        .into_iter()
        .filter(|provider| Client::configured(context, *provider).is_some())
        .collect())
}

/// The one refusal of a provider's sign-in, whatever failed.
fn provider_refused() -> HttpError {
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::SignInRefused(
        bencher_otel::SignInRefusalReason::ProviderRefused,
    ));
    refused(unauthorized_error(REFUSED), SignInRefusal::ProviderRefused)
}

/// The client of a provider the server signs in with.
enum Client<'a> {
    GitHub(&'a GitHubClient),
    Google(&'a GoogleClient),
}

impl<'a> Client<'a> {
    /// The provider's client, where the server has one and may sign in with it: on Bencher Cloud,
    /// or with an Enterprise license for an organization on the server.
    async fn new(
        log: &Logger,
        context: &'a ApiContext,
        provider: SignInProvider,
    ) -> Result<Self, HttpError> {
        let Some(client) = Self::configured(context, provider) else {
            let err = format!("{} sign-in is not configured", provider.name());
            slog::warn!(log, "{err}");
            return Err(payment_required_error(err));
        };
        if is_licensed(context).await? {
            Ok(client)
        } else {
            Err(payment_required_error(format!(
                "You must have a valid Bencher Plus Enterprise license for at least one organization on the server to sign in with {}",
                provider.name()
            )))
        }
    }

    fn configured(context: &'a ApiContext, provider: SignInProvider) -> Option<Self> {
        match provider {
            SignInProvider::Github => context.github_client.as_ref().map(Self::GitHub),
            SignInProvider::Google => context.google_client.as_ref().map(Self::Google),
        }
    }

    /// Where the reader goes to sign in: the provider's authorize URL with the sign-in's state
    /// and the `S256` challenge of its verifier.
    fn authorize_url(
        &self,
        context: &ApiContext,
        secrets: &ProviderSecrets,
    ) -> Result<Url, HttpError> {
        match self {
            Self::GitHub(github) => Ok(github.sign_in_url(
                &secrets.state,
                &secrets.verifier,
                github_sign_in_url(context)?,
            )),
            Self::Google(google) => {
                let nonce = secrets.nonce.as_ref().ok_or_else(|| {
                    issue_error(
                        "Failed to start a sign-in",
                        "A Google sign-in started without a nonce",
                        "no nonce",
                    )
                })?;
                Ok(google.sign_in_url(&secrets.state, &secrets.verifier, nonce))
            },
        }
    }

    /// The account the provider's `code` names, from an exchange that proves the sign-in's
    /// verifier, with its primary verified email.
    async fn account(
        &self,
        context: &ApiContext,
        started: &QueryProviderState,
        code: Secret,
        now: DateTime,
    ) -> Result<ProviderAccount, ProviderError> {
        match self {
            Self::GitHub(github) => {
                let signed_in = github
                    .sign_in_user(code, &started.verifier, github_sign_in_url(context)?)
                    .await?;
                let name = signed_in
                    .name
                    .or_else(|| signed_in.login.as_ref().parse().ok())
                    .unwrap_or_else(|| UserName::from_email(&signed_in.email));
                Ok(ProviderAccount {
                    pin: ProviderPin {
                        provider: SignInProvider::Github,
                        account_id: signed_in.id,
                        handle: signed_in.login,
                    },
                    name,
                    email: signed_in.email,
                    hosted_domain: None,
                })
            },
            Self::Google(google) => {
                let nonce = started.nonce.as_ref().ok_or(ProviderError::NoNonce)?;
                let signed_in = google
                    .sign_in_user(code, &started.verifier, nonce, now)
                    .await?;
                let handle: NonEmpty = signed_in
                    .email
                    .as_ref()
                    .parse()
                    .map_err(ProviderError::Handle)?;
                Ok(ProviderAccount {
                    pin: ProviderPin {
                        provider: SignInProvider::Google,
                        account_id: signed_in.sub,
                        handle,
                    },
                    name: signed_in
                        .name
                        .unwrap_or_else(|| UserName::from_email(&signed_in.email)),
                    email: signed_in.email,
                    hosted_domain: signed_in.hd,
                })
            },
        }
    }
}

/// Whether the server may sign in with a provider: on Bencher Cloud, or with an Enterprise license
/// for an organization on the server.
async fn is_licensed(context: &ApiContext) -> Result<bool, HttpError> {
    Ok(context.is_bencher_cloud
        || !LicenseUsage::get_for_server(
            public_conn!(context),
            &context.licensor,
            Some(PlanLevel::Enterprise),
        )?
        .is_empty())
}

/// The console's page that GitHub sends the reader back to, named on both the authorize URL and
/// the exchange, which leaves the app's first callback URL free for another page.
fn github_sign_in_url(context: &ApiContext) -> Result<Url, HttpError> {
    context.console_url.join(GITHUB_SIGN_IN_PATH).map_err(|e| {
        issue_error(
            "Failed to build the GitHub sign-in URL",
            &format!(
                "Failed to build the GitHub sign-in URL from ({}).",
                context.console_url
            ),
            e,
        )
    })
}

/// A provider account that signed in: the pin it makes, the name a new account takes, its
/// verified email, and, for Google Workspace, its hosted domain.
pub(crate) struct ProviderAccount {
    pub pin: ProviderPin,
    pub name: UserName,
    pub email: Email,
    pub hosted_domain: Option<NonEmpty>,
}

#[derive(Debug, thiserror::Error)]
enum ProviderError {
    #[error(transparent)]
    Http(HttpError),
    #[error(transparent)]
    GitHub(#[from] bencher_github_client::GitHubClientError),
    #[error(transparent)]
    Google(#[from] bencher_google_client::GoogleClientError),
    #[error("A Google sign-in has no nonce")]
    NoNonce,
    #[error("Failed to keep the Google account's email as its handle: {0}")]
    Handle(bencher_json::ValidError),
}

impl From<HttpError> for ProviderError {
    fn from(error: HttpError) -> Self {
        Self::Http(error)
    }
}
