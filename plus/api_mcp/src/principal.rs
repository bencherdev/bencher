use bencher_json::OAuthScopes;
use bencher_schema::{
    auth_conn,
    context::{ApiContext, OAuthServer},
    model::{
        project::QueryProject,
        user::{actor::ApiActor, auth::AuthUser, oauth::is_oauth_token, public::PublicUser},
    },
};
use dropshot::{Body, ClientErrorStatusCode, HttpError, RequestContext};
use http::{HeaderMap, HeaderValue, Response, StatusCode, header};
use rmcp::{
    model::{CallToolResult, ContentBlock, JsonObject, MetaObject, RequestMetaObject},
    transport::common::http_header::JSON_MIME_TYPE,
};
use serde_json::json;
use strum::IntoEnumIterator as _;

use crate::{
    endpoint::insert_cors_headers,
    tools::{McpTool, ToolAccess},
};

const INVALID_TOKEN_CHALLENGE: &str =
    r#"Bearer error="invalid_token", error_description="The access token is invalid""#;
const INSUFFICIENT_SCOPE: &str = "insufficient_scope";
const INSUFFICIENT_SCOPE_DESCRIPTION: &str = "The access token does not permit this tool";
const LOGIN_REQUIRED: &str = "login_required";
/// `ChatGPT` reads a challenge from this key of a tool result's `_meta`, not from the HTTP response.
const MCP_WWW_AUTHENTICATE: &str = "mcp/www_authenticate";
/// `ChatGPT` sends request `_meta` keys under this prefix with its tool calls, such as `openai/subject`.
const OPENAI_META_PREFIX: &str = "openai/";
const OPENAI_USER_AGENT_PREFIX: &str = "openai-mcp";

/// Who calls the MCP server, as the handler sees it.
#[derive(Clone)]
pub enum Caller {
    Principal(McpPrincipal),
    /// An anonymous tool call over its address's request limit,
    /// which is asked to sign in for higher limits instead of refused.
    OverLimit,
}

impl Caller {
    /// Every message is charged to its credential, or once to its address when anonymous,
    /// whatever its method. `tool` is the known tool the message calls, if any.
    pub async fn new(
        rqctx: &RequestContext<ApiContext>,
        tool: Option<McpTool>,
    ) -> Result<Self, HttpError> {
        match McpPrincipal::new(rqctx).await {
            Ok(principal) => {
                if tool.is_none() {
                    charge_project_key(rqctx.context(), principal.actor()).await?;
                }
                Ok(Self::Principal(principal))
            },
            Err(error) => {
                // Only an anonymous caller is charged to its address
                if tool.is_some()
                    && rqctx.context().oauth.is_some()
                    && error.status_code == ClientErrorStatusCode::TOO_MANY_REQUESTS
                    && bearer_token(rqctx.request.headers()).is_none()
                {
                    Ok(Self::OverLimit)
                } else {
                    Err(error)
                }
            },
        }
    }
}

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
    /// The MCP sibling of `ApiActor::new`, which takes bearer credentials alone: once OAuth is
    /// configured, it also accepts an OAuth access token, and every credential it refuses
    /// is answered with a challenge to sign in again.
    pub async fn new(rqctx: &RequestContext<ApiContext>) -> Result<Self, HttpError> {
        let context = rqctx.context();
        let Some(oauth) = &context.oauth else {
            return ApiActor::new_bearer_only(rqctx).await.map(Self::Actor);
        };
        let oauth_token =
            bearer_token(rqctx.request.headers()).filter(|token| is_oauth_token(token));
        let principal = if let Some(token) = oauth_token {
            AuthUser::from_oauth_token(context, oauth, token)
                .await
                .map(|(auth_user, scopes)| Self::OAuth {
                    actor: auth_user.into(),
                    scopes,
                })
        } else {
            ApiActor::new_bearer_only(rqctx).await.map(Self::Actor)
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

    pub fn is_anonymous(&self) -> bool {
        matches!(self.actor(), ApiActor::Public(PublicUser::Public(_)))
    }
}

/// An authorization challenge: a request to sign in, or to sign in again with more scopes.
#[derive(Debug, Clone, Copy)]
pub enum Challenge {
    /// An anonymous caller must sign in to use a tool with this access.
    SignIn {
        access: ToolAccess,
        reason: SignInReason,
    },
    /// The access token's scopes do not permit the tool.
    InsufficientScope,
}

impl Challenge {
    /// The HTTP response a standard client acts on, answered in place of the SDK's.
    pub fn into_response(self, resource_metadata: &str) -> Result<Response<Body>, HttpError> {
        let body = json!({
            "error": self.error(),
            "error_description": self.description(),
        });
        let mut response = Response::builder()
            .status(self.status())
            .header(header::CONTENT_TYPE, JSON_MIME_TYPE)
            .header(
                header::WWW_AUTHENTICATE,
                self.www_authenticate(resource_metadata),
            )
            .body(body.to_string().into())?;
        insert_challenge_cors_headers(response.headers_mut());
        Ok(response)
    }

    /// The tool result `ChatGPT` acts on: an error whose `_meta` carries the challenge.
    pub fn into_result(self, resource_metadata: &str) -> CallToolResult {
        let challenge = format!(
            r#"Bearer resource_metadata="{resource_metadata}", error="{INSUFFICIENT_SCOPE}", error_description="{description}", scope="{scope}""#,
            description = self.description(),
            scope = self.scopes(),
        );
        let mut result = CallToolResult::error(vec![ContentBlock::text(self.description())]);
        result.meta = Some(MetaObject(JsonObject::from_iter([(
            MCP_WWW_AUTHENTICATE.to_owned(),
            json!([challenge]),
        )])));
        result
    }

    /// The tool result that stands in for the HTTP response the endpoint answers instead.
    pub fn into_status_result(self) -> CallToolResult {
        CallToolResult::error(vec![ContentBlock::text(format!(
            "{status}: {description}",
            status = self.status().as_u16(),
            description = self.description(),
        ))])
    }

    #[cfg(feature = "otel")]
    pub const fn kind(self) -> bencher_otel::McpChallengeKind {
        match self {
            Self::SignIn {
                reason: SignInReason::Required,
                ..
            } => bencher_otel::McpChallengeKind::SignIn,
            Self::SignIn {
                reason: SignInReason::OverLimit,
                ..
            } => bencher_otel::McpChallengeKind::OverLimit,
            Self::InsufficientScope => bencher_otel::McpChallengeKind::InsufficientScope,
        }
    }

    const fn status(self) -> StatusCode {
        match self {
            Self::SignIn { .. } => StatusCode::UNAUTHORIZED,
            Self::InsufficientScope => StatusCode::FORBIDDEN,
        }
    }

    const fn error(self) -> &'static str {
        match self {
            Self::SignIn { .. } => LOGIN_REQUIRED,
            Self::InsufficientScope => INSUFFICIENT_SCOPE,
        }
    }

    const fn description(self) -> &'static str {
        match self {
            Self::SignIn {
                reason: SignInReason::OverLimit,
                ..
            } => "Sign in to Bencher for higher request limits",
            Self::SignIn {
                access: ToolAccess::Read,
                reason: SignInReason::Required,
            } => "Sign in to Bencher to read private projects",
            Self::SignIn {
                access: ToolAccess::Write,
                reason: SignInReason::Required,
            } => "Sign in to Bencher to change projects",
            Self::InsufficientScope => INSUFFICIENT_SCOPE_DESCRIPTION,
        }
    }

    fn scopes(self) -> OAuthScopes {
        match self {
            Self::SignIn { access, .. } => access.sign_in_scopes(),
            // Every scope a tool can need, so one consent covers every tool
            Self::InsufficientScope => ToolAccess::iter().map(ToolAccess::scope).collect(),
        }
    }

    /// Asking an anonymous caller to sign in carries no error code (RFC 6750).
    fn www_authenticate(self, resource_metadata: &str) -> String {
        let scope = self.scopes();
        match self {
            Self::SignIn { .. } => {
                format!(r#"Bearer resource_metadata="{resource_metadata}", scope="{scope}""#)
            },
            Self::InsufficientScope => format!(
                r#"Bearer error="{INSUFFICIENT_SCOPE}", scope="{scope}", resource_metadata="{resource_metadata}""#
            ),
        }
    }
}

/// Why an anonymous caller is asked to sign in.
#[derive(Debug, Clone, Copy)]
pub enum SignInReason {
    /// The tool refused an anonymous caller, or found nothing an anonymous caller may see.
    Required,
    /// The caller's address is over its anonymous request limit.
    OverLimit,
}

/// How a client acts on a challenge.
#[derive(Debug, Clone, Copy)]
pub enum ClientStyle {
    /// `ChatGPT` reads the challenge from the tool result.
    ChatGpt,
    /// Every other client reads it from the HTTP response.
    Standard,
}

impl ClientStyle {
    pub fn of(meta: &RequestMetaObject, headers: &HeaderMap) -> Self {
        let openai_meta = meta.keys().any(|key| key.starts_with(OPENAI_META_PREFIX));
        let openai_agent = headers
            .get(header::USER_AGENT)
            .and_then(|agent| agent.to_str().ok())
            .is_some_and(|agent| agent.starts_with(OPENAI_USER_AGENT_PREFIX));
        if openai_meta || openai_agent {
            Self::ChatGpt
        } else {
            Self::Standard
        }
    }
}

#[cfg(feature = "otel")]
impl From<ClientStyle> for bencher_otel::McpClientStyle {
    fn from(style: ClientStyle) -> Self {
        match style {
            ClientStyle::ChatGpt => Self::ChatGpt,
            ClientStyle::Standard => Self::Standard,
        }
    }
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(bencher_json::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(bencher_json::strip_bearer_token)
}

/// A tool charges a project key's project when it reaches the project,
/// so every other message is charged here.
async fn charge_project_key(context: &ApiContext, actor: &ApiActor) -> Result<(), HttpError> {
    if let ApiActor::ProjectKey(project_key) = actor {
        let project_uuid = QueryProject::get_uuid(auth_conn!(context), project_key.project_id)?;
        context.rate_limiting.project_request(project_uuid)?;
    }
    Ok(())
}

/// Point a refused client at the metadata of the MCP server it reached, where it finds how to get
/// a new access token.
fn with_invalid_token_challenge(
    mut error: HttpError,
    oauth: &OAuthServer,
    rqctx: &RequestContext<ApiContext>,
) -> HttpError {
    if error.status_code == ClientErrorStatusCode::UNAUTHORIZED {
        let resource_metadata = resource_metadata(oauth, rqctx);
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

/// The metadata URL of the MCP server a request reached, where a client finds how to sign in.
pub fn resource_metadata(oauth: &OAuthServer, rqctx: &RequestContext<ApiContext>) -> String {
    oauth
        .resource_at(rqctx.request.headers(), rqctx.request.uri())
        .metadata_url()
}

/// A browser client can only read `WWW-Authenticate` once it is exposed.
fn insert_challenge_cors_headers(headers: &mut HeaderMap) {
    insert_cors_headers(headers);
    headers.insert(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static("WWW-Authenticate"),
    );
}
