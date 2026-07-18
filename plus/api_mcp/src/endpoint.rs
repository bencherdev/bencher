use bencher_endpoint::{CorsResponse, Endpoint, Post};
use bencher_schema::{context::ApiContext, error::issue_error};
use dropshot::{Body, HttpError, RequestContext, UntypedBody, endpoint};
use http::Response;
use rmcp::model::ServerJsonRpcMessage;

use crate::server;

const APPLICATION_JSON: &str = "application/json";

#[endpoint {
    method = OPTIONS,
    path = "/mcp",
    unpublished = true,
}]
pub async fn mcp_options(_rqctx: RequestContext<ApiContext>) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Post.into()]))
}

/// MCP Streamable HTTP endpoint (stateless)
///
/// Accepts a single JSON-RPC 2.0 message per request and responds with
/// `application/json`. Notifications are accepted and discarded with
/// `202 Accepted`. SSE streams and sessions are not supported.
#[endpoint {
    method = POST,
    path = "/mcp",
    unpublished = true,
}]
pub async fn mcp_post(
    rqctx: RequestContext<ApiContext>,
    body: UntypedBody,
) -> Result<Response<Body>, HttpError> {
    match server::handle(&rqctx, body.as_bytes()).await? {
        Some(message) => json_response(&message),
        None => accepted_response(),
    }
}

fn json_response(message: &ServerJsonRpcMessage) -> Result<Response<Body>, HttpError> {
    let body = serde_json::to_vec(message).map_err(|e| {
        issue_error(
            "Failed to serialize MCP response",
            "MCP JSON-RPC response",
            e,
        )
    })?;
    mcp_cors_headers(
        Response::builder()
            .status(http::StatusCode::OK)
            .header(http::header::CONTENT_TYPE, APPLICATION_JSON)
            .header(http::header::CONTENT_LENGTH, body.len()),
    )
    .body(Body::from(body))
    .map_err(|e| issue_error("Failed to build MCP response", "MCP JSON-RPC response", e))
}

fn accepted_response() -> Result<Response<Body>, HttpError> {
    mcp_cors_headers(Response::builder().status(http::StatusCode::ACCEPTED))
        .body(Body::from(Vec::new()))
        .map_err(|e| issue_error("Failed to build MCP response", "MCP accepted response", e))
}

/// The `Origin` header is deliberately not validated: the server binds to a
/// public address (not localhost), authentication is bearer-only with no
/// cookie or other ambient credentials, and `Access-Control-Allow-Origin: *`
/// is by design, so DNS rebinding grants an attacker nothing beyond what
/// any cross-origin client already has.
fn mcp_cors_headers(builder: http::response::Builder) -> http::response::Builder {
    builder
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(http::header::ACCESS_CONTROL_ALLOW_METHODS, "POST, OPTIONS")
        .header(
            http::header::ACCESS_CONTROL_ALLOW_HEADERS,
            "Content-Type, Authorization, Mcp-Protocol-Version",
        )
}
