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
//! Implements a stateless MCP server over the Streamable HTTP transport:
//! - POST `/mcp` - A single JSON-RPC 2.0 message per request
//!   (`initialize`, `ping`, `tools/list`, `tools/call`)
//!
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
//!
//! These endpoints are `unpublished`: they speak JSON-RPC, not REST,
//! so they are intentionally excluded from the `OpenAPI` spec.

mod endpoint;
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
        Ok(())
    }
}
