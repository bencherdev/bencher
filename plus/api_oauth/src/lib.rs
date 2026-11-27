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
//! - GET `/.well-known/openid-configuration` - The same, for OIDC
//! - GET and POST `/oauth/userinfo` - Who the user of an access token is
//! - GET `/oauth/jwks.json` - The key that signs ID tokens
//!
//! The Bencher CLI is a client built into the server, with the client ID `bencher_cli`, so the
//! authorization server's metadata and its authorize, consent, token, and revoke routes answer it
//! on every server. Where `plus.oauth` is unset, they answer it alone, as an issuer at the URL the
//! request reached. Every outside app, and the MCP server's metadata, user info, and keys, answer
//! `404` until `plus.oauth` is configured, and the OIDC routes until `plus.oauth.signing_key` is
//! too.
//! These endpoints are `unpublished`: they belong to the OAuth protocol and the console's
//! consent page, not the REST API.

mod authorization;
mod authorize;
mod client;
mod discovery;
mod error;
mod id_token;
mod params;
mod response;
mod revoke;
mod token;
mod userinfo;

use bencher_endpoint::Registrar;
use bencher_json::{NonEmpty, OAuthIssuer, OAuthScope, OAuthScopes};
use bencher_schema::{
    context::{ApiContext, OAuthServer},
    error::not_found_error,
};
use bencher_token::OAuthSigningKey;
use dropshot::{ApiDescription, ApiDescriptionRegisterError, HttpError, RequestContext};
use http::{HeaderMap, header::HOST};

/// The authorization code flow's response type, the only one supported.
const AUTHORIZATION_CODE_RESPONSE: &str = "code";
/// The only PKCE code challenge method supported.
const S256: &str = "S256";
const AUTHORIZATION_CODE: &str = "authorization_code";
const REFRESH_TOKEN: &str = "refresh_token";
/// The name the consent page and Connected Apps give the Bencher CLI.
const BENCHER_CLI: &str = "Bencher CLI";
/// What a reverse proxy that ends TLS says the request came in over.
const FORWARDED_PROTO: &str = "X-Forwarded-Proto";

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
        api_description.register(discovery::openid_configuration_options)?;
        api_description.register(discovery::openid_configuration_get)?;
        api_description.register(discovery::jwks_options)?;
        api_description.register(discovery::jwks_get)?;
        api_description.register(userinfo::oauth_userinfo_options)?;
        api_description.register(userinfo::oauth_userinfo_get)?;
        api_description.register(userinfo::oauth_userinfo_post)?;
        Ok(())
    }
}

/// The authorization server a request reaches: the configured one, which serves outside apps and
/// the Bencher CLI, or else the Bencher CLI's alone.
pub(crate) enum Authority<'c> {
    Configured(&'c OAuthServer),
    /// Without `plus.oauth`: the Bencher CLI only, issued as the URL the request reached.
    Cli(OAuthIssuer),
}

impl<'c> Authority<'c> {
    /// The configured server, or else the Bencher CLI's at the request's own URL, which must be an
    /// issuer: `https`, or plain `http` on any host.
    fn new(rqctx: &'c RequestContext<ApiContext>) -> Result<Self, HttpError> {
        if let Some(oauth) = &rqctx.context().oauth {
            return Ok(Self::Configured(oauth));
        }
        let using_tls = rqctx.server.using_tls();
        request_issuer(rqctx.request.headers(), rqctx.request.uri(), using_tls)
            .map(Self::Cli)
            .ok_or_else(|| not_found_error("No host to issue OAuth tokens as"))
    }

    fn issuer(&self) -> &OAuthIssuer {
        match self {
            Self::Configured(oauth) => oauth.issuer(),
            Self::Cli(issuer) => issuer,
        }
    }

    fn configured(&self) -> Option<&'c OAuthServer> {
        match self {
            Self::Configured(oauth) => Some(oauth),
            Self::Cli(_) => None,
        }
    }
}

/// The issuer a request without a configured one reached: its `Host`, or without one the authority
/// of its URI (HTTP/2), over `https` when the server ended TLS or the proxy the client reached says
/// it did, and else over plain `http` on any host.
fn request_issuer(headers: &HeaderMap, uri: &http::Uri, using_tls: bool) -> Option<OAuthIssuer> {
    let host = headers
        .get(HOST)
        .and_then(|host| host.to_str().ok())
        .or_else(|| uri.authority().map(http::uri::Authority::as_str))?;
    // A chain of proxies lists each hop's scheme, the one the client reached first.
    let forwarded_https = headers
        .get(FORWARDED_PROTO)
        .and_then(|proto| proto.to_str().ok())
        .and_then(|proto| proto.split(',').next())
        .is_some_and(|proto| proto.trim().eq_ignore_ascii_case("https"));
    let scheme = if using_tls || forwarded_https {
        "https"
    } else {
        "http"
    };
    OAuthIssuer::of_request(&format!("{scheme}://{host}"))
}

/// The Bencher CLI's name, which it never chooses.
#[expect(clippy::expect_used, reason = "a fixed name that is not empty")]
fn bencher_cli_name() -> NonEmpty {
    BENCHER_CLI.parse().expect("Invalid name")
}

/// The Bencher CLI's scopes asked for: always acting as the user, and with server admin when it
/// asks for `admin`. Every other scope is ignored.
fn cli_scopes(scope: Option<&str>) -> OAuthScopes {
    scope
        .unwrap_or_default()
        .split(' ')
        .filter_map(|scope| scope.parse::<OAuthScope>().ok())
        .filter(|scope| scope.is_cli())
        .chain([OAuthScope::User])
        .collect()
}

fn oauth_server(context: &ApiContext) -> Result<&OAuthServer, HttpError> {
    context
        .oauth
        .as_ref()
        .ok_or_else(|| not_found_error("OAuth is not configured on this server"))
}

/// The OAuth server and the key it signs ID tokens with, when OIDC is configured.
fn openid_provider(context: &ApiContext) -> Result<(&OAuthServer, &OAuthSigningKey), HttpError> {
    let oauth = oauth_server(context)?;
    let signing_key = oauth
        .signing_key()
        .ok_or_else(|| not_found_error("OpenID Connect is not configured on this server"))?;
    Ok((oauth, signing_key))
}

/// The scopes in `scope` that this server grants an outside app: the identity scopes only when it
/// signs ID tokens. Unknown scopes, the Bencher CLI's and `offline_access` among them, are ignored.
fn supported_scopes(oauth: &OAuthServer, scope: Option<&str>) -> OAuthScopes {
    scope
        .unwrap_or_default()
        .split(' ')
        .filter_map(|scope| scope.parse::<OAuthScope>().ok())
        .filter(|scope| !scope.is_cli())
        .filter(|scope| scope.is_project() || oauth.signing_key().is_some())
        .collect()
}

#[cfg(test)]
mod tests {
    use http::{HeaderMap, HeaderValue, Uri, header::HOST};

    use super::request_issuer;

    // Kills an issuer that ignores where the request came in, that reads a later proxy's scheme,
    // or that refuses plain HTTP, which the Bencher CLI's client takes on any host.
    #[test]
    fn issuer_follows_the_request() {
        let issuer = |host: &str, proto: Option<&str>, tls: bool| {
            let mut headers = HeaderMap::new();
            headers.insert(HOST, HeaderValue::from_str(host).unwrap());
            if let Some(proto) = proto {
                headers.insert("X-Forwarded-Proto", HeaderValue::from_str(proto).unwrap());
            }
            request_issuer(&headers, &Uri::from_static("/oauth/authorize"), tls)
                .map(|issuer| issuer.to_string())
        };
        assert_eq!(
            issuer("127.0.0.1:6610", None, false).as_deref(),
            Some("http://127.0.0.1:6610")
        );
        assert_eq!(
            issuer("localhost:6610", None, false).as_deref(),
            Some("http://localhost:6610")
        );
        assert_eq!(
            issuer("api.example.com", Some("https"), false).as_deref(),
            Some("https://api.example.com")
        );
        assert_eq!(
            issuer("api.example.com", None, true).as_deref(),
            Some("https://api.example.com")
        );
        // The Bencher CLI signs in over plain HTTP too, on any host.
        assert_eq!(
            issuer("bencher.lan:6610", None, false).as_deref(),
            Some("http://bencher.lan:6610")
        );
        assert_eq!(
            issuer("api.example.com", Some("http"), false).as_deref(),
            Some("http://api.example.com")
        );
        // The scheme of the proxy the client reached comes first.
        assert_eq!(
            issuer("api.example.com", Some("https, http"), false).as_deref(),
            Some("https://api.example.com")
        );
        assert_eq!(
            issuer("api.example.com", Some("http, https"), false).as_deref(),
            Some("http://api.example.com")
        );
        assert_eq!(issuer("api.example.com/path", None, false), None);
        assert_eq!(
            request_issuer(
                &HeaderMap::new(),
                &Uri::from_static("https://api.example.com/oauth/authorize"),
                true,
            )
            .map(|issuer| issuer.to_string())
            .as_deref(),
            Some("https://api.example.com")
        );
    }
}
