use std::sync::{Arc, LazyLock};

use bencher_endpoint::TotalCount;
use bencher_schema::{
    context::ApiContext,
    error::{bad_request_error, with_auth_hint},
    model::user::actor::ApiActor,
};
use dropshot::HttpError;
use rmcp::model::{CallToolResult, ContentBlock, JsonObject, ListToolsResult, Tool};
use serde::Serialize;
use serde_json::Value;
use slog::Logger;
use strum::IntoEnumIterator as _;

mod alerts;
mod benchmarks;
mod branches;
mod jobs;
mod measures;
mod metrics;
mod perf;
mod plots;
mod projects;
mod reports;
mod run;
mod testbeds;
mod thresholds;

/// The MCP tool surface: one tool per REST operation available to a
/// project API key (`bencher_run_`), so a connected agent can never do
/// more than the credential it holds. Notably absent: deletes and renames.
/// A project API key is confined to its own project; a user API key spans
/// the user's projects, exactly as over REST.
/// `EnumIter` derives the complete tool list, so a new variant cannot be
/// omitted from `tools/list`: every `match` below is compiler-checked.
#[derive(Debug, Clone, Copy, strum::EnumIter)]
pub enum McpTool {
    SubmitRun,
    ListProjects,
    ViewProject,
    ListReports,
    CreateReport,
    ViewReport,
    ListBranches,
    CreateBranch,
    ViewBranch,
    UpdateBranch,
    ListTestbeds,
    CreateTestbed,
    ViewTestbed,
    UpdateTestbed,
    ListBenchmarks,
    CreateBenchmark,
    ViewBenchmark,
    UpdateBenchmark,
    ListMeasures,
    CreateMeasure,
    ViewMeasure,
    UpdateMeasure,
    ListThresholds,
    CreateThreshold,
    ViewThreshold,
    UpdateThreshold,
    ListAlerts,
    ViewAlert,
    UpdateAlert,
    ViewMetric,
    QueryPerf,
    PerfImage,
    ListPlots,
    ViewPlot,
    ListJobs,
    ViewJob,
}

impl McpTool {
    pub fn from_name(name: &str) -> Option<Self> {
        Self::iter().find(|tool| tool.name() == name)
    }

    pub const fn name(self) -> &'static str {
        match self {
            Self::SubmitRun => "submit_run",
            Self::ListProjects => "list_projects",
            Self::ViewProject => "view_project",
            Self::ListReports => "list_reports",
            Self::CreateReport => "create_report",
            Self::ViewReport => "view_report",
            Self::ListBranches => "list_branches",
            Self::CreateBranch => "create_branch",
            Self::ViewBranch => "view_branch",
            Self::UpdateBranch => "update_branch",
            Self::ListTestbeds => "list_testbeds",
            Self::CreateTestbed => "create_testbed",
            Self::ViewTestbed => "view_testbed",
            Self::UpdateTestbed => "update_testbed",
            Self::ListBenchmarks => "list_benchmarks",
            Self::CreateBenchmark => "create_benchmark",
            Self::ViewBenchmark => "view_benchmark",
            Self::UpdateBenchmark => "update_benchmark",
            Self::ListMeasures => "list_measures",
            Self::CreateMeasure => "create_measure",
            Self::ViewMeasure => "view_measure",
            Self::UpdateMeasure => "update_measure",
            Self::ListThresholds => "list_thresholds",
            Self::CreateThreshold => "create_threshold",
            Self::ViewThreshold => "view_threshold",
            Self::UpdateThreshold => "update_threshold",
            Self::ListAlerts => "list_alerts",
            Self::ViewAlert => "view_alert",
            Self::UpdateAlert => "update_alert",
            Self::ViewMetric => "view_metric",
            Self::QueryPerf => "query_perf",
            Self::PerfImage => "perf_image",
            Self::ListPlots => "list_plots",
            Self::ViewPlot => "view_plot",
            Self::ListJobs => "list_jobs",
            Self::ViewJob => "view_job",
        }
    }

    pub fn list() -> ListToolsResult {
        // The tool list is static, so the schemas are generated once
        static LIST: LazyLock<ListToolsResult> = LazyLock::new(|| {
            ListToolsResult::with_all_items(McpTool::iter().map(McpTool::spec).collect())
        });
        LIST.clone()
    }

    fn spec(self) -> Tool {
        match self {
            Self::SubmitRun => run::submit_tool(self.name()),
            Self::ListProjects => projects::list_tool(self.name()),
            Self::ViewProject => projects::view_tool(self.name()),
            Self::ListReports => reports::list_tool(self.name()),
            Self::CreateReport => reports::create_tool(self.name()),
            Self::ViewReport => reports::view_tool(self.name()),
            Self::ListBranches => branches::list_tool(self.name()),
            Self::CreateBranch => branches::create_tool(self.name()),
            Self::ViewBranch => branches::view_tool(self.name()),
            Self::UpdateBranch => branches::update_tool(self.name()),
            Self::ListTestbeds => testbeds::list_tool(self.name()),
            Self::CreateTestbed => testbeds::create_tool(self.name()),
            Self::ViewTestbed => testbeds::view_tool(self.name()),
            Self::UpdateTestbed => testbeds::update_tool(self.name()),
            Self::ListBenchmarks => benchmarks::list_tool(self.name()),
            Self::CreateBenchmark => benchmarks::create_tool(self.name()),
            Self::ViewBenchmark => benchmarks::view_tool(self.name()),
            Self::UpdateBenchmark => benchmarks::update_tool(self.name()),
            Self::ListMeasures => measures::list_tool(self.name()),
            Self::CreateMeasure => measures::create_tool(self.name()),
            Self::ViewMeasure => measures::view_tool(self.name()),
            Self::UpdateMeasure => measures::update_tool(self.name()),
            Self::ListThresholds => thresholds::list_tool(self.name()),
            Self::CreateThreshold => thresholds::create_tool(self.name()),
            Self::ViewThreshold => thresholds::view_tool(self.name()),
            Self::UpdateThreshold => thresholds::update_tool(self.name()),
            Self::ListAlerts => alerts::list_tool(self.name()),
            Self::ViewAlert => alerts::view_tool(self.name()),
            Self::UpdateAlert => alerts::update_tool(self.name()),
            Self::ViewMetric => metrics::view_tool(self.name()),
            Self::QueryPerf => perf::query_tool(self.name()),
            Self::PerfImage => perf::image_tool(self.name()),
            Self::ListPlots => plots::list_tool(self.name()),
            Self::ViewPlot => plots::view_tool(self.name()),
            Self::ListJobs => jobs::list_tool(self.name()),
            Self::ViewJob => jobs::view_tool(self.name()),
        }
    }

    pub async fn call(
        self,
        log: &Logger,
        context: &ApiContext,
        api_actor: ApiActor,
        headers: &http::HeaderMap,
        arguments: Value,
    ) -> CallToolResult {
        match self
            .try_call(log, context, api_actor, headers, arguments)
            .await
            .map_err(with_auth_hint)
        {
            Ok(result) => result,
            Err(err) => error_result(&err),
        }
    }

    async fn try_call(
        self,
        log: &Logger,
        context: &ApiContext,
        api_actor: ApiActor,
        headers: &http::HeaderMap,
        arguments: Value,
    ) -> Result<CallToolResult, HttpError> {
        // Each arm is boxed so this future stays small: inlining all of the
        // endpoint handler futures into one `match` overflows the compiler's
        // `Send` bound evaluation for the enclosing dropshot endpoint
        let future: ToolFuture = match self {
            Self::SubmitRun => Box::pin(run::submit(log, context, api_actor, headers, arguments)),
            Self::ListProjects => Box::pin(projects::list(context, &api_actor, arguments)),
            Self::ViewProject => Box::pin(projects::view(context, &api_actor, arguments)),
            Self::ListReports => Box::pin(reports::list(log, context, &api_actor, arguments)),
            Self::CreateReport => Box::pin(reports::create(log, context, api_actor, arguments)),
            Self::ViewReport => Box::pin(reports::view(log, context, &api_actor, arguments)),
            Self::ListBranches => Box::pin(branches::list(context, &api_actor, arguments)),
            Self::CreateBranch => Box::pin(branches::create(log, context, &api_actor, arguments)),
            Self::ViewBranch => Box::pin(branches::view(context, &api_actor, arguments)),
            Self::UpdateBranch => Box::pin(branches::update(log, context, &api_actor, arguments)),
            Self::ListTestbeds => Box::pin(testbeds::list(context, &api_actor, arguments)),
            Self::CreateTestbed => Box::pin(testbeds::create(context, &api_actor, arguments)),
            Self::ViewTestbed => Box::pin(testbeds::view(context, &api_actor, arguments)),
            Self::UpdateTestbed => Box::pin(testbeds::update(context, &api_actor, arguments)),
            Self::ListBenchmarks => Box::pin(benchmarks::list(context, &api_actor, arguments)),
            Self::CreateBenchmark => Box::pin(benchmarks::create(context, &api_actor, arguments)),
            Self::ViewBenchmark => Box::pin(benchmarks::view(context, &api_actor, arguments)),
            Self::UpdateBenchmark => Box::pin(benchmarks::update(context, &api_actor, arguments)),
            Self::ListMeasures => Box::pin(measures::list(context, &api_actor, arguments)),
            Self::CreateMeasure => Box::pin(measures::create(context, &api_actor, arguments)),
            Self::ViewMeasure => Box::pin(measures::view(context, &api_actor, arguments)),
            Self::UpdateMeasure => Box::pin(measures::update(context, &api_actor, arguments)),
            Self::ListThresholds => Box::pin(thresholds::list(context, &api_actor, arguments)),
            Self::CreateThreshold => Box::pin(thresholds::create(context, &api_actor, arguments)),
            Self::ViewThreshold => Box::pin(thresholds::view(context, &api_actor, arguments)),
            Self::UpdateThreshold => Box::pin(thresholds::update(context, &api_actor, arguments)),
            Self::ListAlerts => Box::pin(alerts::list(context, &api_actor, arguments)),
            Self::ViewAlert => Box::pin(alerts::view(context, &api_actor, arguments)),
            Self::UpdateAlert => Box::pin(alerts::update(context, &api_actor, arguments)),
            Self::ViewMetric => Box::pin(metrics::view(context, &api_actor, arguments)),
            Self::QueryPerf => Box::pin(perf::query(log, context, &api_actor, arguments)),
            Self::PerfImage => Box::pin(perf::image(log, context, &api_actor, arguments)),
            Self::ListPlots => Box::pin(plots::list(context, &api_actor, arguments)),
            Self::ViewPlot => Box::pin(plots::view(context, &api_actor, arguments)),
            Self::ListJobs => Box::pin(jobs::list(context, &api_actor, arguments)),
            Self::ViewJob => Box::pin(jobs::view(log, context, &api_actor, arguments)),
        };
        future.await
    }
}

/// A boxed tool call future, keeping the dispatch future small.
type ToolFuture<'a> =
    std::pin::Pin<Box<dyn Future<Output = Result<CallToolResult, HttpError>> + Send + 'a>>;

/// Deserialize tool arguments, mirroring the REST layer's `400 Bad Request`
/// for malformed input.
fn parse_input<T>(arguments: Value) -> Result<T, HttpError>
where
    T: serde::de::DeserializeOwned,
{
    serde_json::from_value(arguments)
        .map_err(|err| bad_request_error(format!("Invalid tool arguments: {err}")))
}

/// Generate a self-contained JSON Schema for a tool's input type.
/// Subschemas are inlined because MCP clients do not resolve `$ref`s
/// against a `definitions` map. Inlining recurses without bound, so tool
/// input types must not be recursive.
/// Generation cannot fail for the concrete tool input types; the
/// `tool_specs_have_object_schemas` test guards that every schema is an
/// object, so the empty-object fallback is unreachable in practice.
fn input_schema<T>() -> Arc<JsonObject>
where
    T: schemars::JsonSchema,
{
    let settings = schemars::r#gen::SchemaSettings::draft07().with(|settings| {
        settings.inline_subschemas = true;
    });
    let schema = settings.into_generator().into_root_schema_for::<T>();
    if let Ok(Value::Object(object)) = serde_json::to_value(schema.schema) {
        Arc::new(object)
    } else {
        debug_assert!(false, "Failed to generate JSON Schema object");
        Arc::new(JsonObject::new())
    }
}

/// A successful tool result carrying the same JSON the REST endpoint returns.
fn json_result<T>(json: &T) -> Result<CallToolResult, HttpError>
where
    T: Serialize,
{
    let value = serde_json::to_value(json)
        .map_err(|e| HttpError::for_internal_error(format!("Failed to serialize result: {e}")))?;
    Ok(success_result(value))
}

/// A successful list result. The REST endpoints return the total count in the
/// `X-Total-Count` header; tools have no headers, so it rides in the body.
fn list_result<T>(json: &T, total_count: TotalCount) -> Result<CallToolResult, HttpError>
where
    T: Serialize,
{
    let value = serde_json::to_value(json)
        .map_err(|e| HttpError::for_internal_error(format!("Failed to serialize result: {e}")))?;
    Ok(success_result(serde_json::json!({
        "data": value,
        "total_count": total_count,
    })))
}

fn success_result(value: Value) -> CallToolResult {
    // MCP `structuredContent` must be a JSON object;
    // every tool returns one today and new tools must too
    debug_assert!(
        value.is_object(),
        "MCP structuredContent must be a JSON object"
    );
    // The result is deliberately carried twice, as text content and as
    // structured content, per the MCP compatibility recommendation:
    // clients that predate structured content only read the text block.
    // This roughly doubles the payload for large results; revisit if
    // response sizes become a problem in practice.
    let mut result = CallToolResult::success(vec![ContentBlock::text(value.to_string())]);
    result.structured_content = Some(value);
    result
}

/// Map an `HttpError` to a tool error, exposing only the status code and the
/// user-facing external message (never `internal_message`).
fn error_result(err: &HttpError) -> CallToolResult {
    CallToolResult::error(vec![ContentBlock::text(format!(
        "{status}: {message}",
        status = err.status_code.as_u16(),
        message = err.external_message,
    ))])
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use strum::IntoEnumIterator as _;

    use super::McpTool;

    #[test]
    fn tool_names_round_trip_and_are_unique() {
        let mut names = HashSet::new();
        for tool in McpTool::iter() {
            let name = tool.name();
            assert!(names.insert(name), "Duplicate tool name: {name}");
            assert!(
                McpTool::from_name(name).is_some(),
                "Tool name does not round trip: {name}"
            );
        }
        assert!(McpTool::from_name("no_such_tool").is_none());
    }

    #[test]
    fn tool_specs_have_object_schemas() {
        let list = McpTool::list();
        assert_eq!(list.tools.len(), McpTool::iter().count());
        for tool in &list.tools {
            assert!(
                tool.description.is_some(),
                "Tool is missing a description: {name}",
                name = tool.name
            );
            assert_eq!(
                tool.input_schema.get("type").and_then(|t| t.as_str()),
                Some("object"),
                "Tool input schema is not an object: {name}",
                name = tool.name
            );
        }
    }
}
