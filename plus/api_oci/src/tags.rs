//! OCI Tags Endpoint
//!
//! - GET /v2/<name>/tags/list - List tags for a repository

use bencher_endpoint::{CorsResponse, Endpoint, Get};
use bencher_json::ProjectResourceId;
use bencher_oci_storage::OciError;
use bencher_schema::context::ApiContext;
use dropshot::{Body, HttpError, Path, Query, RequestContext, endpoint};
use http::Response;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use crate::auth::require_pull_access;

/// Path parameters for tags list
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TagsPath {
    /// Project resource ID (UUID or slug)
    pub name: ProjectResourceId,
}

/// Query parameters for tags list pagination
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TagsQuery {
    /// Number of tags to return
    pub n: Option<u32>,
    /// Last tag from previous response (for pagination)
    pub last: Option<String>,
}

/// Response for tags list
#[derive(Debug, Serialize, JsonSchema)]
pub struct TagsListResponse {
    /// Repository name
    pub name: String,
    /// List of tags
    pub tags: Vec<String>,
}

/// CORS preflight for tags list
#[endpoint {
    method = OPTIONS,
    path = "/v2/{name}/tags/list",
    tags = ["oci"],
    unpublished = true,
}]
pub async fn oci_tags_options(
    _rqctx: RequestContext<ApiContext>,
    _path: Path<TagsPath>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// Default number of tags to return when n is not specified
const DEFAULT_PAGE_SIZE: u32 = 100;

/// List tags for a repository
///
/// Returns a list of tags for the specified repository with optional pagination.
/// When more results are available, a `Link` header is included with a URL to fetch
/// the next page of results per the OCI Distribution Spec.
#[endpoint {
    method = GET,
    path = "/v2/{name}/tags/list",
    tags = ["oci"],
}]
pub async fn oci_tags_list(
    rqctx: RequestContext<ApiContext>,
    path: Path<TagsPath>,
    query: Query<TagsQuery>,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let path = path.into_inner();
    let query = query.into_inner();

    // Authenticate and apply rate limiting
    let name_str = path.name.to_string();
    let _access = require_pull_access(&rqctx, &name_str).await?;

    // Get storage
    let storage = context.oci_storage();

    // List tags
    let mut tags = storage
        .list_tags(&path.name)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Record metric
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OciTagsList);

    // Sort tags for consistent ordering
    tags.sort();

    // Apply pagination - filter by last
    if let Some(last) = &query.last {
        // Find the position of the last tag and skip to after it
        if let Some(pos) = tags.iter().position(|t| t == last) {
            tags = tags.into_iter().skip(pos + 1).collect();
        }
    }

    // Determine page size and check if more results exist
    let page_size = query.n.unwrap_or(DEFAULT_PAGE_SIZE) as usize;
    let has_more = tags.len() > page_size;

    // Truncate to requested page size
    tags.truncate(page_size);

    // Build response body
    let response_body = TagsListResponse {
        name: name_str.clone(),
        tags: tags.clone(),
    };
    let body = serde_json::to_vec(&response_body)
        .map_err(|e| HttpError::for_internal_error(format!("Failed to serialize response: {e}")))?;

    // Build response with OCI-compliant headers
    let mut builder = Response::builder()
        .status(http::StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "application/json")
        .header(http::header::CONTENT_LENGTH, body.len())
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*");

    // Add Link header for pagination if there are more results
    if has_more && let Some(last_tag) = tags.last() {
        let n = query.n.unwrap_or(DEFAULT_PAGE_SIZE);
        let link = format!(
            "</v2/{}/tags/list?n={}&last={}>; rel=\"next\"",
            name_str,
            n,
            urlencoding::encode(last_tag)
        );
        builder = builder
            .header("Link", link)
            .header(http::header::ACCESS_CONTROL_EXPOSE_HEADERS, "Link");
    }

    builder
        .body(Body::from(body))
        .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))
}
