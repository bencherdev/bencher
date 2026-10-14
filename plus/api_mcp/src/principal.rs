use bencher_json::OAuthScopes;
use bencher_schema::{
    context::{ApiContext, OAuthServer},
    model::user::{actor::ApiActor, auth::AuthUser, oauth::is_oauth_token},
};
use dropshot::{Body, ClientErrorStatusCode, HttpError, RequestContext};
use http::{HeaderMap, HeaderValue, Response, StatusCode, header};
use rmcp::transport::common::http_header::JSON_MIME_TYPE;
use strum::IntoEnumIterator as _;

use crate::{endpoint::insert_cors_headers, tools::ToolAccess};

const INVALID_TOKEN_CHALLENGE: &str =
    r#"Bearer error="invalid_token", error_description="The access token is invalid""#;
const INSUFFICIENT_SCOPE: &str = "insufficient_scope";
const INSUFFICIENT_SCOPE_DESCRIPTION: &str = "The access token does not permit this tool";

/// Who an MCP request acts as.
#[derive(Clone)]
pub enum McpPrincipal {
    /// A Bencher credential, or none: unscoped, exactly as over REST.
    Actor(ApiActor),
    /// An OAuth access token, which may do only what its scopes permit.
    OAuth {
        actor: ApiActor,
        scopes: OAuthScopes,
    },
}

impl McpPrincipal {
    /// The MCP sibling of `ApiActor::new`: once OAuth is configured,
    /// it also accepts an OAuth access token, and every credential it refuses
    /// is answered with a challenge to sign in again.
    pub async fn new(rqctx: &RequestContext<ApiContext>) -> Result<Self, HttpError> {
        let context = rqctx.context();
        let Some(oauth) = &context.oauth else {
            return ApiActor::new(rqctx).await.map(Self::Actor);
        };
        let oauth_token = rqctx
            .request
            .headers()
            .get(bencher_json::AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .and_then(bencher_json::strip_bearer_token)
            .filter(|token| is_oauth_token(token));
        let principal = if let Some(token) = oauth_token {
            AuthUser::from_oauth_token(context, oauth, token)
                .await
                .map(|(auth_user, scopes)| Self::OAuth {
                    actor: auth_user.into(),
                    scopes,
                })
        } else {
            ApiActor::new(rqctx).await.map(Self::Actor)
        };
        principal.map_err(|error| with_invalid_token_challenge(error, oauth, rqctx))
    }

    pub fn actor(&self) -> &ApiActor {
        match self {
            Self::Actor(actor) | Self::OAuth { actor, .. } => actor,
        }
    }

    pub fn permits(&self, access: ToolAccess) -> bool {
        match self {
            Self::Actor(_) => true,
            Self::OAuth { scopes, .. } => scopes.permits(access.scope()),
        }
    }
}

/// An authorization challenge, answered in place of the SDK's response.
#[derive(Debug, Clone, Copy)]
pub enum Challenge {
    /// The access token's scopes do not permit the tool.
    InsufficientScope,
}

impl Challenge {
    pub const fn description(self) -> &'static str {
        match self {
            Self::InsufficientScope => INSUFFICIENT_SCOPE_DESCRIPTION,
        }
    }

    pub fn into_response(self) -> Result<Response<Body>, HttpError> {
        let body = serde_json::json!({
            "error": INSUFFICIENT_SCOPE,
            "error_description": self.description(),
        });
        // Every scope a tool can need, so one consent covers every tool
        let scope: OAuthScopes = ToolAccess::iter().map(ToolAccess::scope).collect();
        let challenge = format!(r#"Bearer error="{INSUFFICIENT_SCOPE}", scope="{scope}""#);
        let mut response = Response::builder()
            .status(StatusCode::FORBIDDEN)
            .header(header::CONTENT_TYPE, JSON_MIME_TYPE)
            .header(header::WWW_AUTHENTICATE, challenge)
            .body(body.to_string().into())?;
        insert_challenge_cors_headers(response.headers_mut());
        Ok(response)
    }
}

/// Point a refused client at the metadata of the MCP server it reached, where it finds how to get
/// a new access token.
fn with_invalid_token_challenge(
    mut error: HttpError,
    oauth: &OAuthServer,
    rqctx: &RequestContext<ApiContext>,
) -> HttpError {
    if error.status_code == ClientErrorStatusCode::UNAUTHORIZED {
        let resource_metadata = oauth
            .resource_at(rqctx.request.headers(), rqctx.request.uri())
            .metadata_url();
        let challenge =
            format!(r#"{INVALID_TOKEN_CHALLENGE}, resource_metadata="{resource_metadata}""#);
        // A resource is a normalized URL, which is always a valid header value.
        let challenge = HeaderValue::from_str(&challenge)
            .unwrap_or_else(|_err| HeaderValue::from_static(INVALID_TOKEN_CHALLENGE));
        let headers = error.headers_mut();
        headers.insert(header::WWW_AUTHENTICATE, challenge);
        insert_challenge_cors_headers(headers);
    }
    error
}

/// A browser client can only read `WWW-Authenticate` once it is exposed.
fn insert_challenge_cors_headers(headers: &mut HeaderMap) {
    insert_cors_headers(headers);
    headers.insert(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static("WWW-Authenticate"),
    );
}
