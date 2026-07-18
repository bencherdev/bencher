use bencher_schema::{
    context::{ApiContext, RateLimiting},
    model::user::{actor::ApiActor, public::PublicUser},
};
use dropshot::{HttpError, RequestContext};
use rmcp::model::{
    CallToolRequest, CallToolRequestParams, ClientJsonRpcMessage, ClientRequest, ErrorCode,
    ErrorData, Implementation, InitializeRequest, InitializeResult, JsonRpcError, JsonRpcMessage,
    JsonRpcRequest, ProtocolVersion, RequestId, ServerCapabilities, ServerJsonRpcMessage,
    ServerResult, ToolsCapability,
};

use crate::tools::McpTool;

const SERVER_NAME: &str = "bencher";
const SERVER_INSTRUCTIONS: &str = "Bencher tracks benchmark results over time and catches performance regressions. \
Most tools take a `project` argument that accepts either a project slug or UUID. \
Authenticate with an `Authorization: Bearer` header using a project API key (`bencher_run_`) \
or a user API key (`bencher_user_`); public projects can be read without authentication. \
List tools return an object with the items under `data` and the total number of results under `total_count`. \
To run benchmarks and submit results from a local checkout, prefer the `bencher` CLI (`bencher run`).";

/// Protocol versions this server accepts, newest first.
/// The Streamable HTTP transport was introduced in 2025-03-26; older
/// clients are offered the newest supported version and may disconnect.
const SUPPORTED_PROTOCOL_VERSIONS: [ProtocolVersion; 3] = [
    ProtocolVersion::V_2025_11_25,
    ProtocolVersion::V_2025_06_18,
    ProtocolVersion::V_2025_03_26,
];

/// Handle a single JSON-RPC message.
/// Returns `None` when there is nothing to send back (notifications).
pub async fn handle(
    rqctx: &RequestContext<ApiContext>,
    body: &[u8],
) -> Result<Option<ServerJsonRpcMessage>, HttpError> {
    let message: ClientJsonRpcMessage = match serde_json::from_slice(body) {
        Ok(message) => message,
        Err(err) => {
            apply_public_rate_limit(rqctx)?;
            // JSON-RPC batching was removed in MCP 2025-06-18
            let error = if body.trim_ascii_start().first() == Some(&b'[') {
                ErrorData::invalid_request("JSON-RPC batching is not supported", None)
            } else {
                ErrorData::parse_error(format!("Invalid JSON-RPC message: {err}"), None)
            };
            return Ok(Some(JsonRpcMessage::Error(JsonRpcError::new(None, error))));
        },
    };

    match message {
        JsonRpcMessage::Request(request) => handle_request(rqctx, request).await.map(Some),
        // This server never sends requests, so client responses, errors,
        // and notifications are accepted and discarded
        JsonRpcMessage::Response(_)
        | JsonRpcMessage::Notification(_)
        | JsonRpcMessage::Error(_) => {
            apply_public_rate_limit(rqctx)?;
            Ok(None)
        },
    }
}

#[expect(
    clippy::wildcard_enum_match_arm,
    reason = "unsupported MCP methods are rejected as method not found"
)]
async fn handle_request(
    rqctx: &RequestContext<ApiContext>,
    request: JsonRpcRequest<ClientRequest>,
) -> Result<ServerJsonRpcMessage, HttpError> {
    let JsonRpcRequest { id, request, .. } = request;
    match request {
        ClientRequest::InitializeRequest(request) => {
            apply_public_rate_limit(rqctx)?;
            Ok(initialize(id, &request))
        },
        ClientRequest::PingRequest(_) => {
            apply_public_rate_limit(rqctx)?;
            Ok(JsonRpcMessage::response(ServerResult::empty(()), id))
        },
        ClientRequest::ListToolsRequest(_) => {
            apply_public_rate_limit(rqctx)?;
            Ok(JsonRpcMessage::response(
                ServerResult::ListToolsResult(McpTool::list()),
                id,
            ))
        },
        ClientRequest::CallToolRequest(request) => call_tool(rqctx, id, request).await,
        request => {
            apply_public_rate_limit(rqctx)?;
            Ok(JsonRpcMessage::Error(JsonRpcError::new(
                Some(id),
                ErrorData::new(
                    ErrorCode::METHOD_NOT_FOUND,
                    format!("Method not found: {method}", method = request.method()),
                    None,
                ),
            )))
        },
    }
}

fn initialize(id: RequestId, request: &InitializeRequest) -> ServerJsonRpcMessage {
    let protocol_version = negotiate_protocol_version(&request.params.protocol_version);
    let result = InitializeResult::new(server_capabilities())
        .with_protocol_version(protocol_version)
        .with_server_info(Implementation::new(SERVER_NAME, env!("CARGO_PKG_VERSION")))
        .with_instructions(SERVER_INSTRUCTIONS);
    JsonRpcMessage::response(ServerResult::InitializeResult(result), id)
}

/// Echo a supported client protocol version;
/// otherwise offer the newest version this server supports.
fn negotiate_protocol_version(client_version: &ProtocolVersion) -> ProtocolVersion {
    if SUPPORTED_PROTOCOL_VERSIONS.contains(client_version) {
        client_version.clone()
    } else {
        SUPPORTED_PROTOCOL_VERSIONS[0].clone()
    }
}

fn server_capabilities() -> ServerCapabilities {
    // `ServerCapabilities` is non-exhaustive, so its builder is unavailable
    // without the rmcp `server` feature and a struct literal is not allowed
    let mut capabilities = ServerCapabilities::default();
    capabilities.tools = Some(ToolsCapability::default());
    capabilities
}

async fn call_tool(
    rqctx: &RequestContext<ApiContext>,
    id: RequestId,
    request: CallToolRequest,
) -> Result<ServerJsonRpcMessage, HttpError> {
    let CallToolRequestParams {
        name, arguments, ..
    } = request.params;
    let Some(tool) = McpTool::from_name(&name) else {
        apply_public_rate_limit(rqctx)?;
        return Ok(JsonRpcMessage::Error(JsonRpcError::new(
            Some(id),
            ErrorData::invalid_params(format!("Unknown tool: {name}"), None),
        )));
    };

    // Invalid or expired credentials fail the whole request with a `401`,
    // just like the REST API; anonymous requests may still read public projects.
    // Rate limiting is applied per-actor inside the shared endpoint handlers,
    // with the same exceptions as REST (`list_projects` mirrors `GET /v0/projects`).
    let api_actor = ApiActor::new(rqctx).await?;

    // Anonymous tool calls are also throttled before dispatch: malformed
    // arguments are rejected before reaching the shared handlers where
    // per-actor rate limiting lives (REST body and query parse failures
    // are likewise rejected before rate limiting)
    if matches!(&api_actor, ApiActor::Public(PublicUser::Public(_))) {
        apply_public_rate_limit(rqctx)?;
    }

    let arguments = serde_json::Value::Object(arguments.unwrap_or_default());
    let result = tool
        .call(
            &rqctx.log,
            rqctx.context(),
            api_actor,
            rqctx.request.headers(),
            arguments,
        )
        .await;
    Ok(JsonRpcMessage::response(
        ServerResult::CallToolResult(result),
        id,
    ))
}

fn apply_public_rate_limit(rqctx: &RequestContext<ApiContext>) -> Result<(), HttpError> {
    let context = rqctx.context();
    if let Some(remote_ip) = RateLimiting::remote_ip(&rqctx.log, rqctx.request.headers()) {
        context.rate_limiting.public_request(remote_ip)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{SUPPORTED_PROTOCOL_VERSIONS, negotiate_protocol_version, server_capabilities};
    use rmcp::model::ProtocolVersion;

    #[test]
    fn negotiate_supported_versions_are_echoed() {
        for version in &SUPPORTED_PROTOCOL_VERSIONS {
            assert_eq!(negotiate_protocol_version(version), version.clone());
        }
    }

    #[test]
    fn negotiate_unsupported_version_offers_newest_supported() {
        assert_eq!(
            negotiate_protocol_version(&ProtocolVersion::V_2024_11_05),
            SUPPORTED_PROTOCOL_VERSIONS[0]
        );
    }

    #[test]
    fn capabilities_enable_tools() {
        assert!(server_capabilities().tools.is_some());
    }
}
