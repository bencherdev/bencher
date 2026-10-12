use std::sync::Arc;

use bencher_schema::{
    context::{ApiContext, RateLimiting},
    model::user::{actor::ApiActor, public::PublicUser},
};
use bytes::Bytes;
use dropshot::{Body, HttpError, RequestContext, UntypedBody, endpoint};
use http::{HeaderMap, HeaderValue, Request, Response, StatusCode, header};
use http_body_util::Full;
use rmcp::{
    model::{ClientJsonRpcMessage, ClientRequest},
    transport::{
        StreamableHttpServerConfig, StreamableHttpService, common::http_header::JSON_MIME_TYPE,
        streamable_http_server::session::never::NeverSessionManager,
    },
};

use crate::{principal::McpPrincipal, server::BencherMcp, tools::McpTool};

const MCP_ACCEPT: &str = "application/json, text/event-stream";
const MCP_ALLOW_METHODS: &str = "POST, OPTIONS";
const MCP_ALLOW_HEADERS: &str =
    "Content-Type, Authorization, Mcp-Protocol-Version, Mcp-Method, Mcp-Name";

#[endpoint {
    method = OPTIONS,
    path = "/mcp",
    unpublished = true,
}]
pub async fn mcp_options(_rqctx: RequestContext<ApiContext>) -> Result<Response<Body>, HttpError> {
    let mut response = Response::new(Body::empty());
    *response.status_mut() = StatusCode::NO_CONTENT;
    insert_cors_headers(response.headers_mut());
    Ok(response)
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
    let principal = if is_tool_call(body.as_bytes()) {
        // Invalid or expired credentials fail the whole request with a `401`,
        // just like the REST API; anonymous requests may still read public projects.
        // Rate limiting is applied per-actor inside the shared endpoint handlers,
        // with the same exceptions as REST (`list_projects` mirrors `GET /v0/projects`).
        let principal = McpPrincipal::new(&rqctx).await?;
        // Anonymous tool calls are also throttled before dispatch: malformed
        // arguments are rejected before reaching the shared handlers where
        // per-actor rate limiting lives (REST body and query parse failures
        // are likewise rejected before rate limiting)
        if matches!(principal.actor(), ApiActor::Public(PublicUser::Public(_))) {
            apply_public_rate_limit(&rqctx)?;
        }
        Some(principal)
    } else {
        apply_public_rate_limit(&rqctx)?;
        None
    };

    let request = mcp_request(&rqctx, &body);
    let config = server_config(rqctx.request_body_max_bytes());
    let server = Arc::new(BencherMcp::new(rqctx, principal));
    let handler = Arc::clone(&server);
    let service: StreamableHttpService<_, NeverSessionManager> =
        StreamableHttpService::new(move || Ok(Arc::clone(&handler)), Arc::default(), config);
    let (mut parts, response_body) = service.handle(request).await.into_parts();
    if let Some(challenge) = server.challenge() {
        return challenge.into_response();
    }
    insert_cors_headers(&mut parts.headers);
    Ok(Response::from_parts(parts, Body::wrap(response_body)))
}

/// Only a request to call a known tool needs credentials;
/// every other message takes the public rate limit.
/// The body is read with the SDK's own types so this agrees with what it dispatches.
fn is_tool_call(body: &[u8]) -> bool {
    if let Ok(ClientJsonRpcMessage::Request(request)) = serde_json::from_slice(body)
        && let ClientRequest::CallToolRequest(call) = request.request
    {
        McpTool::from_name(&call.params.name).is_some()
    } else {
        false
    }
}

fn apply_public_rate_limit(rqctx: &RequestContext<ApiContext>) -> Result<(), HttpError> {
    let context = rqctx.context();
    if let Some(remote_ip) = RateLimiting::remote_ip(&rqctx.log, rqctx.request.headers()) {
        context.rate_limiting.public_request(remote_ip)?;
    }
    Ok(())
}

fn mcp_request(rqctx: &RequestContext<ApiContext>, body: &UntypedBody) -> Request<Full<Bytes>> {
    let mut request = Request::new(Full::new(Bytes::copy_from_slice(body.as_bytes())));
    request.method_mut().clone_from(rqctx.request.method());
    request.uri_mut().clone_from(rqctx.request.uri());
    let headers = request.headers_mut();
    headers.clone_from(rqctx.request.headers());
    // Every body is read as JSON and every response is JSON,
    // so a client's content negotiation headers never fail a request
    headers.insert(header::ACCEPT, HeaderValue::from_static(MCP_ACCEPT));
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static(JSON_MIME_TYPE),
    );
    request
}

fn server_config(max_request_body_bytes: usize) -> StreamableHttpServerConfig {
    StreamableHttpServerConfig::default()
        .with_legacy_session_mode(false)
        .with_json_response(true)
        .with_max_request_body_bytes(max_request_body_bytes)
        // The `Host` and `Origin` headers are deliberately not validated: the server binds
        // to a public address (not localhost), authentication is bearer-only with no
        // cookie or other ambient credentials, and `Access-Control-Allow-Origin: *`
        // is by design, so DNS rebinding grants an attacker nothing beyond what
        // any cross-origin client already has.
        .disable_allowed_hosts()
        .disable_allowed_origins()
}

pub fn insert_cors_headers(headers: &mut HeaderMap) {
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static(MCP_ALLOW_METHODS),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static(MCP_ALLOW_HEADERS),
    );
}
