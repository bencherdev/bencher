use bencher_schema::context::OAuthServer;
use rmcp::model::{
    JsonObject, ListResourcesResult, MetaObject, ReadResourceResult, Resource, ResourceContents,
};
use serde_json::json;

/// Explore's plot, built into one file by `npm run mcp` in `services/console`.
const PERF_HTML: &str = include_str!("../ui/perf.html");
/// Hosts cache a page by its URI, so each release names its own.
const PERF_URI: &str = concat!("ui://bencher/perf-v", env!("CARGO_PKG_VERSION"), ".html");
const MCP_APP_MIME_TYPE: &str = "text/html;profile=mcp-app";

/// The page that draws `query_perf`'s result, under the MCP Apps key and `ChatGPT`'s, which may call
/// the tool again to refresh.
pub fn perf_tool_meta() -> MetaObject {
    MetaObject(JsonObject::from_iter([
        ("ui".to_owned(), json!({"resourceUri": PERF_URI})),
        ("openai/outputTemplate".to_owned(), json!(PERF_URI)),
        ("openai/widgetAccessible".to_owned(), json!(true)),
    ]))
}

pub fn list(oauth: Option<&OAuthServer>) -> ListResourcesResult {
    ListResourcesResult::with_all_items(vec![
        Resource::new(PERF_URI, "perf_plot")
            .with_title("Perf plot")
            .with_mime_type(MCP_APP_MIME_TYPE)
            .with_meta(resource_meta(oauth)),
    ])
}

pub fn read(uri: &str, oauth: Option<&OAuthServer>) -> Option<ReadResourceResult> {
    (uri == PERF_URI).then(|| {
        ReadResourceResult::new(vec![
            ResourceContents::text(PERF_HTML, PERF_URI)
                .with_mime_type(MCP_APP_MIME_TYPE)
                .with_meta(resource_meta(oauth)),
        ])
    })
}

/// The host draws the border, and `ChatGPT` serves the page from the origin of the server's public MCP endpoint.
fn resource_meta(oauth: Option<&OAuthServer>) -> MetaObject {
    let mut meta = JsonObject::from_iter([
        ("ui".to_owned(), json!({"prefersBorder": true})),
        ("openai/widgetPrefersBorder".to_owned(), json!(true)),
    ]);
    if let Some(oauth) = oauth {
        meta.insert(
            "openai/widgetDomain".to_owned(),
            json!(oauth.canonical_resource().origin()),
        );
    }
    MetaObject(meta)
}
