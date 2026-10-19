use bencher_endpoint::{CorsResponse, Endpoint, Get};
use bencher_json::{OAuthIssuer, OAuthResource, OAuthScope};
use bencher_schema::context::ApiContext;
use dropshot::{Body, HttpError, RequestContext, endpoint};
use http::Response;
use serde::Serialize;

use crate::{
    AUTHORIZATION_CODE, AUTHORIZATION_CODE_RESPONSE, REFRESH_TOKEN, S256, oauth_server,
    response::metadata,
};

const AUTHORIZE_PATH: &str = "/oauth/authorize";
const TOKEN_PATH: &str = "/oauth/token";
const REVOKE_PATH: &str = "/oauth/revoke";
const DOCUMENTATION: &str = "https://bencher.dev/docs/how-to/mcp/";
/// Every scope a client can be granted.
const SCOPES: [OAuthScope; 2] = [OAuthScope::ProjectsRead, OAuthScope::ProjectsWrite];
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
    let oauth = oauth_server(rqctx.context())?;
    let issuer = oauth.issuer();
    metadata(&AuthorizationServer {
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
            .map(ToString::to_string)
            .chain([OFFLINE_ACCESS.to_owned()])
            .collect(),
        service_documentation: DOCUMENTATION,
    })
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
}
