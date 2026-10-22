use std::sync::Arc;

use bencher_schema::context::ApiContext;
use bytes::Bytes;
use dropshot::{Body, HttpError, RequestContext, UntypedBody, endpoint};
use http::{HeaderMap, HeaderValue, Request, Response, StatusCode, header};
use http_body_util::Full;
use rmcp::{
    model::{ClientJsonRpcMessage, ClientRequest},
    transport::{
        StreamableHttpServerConfig, StreamableHttpService,
        common::http_header::{HEADER_MCP_PROTOCOL_VERSION, JSON_MIME_TYPE},
        streamable_http_server::session::never::NeverSessionManager,
    },
};

use crate::{principal::Caller, server::BencherMcp, tools::McpTool};

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
    // Invalid or expired credentials fail the whole request with a `401` whatever its method,
    // just like the REST API; anonymous requests may still read public projects.
    let caller = Caller::new(&rqctx, called_tool(body.as_bytes())).await?;

    let request = mcp_request(&rqctx, &body);
    let config = server_config(rqctx.request_body_max_bytes());
    let server = Arc::new(BencherMcp::new(rqctx, caller));
    let handler = Arc::clone(&server);
    let service: StreamableHttpService<_, NeverSessionManager> =
        StreamableHttpService::new(move || Ok(Arc::clone(&handler)), Arc::default(), config);
    let (mut parts, response_body) = service.handle(request).await.into_parts();
    if let Some(challenge) = server.challenge_response() {
        return challenge;
    }
    if parts.status.is_client_error() {
        log_rejection(server.rqctx(), parts.status, body.as_bytes());
    }
    insert_cors_headers(&mut parts.headers);
    Ok(Response::from_parts(parts, Body::wrap(response_body)))
}

/// The known tool a message calls, read with the SDK's own types so this agrees with what it
/// dispatches.
fn called_tool(body: &[u8]) -> Option<McpTool> {
    if let Ok(ClientJsonRpcMessage::Request(request)) = serde_json::from_slice(body)
        && let ClientRequest::CallToolRequest(call) = request.request
    {
        McpTool::from_name(&call.params.name)
    } else {
        None
    }
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

fn log_rejection(rqctx: &RequestContext<ApiContext>, status: StatusCode, body: &[u8]) {
    let headers = rqctx.request.headers();
    let sent = |name: &str| headers.get(name).and_then(|value| value.to_str().ok());
    let body_shape = if body.trim_ascii().is_empty() {
        "empty"
    } else {
        match serde_json::from_slice(body) {
            Ok(serde_json::Value::Array(_)) => "array",
            Ok(serde_json::Value::Object(_)) => "object",
            Ok(_) => "other JSON",
            Err(_) => "not JSON",
        }
    };
    slog::info!(
        &rqctx.log,
        "MCP request rejected";
        "status" => status.as_u16(),
        "content_type" => sent(header::CONTENT_TYPE.as_str()),
        "content_encoding" => sent(header::CONTENT_ENCODING.as_str()),
        "content_length" => sent(header::CONTENT_LENGTH.as_str()),
        "transfer_encoding" => sent(header::TRANSFER_ENCODING.as_str()),
        "user_agent" => sent(header::USER_AGENT.as_str()),
        "mcp_protocol_version" => sent(HEADER_MCP_PROTOCOL_VERSION),
        "body_bytes" => body.len(),
        "body_shape" => body_shape,
    );
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
