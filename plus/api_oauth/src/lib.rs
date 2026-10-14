#![cfg(feature = "plus")]
// Dev-dependencies are used in tests but not in the lib crate
#![cfg_attr(
    test,
    expect(
        unused_crate_dependencies,
        reason = "dev-dependencies used by integration tests"
    )
)]

//! Bencher as an OAuth 2.1 authorization server for its MCP server - A Bencher Plus Feature
//!
//! - GET `/oauth/authorize` - An app's request to connect, sent on to the console for consent
//! - GET and POST `/v0/oauth/authorizations/{authorization}` - The consent page's view of a
//!   request, and the user's answer to it
//! - POST `/oauth/token` - An authorization code or refresh token exchanged for tokens
//! - POST `/oauth/revoke` - A refresh token's grant, or an access token, revoked
//! - GET `/.well-known/oauth-protected-resource` (also at `/mcp`) - The MCP server's metadata
//! - GET `/.well-known/oauth-authorization-server` - The authorization server's metadata
//!
//! Every route answers `404` until `plus.oauth` is configured.
//! These endpoints are `unpublished`: they belong to the OAuth protocol and the console's
//! consent page, not the REST API.

mod authorization;
mod authorize;
mod client;
mod discovery;
mod error;
mod params;
mod response;
mod revoke;
mod token;

use bencher_endpoint::Registrar;
use bencher_schema::{
    context::{ApiContext, OAuthServer},
    error::not_found_error,
};
use dropshot::{ApiDescription, ApiDescriptionRegisterError, HttpError};

/// The authorization code flow's response type, the only one supported.
const AUTHORIZATION_CODE_RESPONSE: &str = "code";
/// The only PKCE code challenge method supported.
const S256: &str = "S256";
const AUTHORIZATION_CODE: &str = "authorization_code";
const REFRESH_TOKEN: &str = "refresh_token";

pub struct Api;

impl Registrar for Api {
    fn register(
        api_description: &mut ApiDescription<ApiContext>,
        _http_options: bool,
        #[cfg(feature = "plus")] _is_bencher_cloud: bool,
    ) -> Result<(), ApiDescriptionRegisterError> {
        // Like `api_mcp`, `OPTIONS` is registered unconditionally: the endpoints are
        // `unpublished`, so the OpenAPI spec is unaffected either way
        api_description.register(authorize::oauth_authorize_options)?;
        api_description.register(authorize::oauth_authorize_get)?;
        api_description.register(authorization::oauth_authorization_options)?;
        api_description.register(authorization::oauth_authorization_get)?;
        api_description.register(authorization::oauth_authorization_post)?;
        api_description.register(token::oauth_token_options)?;
        api_description.register(token::oauth_token_post)?;
        api_description.register(revoke::oauth_revoke_options)?;
        api_description.register(revoke::oauth_revoke_post)?;
        api_description.register(discovery::protected_resource_options)?;
        api_description.register(discovery::protected_resource_get)?;
        api_description.register(discovery::mcp_protected_resource_options)?;
        api_description.register(discovery::mcp_protected_resource_get)?;
        api_description.register(discovery::authorization_server_options)?;
        api_description.register(discovery::authorization_server_get)?;
        Ok(())
    }
}

fn oauth_server(context: &ApiContext) -> Result<&OAuthServer, HttpError> {
    context
        .oauth
        .as_ref()
        .ok_or_else(|| not_found_error("OAuth is not configured on this server"))
}
