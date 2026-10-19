use std::net::IpAddr;

use bencher_cimd::{OAuthClientError, Throttled};
use bencher_endpoint::{CorsResponse, Endpoint, Get};
use bencher_json::{
    OAuthAuthorizationUuid, OAuthClientId, OAuthCodeChallenge, OAuthMaxAge, OAuthNonce,
    OAuthRedirectUri, OAuthRequestState, OAuthResource, OAuthScope, OAuthScopes,
};
use bencher_schema::{
    context::{ApiContext, OAuthServer, RateLimiting},
    error::issue_error,
    model::user::oauth_authorization::NewOAuthAuthorization,
    write_transaction,
};
use dropshot::{Body, HttpError, RequestContext, endpoint};
use http::Response;
use slog::Logger;
use url::Url;

use crate::{
    AUTHORIZATION_CODE_RESPONSE, S256, oauth_server,
    params::Params,
    response::{client_redirect, consent_page, found},
    supported_scopes,
};

const RESPONSE_TYPE: &str = "response_type";
const CLIENT_ID: &str = "client_id";
const REDIRECT_URI: &str = "redirect_uri";
const SCOPE: &str = "scope";
const STATE: &str = "state";
const CODE_CHALLENGE: &str = "code_challenge";
const CODE_CHALLENGE_METHOD: &str = "code_challenge_method";
const RESOURCE: &str = "resource";
const NONCE: &str = "nonce";
const MAX_AGE: &str = "max_age";
const PROMPT: &str = "prompt";
const PARAMETERS: [&str; 11] = [
    RESPONSE_TYPE,
    CLIENT_ID,
    REDIRECT_URI,
    SCOPE,
    STATE,
    CODE_CHALLENGE,
    CODE_CHALLENGE_METHOD,
    RESOURCE,
    NONCE,
    MAX_AGE,
    PROMPT,
];

#[endpoint {
    method = OPTIONS,
    path = "/oauth/authorize",
    unpublished = true,
}]
pub async fn oauth_authorize_options(
    rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    oauth_server(rqctx.context())?;
    Ok(Endpoint::cors(&[Get.into()]))
}

/// OAuth authorization endpoint
///
/// Checks an app's request to connect to a user's account, then sends the browser to the
/// console to ask the user. A refusal goes back to the app only when its client and redirect URI
/// are trusted and the consent page vouches for it; any other goes to the console's error page.
#[endpoint {
    method = GET,
    path = "/oauth/authorize",
    unpublished = true,
}]
pub async fn oauth_authorize_get(
    rqctx: RequestContext<ApiContext>,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let oauth = oauth_server(context)?;
    let remote_ip = RateLimiting::remote_ip(&rqctx.log, rqctx.request.headers());
    let query = rqctx.request.uri().query().unwrap_or_default();
    let location = match authorize(&rqctx.log, context, oauth, remote_ip, query).await {
        Ok(uuid) => {
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OAuthAuthorize);
            consent_page(&context.console_url, ("request", &uuid.to_string()))
        },
        Err(refusal) => {
            let error = refusal.error();
            slog::info!(rqctx.log, "OAuth authorization request refused";
                "error" => error.code(),
                "description" => error.description(),
            );
            #[cfg(feature = "otel")]
            bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OAuthAuthorizeFailed(
                error.reason(),
            ));
            match refusal {
                Refusal::Page(error) => consent_page(&context.console_url, ("error", error.code())),
                Refusal::Client(_, location) => location,
            }
        },
    };
    found(&location)
}

/// Check a request and record it as pending, in the order that keeps a client's redirect URI
/// untrusted until both the client and that URI are known good.
async fn authorize(
    log: &Logger,
    context: &ApiContext,
    oauth: &OAuthServer,
    remote_ip: Option<IpAddr>,
    query: &str,
) -> Result<OAuthAuthorizationUuid, Refusal> {
    if let Some(remote_ip) = remote_ip {
        context
            .rate_limiting
            .public_request(remote_ip)
            .map_err(|_err| Refusal::Page(AuthorizeError::Throttled))?;
    }
    let params = Params::parse(query.as_bytes(), &PARAMETERS)
        .ok_or(Refusal::Page(AuthorizeError::Repeated))?;
    let (Some(client_id), Some(redirect_uri)) = (params.get(CLIENT_ID), params.get(REDIRECT_URI))
    else {
        return Err(Refusal::Page(AuthorizeError::Missing));
    };
    let client_id = client_id
        .parse::<OAuthClientId>()
        .map_err(|_err| Refusal::Page(AuthorizeError::Client))?;
    let charge = || {
        remote_ip.map_or(Ok(()), |remote_ip| {
            context
                .rate_limiting
                .public_auth_attempt(remote_ip)
                .map_err(|_err| Throttled)
        })
    };
    let metadata = oauth
        .clients()
        .resolve(&client_id, &context.clock, charge)
        .await
        .map_err(|error| {
            // Only the log may say why, since the reason can tell a client which internal
            // names resolve.
            slog::warn!(log, "OAuth client refused";
                "client_id" => %client_id,
                "error" => bencher_egress::error_chain(&error),
            );
            Refusal::Page(AuthorizeError::from(error))
        })?;
    let redirect_uri = redirect_uri
        .parse::<OAuthRedirectUri>()
        .ok()
        .filter(|redirect_uri| metadata.admits(redirect_uri))
        .ok_or(Refusal::Page(AuthorizeError::RedirectUri))?;

    // The client and its redirect URI are trusted from here on.
    let refuse_with =
        |state, error| Refusal::client(oauth, &client_id, &redirect_uri, state, error);
    let state = params
        .value::<OAuthRequestState>(STATE)
        .map_err(|()| refuse_with(None, AuthorizeError::State))?;
    let refuse = |error| refuse_with(state.as_ref(), error);
    match params.get(RESPONSE_TYPE) {
        Some(AUTHORIZATION_CODE_RESPONSE) => {},
        Some(_) => return Err(refuse(AuthorizeError::ResponseType)),
        None => return Err(refuse(AuthorizeError::ResponseTypeMissing)),
    }
    if params.get(CODE_CHALLENGE_METHOD) != Some(S256) {
        return Err(refuse(AuthorizeError::CodeChallengeMethod));
    }
    let Ok(Some(code_challenge)) = params.value::<OAuthCodeChallenge>(CODE_CHALLENGE) else {
        return Err(refuse(AuthorizeError::CodeChallenge));
    };
    let resource = if let Some(resource) = params.get(RESOURCE) {
        resource
            .parse::<OAuthResource>()
            .ok()
            .filter(|resource| oauth.serves(resource))
            .ok_or_else(|| refuse(AuthorizeError::Resource))?
    } else {
        oauth.canonical_resource().clone()
    };
    let nonce = params
        .value::<OAuthNonce>(NONCE)
        .map_err(|()| refuse(AuthorizeError::Nonce))?;
    let max_age = params
        .value::<OAuthMaxAge>(MAX_AGE)
        .map_err(|()| refuse(AuthorizeError::MaxAge))?;
    if let Some(error) = prompt_refusal(params.get(PROMPT)) {
        return Err(refuse(error));
    }

    let authorization = NewOAuthAuthorization {
        client_id: client_id.clone(),
        client_name: metadata.name.clone(),
        redirect_uri: redirect_uri.clone(),
        scope: scopes(oauth, params.get(SCOPE)),
        resource,
        code_challenge,
        state: state.clone(),
        nonce,
        max_age,
        refresh: metadata.refresh,
    };
    let now = context.clock.now();
    write_transaction!(context, |conn| authorization.insert(conn, now)).map_err(|error| {
        let _err = issue_error(
            "Failed to record an OAuth authorization request",
            "Failed to record an OAuth authorization request:",
            error,
        );
        refuse(AuthorizeError::Server)
    })
}

/// Why consent cannot do what a request's `prompt` asks, if it cannot.
fn prompt_refusal(prompt: Option<&str>) -> Option<AuthorizeError> {
    let prompts = || prompt.unwrap_or_default().split(' ');
    // Consent is always asked for, so a request that forbids asking cannot succeed.
    if prompts().any(|value| value == "none") {
        Some(AuthorizeError::ConsentRequired)
    } else if prompts().any(|value| value == "login") {
        // Nor can asking for consent make anyone sign in again.
        Some(AuthorizeError::LoginRequired)
    } else {
        None
    }
}

/// The scopes asked for that this server grants, with reading projects when none of them reach
/// projects, so that every grant can use the MCP server.
fn scopes(oauth: &OAuthServer, scope: Option<&str>) -> OAuthScopes {
    let scopes = supported_scopes(oauth, scope);
    if scopes.iter().any(OAuthScope::is_project) {
        scopes
    } else {
        scopes.iter().chain([OAuthScope::ProjectsRead]).collect()
    }
}

/// Why an authorization request is refused, and who hears about it.
enum Refusal {
    /// The console's error page, for a client or redirect URI that is untrusted or unverified.
    Page(AuthorizeError),
    /// The client, at its redirect URI, which carries the error.
    Client(AuthorizeError, Url),
}

impl Refusal {
    /// To the client at its trusted redirect URI, only when the consent page vouches for it:
    /// anyone can register a client, so any other client's redirect could lead anywhere.
    fn client(
        oauth: &OAuthServer,
        client_id: &OAuthClientId,
        redirect_uri: &OAuthRedirectUri,
        state: Option<&OAuthRequestState>,
        error: AuthorizeError,
    ) -> Self {
        if !oauth.clients().is_verified(client_id, redirect_uri) {
            return Self::Page(error);
        }
        let location = client_redirect(
            redirect_uri,
            &[
                ("error", error.code()),
                ("error_description", error.description()),
            ],
            state,
            oauth.issuer(),
        );
        Self::Client(error, location)
    }

    fn error(&self) -> AuthorizeError {
        match self {
            Self::Page(error) | Self::Client(error, _) => *error,
        }
    }
}

#[derive(Debug, Clone, Copy)]
enum AuthorizeError {
    Throttled,
    Repeated,
    Missing,
    Client,
    ClientFetch,
    ClientDocument,
    ClientBusy,
    RedirectUri,
    State,
    ResponseType,
    ResponseTypeMissing,
    CodeChallengeMethod,
    CodeChallenge,
    Resource,
    Nonce,
    MaxAge,
    ConsentRequired,
    LoginRequired,
    Server,
}

impl From<OAuthClientError> for AuthorizeError {
    fn from(error: OAuthClientError) -> Self {
        match error {
            OAuthClientError::NotAllowed => Self::Client,
            OAuthClientError::Throttled => Self::Throttled,
            OAuthClientError::Busy => Self::ClientBusy,
            OAuthClientError::Fetch(_) => Self::ClientFetch,
            OAuthClientError::Document(_) => Self::ClientDocument,
        }
    }
}

impl AuthorizeError {
    /// The OAuth error code, which the console's error page explains in its own words.
    fn code(self) -> &'static str {
        match self {
            Self::Repeated
            | Self::Missing
            | Self::State
            | Self::ResponseTypeMissing
            | Self::CodeChallengeMethod
            | Self::CodeChallenge
            | Self::Nonce
            | Self::MaxAge => "invalid_request",
            Self::Client | Self::ClientFetch | Self::ClientDocument => "invalid_client",
            Self::Throttled | Self::ClientBusy => "temporarily_unavailable",
            Self::RedirectUri => "invalid_redirect_uri",
            Self::ResponseType => "unsupported_response_type",
            Self::Resource => "invalid_target",
            Self::ConsentRequired => "consent_required",
            Self::LoginRequired => "login_required",
            Self::Server => "server_error",
        }
    }

    /// Why, for the client: fixed text that never quotes the request.
    fn description(self) -> &'static str {
        match self {
            Self::Throttled | Self::ClientBusy => "Too many requests, please try again later",
            Self::Repeated => "A request parameter is repeated",
            Self::Missing => "The client_id and redirect_uri parameters are required",
            Self::Client | Self::ClientFetch | Self::ClientDocument => {
                "The client ID does not lead to a valid client metadata document"
            },
            Self::RedirectUri => "The redirect_uri is not one the client registered",
            Self::State => "The state parameter is longer than 1 KiB",
            Self::ResponseType => "Only the code response type is supported",
            Self::ResponseTypeMissing => "The response_type parameter is required",
            Self::CodeChallengeMethod => "The code_challenge_method parameter must be S256",
            Self::CodeChallenge => "The code_challenge parameter must be 43 base64url characters",
            Self::Resource => "The resource is not one this server issues tokens for",
            Self::Nonce => "The nonce parameter is longer than 256 bytes",
            Self::MaxAge => "The max_age parameter must be a number of seconds",
            Self::ConsentRequired => "The user is always asked for consent",
            Self::LoginRequired => "The user cannot be asked to sign in again",
            Self::Server => "The request could not be recorded, please try again",
        }
    }

    #[cfg(feature = "otel")]
    fn reason(self) -> bencher_otel::OAuthAuthorizeFailureReason {
        use bencher_otel::OAuthAuthorizeFailureReason as Reason;
        match self {
            Self::Repeated
            | Self::Missing
            | Self::State
            | Self::ResponseTypeMissing
            | Self::CodeChallengeMethod
            | Self::CodeChallenge
            | Self::Nonce
            | Self::MaxAge => Reason::InvalidRequest,
            Self::Client => Reason::InvalidClient,
            Self::ClientFetch => Reason::ClientFetch,
            Self::ClientDocument => Reason::ClientDocument,
            Self::ClientBusy => Reason::ClientBusy,
            Self::Throttled => Reason::Throttled,
            Self::RedirectUri => Reason::RedirectUri,
            Self::ResponseType => Reason::ResponseType,
            Self::Resource => Reason::Resource,
            Self::ConsentRequired => Reason::ConsentRequired,
            Self::LoginRequired => Reason::LoginRequired,
            Self::Server => Reason::ServerError,
        }
    }
}

#[cfg(test)]
mod tests {
    use bencher_json::system::config::JsonOAuthServer;
    use bencher_schema::context::OAuthServer;
    use serde_json::json;

    use super::{AuthorizeError, Refusal};

    // Kills a refusal sent to the redirect URI of a client the consent page does not vouch for:
    // only a vouched-for client hears a refusal at its redirect, and every other refusal shows on
    // the console's error page.
    #[test]
    fn only_a_verified_client_hears_why_it_was_refused() {
        let oauth = OAuthServer::try_from(
            serde_json::from_value::<JsonOAuthServer>(json!({"issuer": "https://api.example.com"}))
                .unwrap(),
        )
        .unwrap();
        let state = "af0ifjsldkj".parse().unwrap();

        let refusal = Refusal::client(
            &oauth,
            &"https://anyone.example/oauth/client.json".parse().unwrap(),
            &"https://anyone.example/landing".parse().unwrap(),
            Some(&state),
            AuthorizeError::ResponseType,
        );
        assert!(matches!(
            refusal,
            Refusal::Page(AuthorizeError::ResponseType)
        ));

        let refusal = Refusal::client(
            &oauth,
            &"https://chatgpt.com/oauth/client.json".parse().unwrap(),
            &"https://chatgpt.com/connector_platform_oauth_redirect"
                .parse()
                .unwrap(),
            Some(&state),
            AuthorizeError::ResponseType,
        );
        let Refusal::Client(AuthorizeError::ResponseType, location) = refusal else {
            panic!("Expected the verified client to hear the refusal");
        };
        assert_eq!(
            location.as_str(),
            "https://chatgpt.com/connector_platform_oauth_redirect?error=unsupported_response_type&error_description=Only+the+code+response+type+is+supported&state=af0ifjsldkj&iss=https%3A%2F%2Fapi.example.com"
        );
    }
}
