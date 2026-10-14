#![cfg(feature = "plus")]
// Dev-dependencies are used in tests but not in the lib crate
#![cfg_attr(
    test,
    expect(
        unused_crate_dependencies,
        reason = "dev-dependencies used by integration tests"
    )
)]

//! Bencher MCP (Model Context Protocol) Server - A Bencher Plus Feature
//!
//! Serves a stateless MCP server over the Streamable HTTP transport,
//! with the protocol handled by the official MCP Rust SDK (`rmcp`):
//! - POST `/mcp` - A single JSON-RPC 2.0 message per request
//!
//! Protocol `2026-07-28` clients send self-contained requests
//! (`server/discover`, per-request `_meta`); clients of `2025-03-26` through
//! `2025-11-25` still open with `initialize`, which is answered statelessly.
//! Every request is independent: responses are plain `application/json`,
//! with no SSE stream and no `Mcp-Session-Id`.
//! Server-initiated messages (sampling, elicitation) are not supported.
//! On Bencher Cloud the endpoint is fronted by `mcp.bencher.dev`,
//! a DNS alias for the API server, so the connector URL is
//! `https://mcp.bencher.dev/mcp`.
//!
//! The tool surface mirrors the operations available to a project API key
//! (`bencher_run_`): project-scoped reads and non-destructive writes.
//! Authentication accepts the same `Authorization: Bearer` credentials as
//! the REST API (project key, user key, or token) by reusing the REST
//! endpoint handlers; anonymous requests may read public projects.
//! Once `plus.oauth` is configured, an OAuth access token is accepted too,
//! acting as its user within the token's scopes, and every refused credential
//! is challenged with where to find how to get a new one.
//!
//! These endpoints are `unpublished`: they speak JSON-RPC, not REST,
//! so they are intentionally excluded from the `OpenAPI` spec.
//!
//! Also serves `GET /.well-known/openai-apps-challenge` (`unpublished`).

mod challenge;
mod endpoint;
mod principal;
mod server;
mod tools;

use bencher_endpoint::Registrar;
use bencher_schema::context::ApiContext;
use dropshot::{ApiDescription, ApiDescriptionRegisterError};

pub struct Api;

impl Registrar for Api {
    fn register(
        api_description: &mut ApiDescription<ApiContext>,
        _http_options: bool,
        #[cfg(feature = "plus")] _is_bencher_cloud: bool,
    ) -> Result<(), ApiDescriptionRegisterError> {
        // Like `api_oci`, `OPTIONS` is registered unconditionally: browser
        // based MCP clients need the CORS preflight, and the endpoints are
        // `unpublished` so the OpenAPI spec is unaffected either way
        api_description.register(endpoint::mcp_options)?;
        api_description.register(endpoint::mcp_post)?;
        api_description.register(challenge::openai_apps_challenge_options)?;
        api_description.register(challenge::openai_apps_challenge_get)?;
        Ok(())
    }
}
