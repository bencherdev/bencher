use std::{
    borrow::Cow,
    panic::AssertUnwindSafe,
    sync::{Arc, OnceLock},
};

use bencher_schema::context::ApiContext;
use dropshot::{Body, ClientErrorStatusCode, HttpError};
use futures::FutureExt as _;
use http::Response;
use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResponse, CallToolResult, CompleteRequestMethod,
        CompleteRequestParams, CompleteResult, Implementation, ListPromptsRequestMethod,
        ListPromptsResult, ListResourceTemplatesRequestMethod, ListResourceTemplatesResult,
        ListResourcesRequestMethod, ListResourcesResult, ListToolsResult, PaginatedRequestParams,
        ProtocolVersion, RequestMetaObject, ServerCapabilities, ServerConfig,
    },
    service::{MaybeSendFuture, RequestContext},
};
use slog::Logger;

use crate::{
    principal::{Caller, Challenge, ClientStyle, McpPrincipal, SignInReason, resource_metadata},
    tools::{McpTool, error_result},
};

const SERVER_NAME: &str = "bencher";
const SERVER_INSTRUCTIONS: &str = "Bencher tracks benchmark results over time and catches performance regressions. \
Most tools take a `project` argument that accepts either a project slug or UUID. \
Authenticate with an `Authorization: Bearer` header using a project API key (`bencher_run_`) \
or a user API key (`bencher_user_`); public projects can be read without authentication. \
List tools return an object with the items under `data` and the total number of results under `total_count`. \
To run benchmarks and submit results from a local checkout, prefer the `bencher` CLI (`bencher run`).";
const SIGN_IN_INSTRUCTIONS: &str = "Clients that support OAuth can sign in instead of using a key, \
and a tool that needs a signed-in caller asks for it.";

/// The Streamable HTTP transport was introduced in 2025-03-26.
const SUPPORTED_PROTOCOL_VERSIONS: &[ProtocolVersion] = &[
    ProtocolVersion::V_2025_03_26,
    ProtocolVersion::V_2025_06_18,
    ProtocolVersion::V_2025_11_25,
    ProtocolVersion::V_2026_07_28,
];

/// Serves a single MCP request as the caller the endpoint resolved for it.
pub struct BencherMcp {
    rqctx: Arc<dropshot::RequestContext<ApiContext>>,
    caller: Caller,
    /// Set when the endpoint must answer with a challenge in place of the SDK's response.
    challenge: OnceLock<Challenge>,
}

impl BencherMcp {
    pub fn new(rqctx: dropshot::RequestContext<ApiContext>, caller: Caller) -> Self {
        Self {
            rqctx: Arc::new(rqctx),
            caller,
            challenge: OnceLock::new(),
        }
    }

    /// The response to answer in place of the SDK's, once a tool call raised a challenge.
    pub fn challenge_response(&self) -> Option<Result<Response<Body>, HttpError>> {
        let challenge = self.challenge.get()?;
        let oauth = self.rqctx.context().oauth.as_ref()?;
        Some(challenge.into_response(&resource_metadata(oauth, &self.rqctx)))
    }

    /// Ask the client to sign in where it looks for the request:
    /// `ChatGPT` in the tool result, every other client in the HTTP response.
    fn challenge(&self, challenge: Challenge, meta: &RequestMetaObject) -> CallToolResult {
        let style = ClientStyle::of(meta, self.rqctx.request.headers());
        #[cfg(feature = "otel")]
        bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::McpChallenge(
            challenge.kind(),
            style.into(),
        ));
        if matches!(style, ClientStyle::ChatGpt)
            && let Some(oauth) = &self.rqctx.context().oauth
        {
            return challenge.into_result(&resource_metadata(oauth, &self.rqctx));
        }
        self.challenge
            .get_or_init(|| challenge)
            .into_status_result()
    }

    /// An anonymous caller is asked to sign in on a `404` as well as a `401`, with text that never
    /// carries the tool's error, so a private project and a missing one answer alike.
    fn tool_error(
        &self,
        tool: McpTool,
        principal: &McpPrincipal,
        error: HttpError,
        meta: &RequestMetaObject,
    ) -> CallToolResult {
        if principal.is_anonymous()
            && self.rqctx.context().oauth.is_some()
            && (error.status_code == ClientErrorStatusCode::UNAUTHORIZED
                || error.status_code == ClientErrorStatusCode::NOT_FOUND)
        {
            let challenge = Challenge::SignIn {
                access: tool.access(),
                reason: SignInReason::Required,
            };
            self.challenge(challenge, meta)
        } else {
            error_result(error)
        }
    }
}

impl ServerHandler for BencherMcp {
    fn get_info(&self) -> ServerConfig {
        let instructions = if self.rqctx.context().oauth.is_some() {
            format!("{SERVER_INSTRUCTIONS} {SIGN_IN_INSTRUCTIONS}")
        } else {
            SERVER_INSTRUCTIONS.to_owned()
        };
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(Implementation::new(SERVER_NAME, env!("CARGO_PKG_VERSION")))
            .with_instructions(instructions)
    }

    fn supported_protocol_versions(&self) -> Cow<'static, [ProtocolVersion]> {
        Cow::Borrowed(SUPPORTED_PROTOCOL_VERSIONS)
    }

    fn list_tools(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListToolsResult, ErrorData>> + MaybeSendFuture + '_ {
        let list = if self.rqctx.context().oauth.is_some() {
            McpTool::list_with_security_schemes()
        } else {
            McpTool::list()
        };
        std::future::ready(Ok(list))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
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
        let principal = match &self.caller {
            Caller::Principal(principal) => principal,
            Caller::OverLimit => {
                let challenge = Challenge::SignIn {
                    access: tool.access(),
                    reason: SignInReason::OverLimit,
                };
                return Ok(self.challenge(challenge, &context.meta).into());
            },
        };
        if !principal.permits(tool.access()) {
            return Ok(self
                .challenge(Challenge::InsufficientScope, &context.meta)
                .into());
        }

        let arguments = serde_json::Value::Object(arguments.unwrap_or_default());
        let call = tool.call(
            &self.rqctx.log,
            self.rqctx.context(),
            principal.actor().clone(),
            self.rqctx.request.headers(),
            arguments,
        );
        let result = catch_tool_panic(&self.rqctx.log, tool, call)
            .await?
            .unwrap_or_else(|error| self.tool_error(tool, principal, error, &context.meta));
        Ok(result.into())
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
async fn catch_tool_panic<F, T>(log: &Logger, tool: McpTool, call: F) -> Result<T, ErrorData>
where
    F: Future<Output = T>,
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
