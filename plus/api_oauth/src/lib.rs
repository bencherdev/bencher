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
//!
//! Every route answers `404` until `plus.oauth` is configured.
//! These endpoints are `unpublished`: they belong to the OAuth protocol and the console's
//! consent page, not the REST API.

mod authorization;
mod authorize;
mod response;

use bencher_endpoint::Registrar;
use bencher_schema::{
    context::{ApiContext, OAuthServer},
    error::not_found_error,
};
use dropshot::{ApiDescription, ApiDescriptionRegisterError, HttpError};

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
        Ok(())
    }
}

fn oauth_server(context: &ApiContext) -> Result<&OAuthServer, HttpError> {
    context
        .oauth
        .as_ref()
        .ok_or_else(|| not_found_error("OAuth is not configured on this server"))
}
