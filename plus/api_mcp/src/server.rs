use std::{borrow::Cow, panic::AssertUnwindSafe, sync::Arc};

use bencher_schema::{context::ApiContext, model::user::actor::ApiActor};
use futures::FutureExt as _;
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, CompleteRequestMethod,
        CompleteRequestParams, CompleteResult, Implementation, ListPromptsRequestMethod,
        ListPromptsResult, ListResourceTemplatesRequestMethod, ListResourceTemplatesResult,
        ListResourcesRequestMethod, ListResourcesResult, ListToolsResult, PaginatedRequestParams,
        ProtocolVersion, ServerCapabilities, ServerConfig,
    },
    service::{MaybeSendFuture, RequestContext},
};
use slog::Logger;

use crate::tools::McpTool;

const SERVER_NAME: &str = "bencher";
const SERVER_INSTRUCTIONS: &str = "Bencher tracks benchmark results over time and catches performance regressions. \
Most tools take a `project` argument that accepts either a project slug or UUID. \
Authenticate with an `Authorization: Bearer` header using a project API key (`bencher_run_`) \
or a user API key (`bencher_user_`); public projects can be read without authentication. \
List tools return an object with the items under `data` and the total number of results under `total_count`. \
To run benchmarks and submit results from a local checkout, prefer the `bencher` CLI (`bencher run`).";

/// The Streamable HTTP transport was introduced in 2025-03-26.
const SUPPORTED_PROTOCOL_VERSIONS: &[ProtocolVersion] = &[
    ProtocolVersion::V_2025_03_26,
    ProtocolVersion::V_2025_06_18,
    ProtocolVersion::V_2025_11_25,
    ProtocolVersion::V_2026_07_28,
];

/// Serves a single MCP request with the credentials the endpoint resolved for it.
pub struct BencherMcp {
    rqctx: Arc<dropshot::RequestContext<ApiContext>>,
    /// Only resolved for a call to a known tool.
    api_actor: Option<ApiActor>,
}

impl BencherMcp {
    pub fn new(rqctx: dropshot::RequestContext<ApiContext>, api_actor: Option<ApiActor>) -> Self {
        Self {
            rqctx: Arc::new(rqctx),
            api_actor,
        }
    }
}

impl ServerHandler for BencherMcp {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(SERVER_NAME, env!("CARGO_PKG_VERSION")))
            .with_instructions(SERVER_INSTRUCTIONS)
    }

    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(SUPPORTED_PROTOCOL_VERSIONS)
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + MaybeSendFuture + '_ {
        std::future::ready(Ok(McpTool::list()))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let CallToolRequestParams {
            name, arguments, ..
        } = request;
        let Some(tool) = McpTool::from_name(&name) else {
            return Err(ErrorData::invalid_params(
                format!("Unknown tool: {name}"),
                None,
            ));
        };
        let Some(api_actor) = self.api_actor.clone() else {
            return Err(ErrorData::invalid_request("Malformed tool call", None));
        };

        let arguments = serde_json::Value::Object(arguments.unwrap_or_default());
        let call = tool.call(
            &self.rqctx.log,
            self.rqctx.context(),
            api_actor,
            self.rqctx.request.headers(),
            arguments,
        );
        catch_tool_panic(&self.rqctx.log, tool, call)
            .await
            .map(Into::into)
    }

    // Only the tools capability is advertised, but the SDK answers these with empty results
    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourcesResult, ErrorData>> + MaybeSendFuture + '_ {
        std::future::ready(Err(
            ErrorData::method_not_found::<ListResourcesRequestMethod>(),
        ))
    }

    fn list_resource_templates(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourceTemplatesResult, ErrorData>> + MaybeSendFuture + '_
    {
        std::future::ready(Err(ErrorData::method_not_found::<
            ListResourceTemplatesRequestMethod,
        >()))
    }

    fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListPromptsResult, ErrorData>> + MaybeSendFuture + '_ {
        std::future::ready(Err(
            ErrorData::method_not_found::<ListPromptsRequestMethod>(),
        ))
    }

    fn complete(
        &self,
        _request: CompleteRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<CompleteResult, ErrorData>> + MaybeSendFuture + '_ {
        std::future::ready(Err(ErrorData::method_not_found::<CompleteRequestMethod>()))
    }
}

/// The SDK runs the handler on a task of its own,
/// where a panic would leave the request waiting forever for a response.
async fn catch_tool_panic<F>(
    log: &Logger,
    tool: McpTool,
    call: F,
) -> Result<CallToolResult, ErrorData>
where
    F: Future<Output = CallToolResult>,
{
    AssertUnwindSafe(call)
        .catch_unwind()
        .await
        .map_err(|_panic| {
            slog::error!(log, "MCP tool call panicked"; "tool" => tool.name());
            ErrorData::internal_error("Tool call failed", None)
        })
}

#[cfg(test)]
mod tests {
    use rmcp::model::{CallToolResult, ErrorCode};

    use super::catch_tool_panic;
    use crate::tools::McpTool;

    #[tokio::test]
    async fn tool_panic_is_an_internal_error() {
        let log = slog::Logger::root(slog::Discard, slog::o!());
        let call = futures::future::lazy(|_| -> CallToolResult { panic!("tool panic") });

        let result = catch_tool_panic(&log, McpTool::ViewProject, call).await;

        assert_eq!(
            result.map_err(|error| error.code),
            Err(ErrorCode::INTERNAL_ERROR)
        );
    }
}
