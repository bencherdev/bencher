use bencher_endpoint::{CorsResponse, Endpoint, Get};
use bencher_json::{OAuthIssuer, OAuthResource, OAuthScope};
use bencher_schema::context::{ApiContext, OAuthServer};
use dropshot::{Body, HttpError, RequestContext, endpoint};
use http::Response;
use serde::Serialize;

use crate::{
    AUTHORIZATION_CODE, AUTHORIZATION_CODE_RESPONSE, REFRESH_TOKEN, S256, oauth_server,
    openid_provider, response::metadata,
};

const AUTHORIZE_PATH: &str = "/oauth/authorize";
const TOKEN_PATH: &str = "/oauth/token";
const REVOKE_PATH: &str = "/oauth/revoke";
const USERINFO_PATH: &str = "/oauth/userinfo";
const JWKS_PATH: &str = "/oauth/jwks.json";
const DOCUMENTATION: &str = "https://bencher.dev/docs/how-to/mcp/";
/// Every scope that reaches the MCP server.
const SCOPES: [OAuthScope; 2] = [OAuthScope::ProjectsRead, OAuthScope::ProjectsWrite];
/// The scopes that ask who the user is, granted only when ID tokens are signed.
const IDENTITY_SCOPES: [OAuthScope; 3] =
    [OAuthScope::Openid, OAuthScope::Email, OAuthScope::Profile];
/// Every claim an ID token or the user info endpoint may hold.
const CLAIMS: [&str; 11] = [
    "sub",
    "iss",
    "aud",
    "exp",
    "iat",
    "auth_time",
    "nonce",
    "email",
    "email_verified",
    "name",
    "preferred_username",
];
/// Accepted and ignored: whether a client gets refresh tokens follows its registered grant types.
const OFFLINE_ACCESS: &str = "offline_access";
/// Public clients, which identify themselves and never authenticate.
const NO_CLIENT_AUTHENTICATION: &str = "none";

#[endpoint {
    method = OPTIONS,
    path = "/.well-known/oauth-protected-resource",
    unpublished = true,
}]
pub async fn protected_resource_options(
    rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    oauth_server(rqctx.context())?;
    Ok(Endpoint::cors(&[Get.into()]))
}

/// OAuth protected resource metadata (RFC 9728)
///
/// The metadata of the MCP server reached by the request's `Host`, or of the canonical one.
#[endpoint {
    method = GET,
    path = "/.well-known/oauth-protected-resource",
    unpublished = true,
}]
pub async fn protected_resource_get(
    rqctx: RequestContext<ApiContext>,
) -> Result<Response<Body>, HttpError> {
    protected_resource(&rqctx)
}

#[endpoint {
    method = OPTIONS,
    path = "/.well-known/oauth-protected-resource/mcp",
    unpublished = true,
}]
pub async fn mcp_protected_resource_options(
    rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    oauth_server(rqctx.context())?;
    Ok(Endpoint::cors(&[Get.into()]))
}

/// OAuth protected resource metadata for the MCP server (RFC 9728)
///
/// Where an MCP client first looks, at the path of the MCP server's URL.
#[endpoint {
    method = GET,
    path = "/.well-known/oauth-protected-resource/mcp",
    unpublished = true,
}]
pub async fn mcp_protected_resource_get(
    rqctx: RequestContext<ApiContext>,
) -> Result<Response<Body>, HttpError> {
    protected_resource(&rqctx)
}

#[endpoint {
    method = OPTIONS,
    path = "/.well-known/oauth-authorization-server",
    unpublished = true,
}]
pub async fn authorization_server_options(
    rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    oauth_server(rqctx.context())?;
    Ok(Endpoint::cors(&[Get.into()]))
}

/// OAuth authorization server metadata (RFC 8414)
#[endpoint {
    method = GET,
    path = "/.well-known/oauth-authorization-server",
    unpublished = true,
}]
pub async fn authorization_server_get(
    rqctx: RequestContext<ApiContext>,
) -> Result<Response<Body>, HttpError> {
    metadata(&AuthorizationServer::new(oauth_server(rqctx.context())?))
}

#[endpoint {
    method = OPTIONS,
    path = "/.well-known/openid-configuration",
    unpublished = true,
}]
pub async fn openid_configuration_options(
    rqctx: RequestContext<ApiContext>,
) -> Result<CorsResponse, HttpError> {
    openid_provider(rqctx.context())?;
    Ok(Endpoint::cors(&[Get.into()]))
}

/// OIDC discovery
///
/// The authorization server metadata, served only while ID tokens are signed.
#[endpoint {
    method = GET,
    path = "/.well-known/openid-configuration",
    unpublished = true,
}]
pub async fn openid_configuration_get(
    rqctx: RequestContext<ApiContext>,
) -> Result<Response<Body>, HttpError> {
    let (oauth, _signing_key) = openid_provider(rqctx.context())?;
    metadata(&AuthorizationServer::new(oauth))
}

#[endpoint {
    method = OPTIONS,
    path = "/oauth/jwks.json",
    unpublished = true,
}]
pub async fn jwks_options(rqctx: RequestContext<ApiContext>) -> Result<CorsResponse, HttpError> {
    openid_provider(rqctx.context())?;
    Ok(Endpoint::cors(&[Get.into()]))
}

/// The key set ID tokens are signed with (RFC 7517)
#[endpoint {
    method = GET,
    path = "/oauth/jwks.json",
    unpublished = true,
}]
pub async fn jwks_get(rqctx: RequestContext<ApiContext>) -> Result<Response<Body>, HttpError> {
    let (_oauth, signing_key) = openid_provider(rqctx.context())?;
    metadata(&signing_key.jwk_set())
}

fn protected_resource(rqctx: &RequestContext<ApiContext>) -> Result<Response<Body>, HttpError> {
    let oauth = oauth_server(rqctx.context())?;
    metadata(&ProtectedResource {
        resource: oauth.resource_at(rqctx.request.headers(), rqctx.request.uri()),
        authorization_servers: [oauth.issuer()],
        scopes_supported: SCOPES,
        bearer_methods_supported: ["header"],
        resource_name: "Bencher",
        resource_documentation: DOCUMENTATION,
    })
}

#[derive(Serialize)]
struct ProtectedResource<'a> {
    resource: &'a OAuthResource,
    authorization_servers: [&'a OAuthIssuer; 1],
    scopes_supported: [OAuthScope; 2],
    bearer_methods_supported: [&'static str; 1],
    resource_name: &'static str,
    resource_documentation: &'static str,
}

#[derive(Serialize)]
struct AuthorizationServer<'a> {
    issuer: &'a OAuthIssuer,
    authorization_endpoint: String,
    token_endpoint: String,
    revocation_endpoint: String,
    response_types_supported: [&'static str; 1],
    response_modes_supported: [&'static str; 1],
    grant_types_supported: [&'static str; 2],
    code_challenge_methods_supported: [&'static str; 1],
    token_endpoint_auth_methods_supported: [&'static str; 1],
    revocation_endpoint_auth_methods_supported: [&'static str; 1],
    client_id_metadata_document_supported: bool,
    authorization_response_iss_parameter_supported: bool,
    scopes_supported: Vec<String>,
    service_documentation: &'static str,
    #[serde(flatten, skip_serializing_if = "Option::is_none")]
    openid: Option<OpenIdProvider>,
}

impl<'a> AuthorizationServer<'a> {
    fn new(oauth: &'a OAuthServer) -> Self {
        let issuer = oauth.issuer();
        let openid = oauth.signing_key().map(|_signing_key| OpenIdProvider {
            userinfo_endpoint: format!("{issuer}{USERINFO_PATH}"),
            jwks_uri: format!("{issuer}{JWKS_PATH}"),
            subject_types_supported: ["public"],
            id_token_signing_alg_values_supported: ["RS256"],
            claims_supported: CLAIMS,
        });
        let identity_scopes = if openid.is_some() {
            IDENTITY_SCOPES.as_slice()
        } else {
            &[]
        };
        Self {
            issuer,
            authorization_endpoint: format!("{issuer}{AUTHORIZE_PATH}"),
            token_endpoint: format!("{issuer}{TOKEN_PATH}"),
            revocation_endpoint: format!("{issuer}{REVOKE_PATH}"),
            response_types_supported: [AUTHORIZATION_CODE_RESPONSE],
            response_modes_supported: ["query"],
            grant_types_supported: [AUTHORIZATION_CODE, REFRESH_TOKEN],
            code_challenge_methods_supported: [S256],
            token_endpoint_auth_methods_supported: [NO_CLIENT_AUTHENTICATION],
            revocation_endpoint_auth_methods_supported: [NO_CLIENT_AUTHENTICATION],
            client_id_metadata_document_supported: true,
            authorization_response_iss_parameter_supported: true,
            scopes_supported: SCOPES
                .iter()
                .chain(identity_scopes)
                .map(ToString::to_string)
                .chain([OFFLINE_ACCESS.to_owned()])
                .collect(),
            service_documentation: DOCUMENTATION,
            openid,
        }
    }
}

/// What an OIDC provider adds to the metadata (OIDC Discovery, section 3).
#[derive(Serialize)]
struct OpenIdProvider {
    userinfo_endpoint: String,
    jwks_uri: String,
    subject_types_supported: [&'static str; 1],
    id_token_signing_alg_values_supported: [&'static str; 1],
    claims_supported: [&'static str; CLAIMS.len()],
}
