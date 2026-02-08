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

use crate::auth::{require_pull_access, resolve_project_uuid};
use crate::response::oci_cors_headers;

/// Path parameters for tags list
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TagsPath {
    /// Project resource ID (UUID or slug)
    pub name: ProjectResourceId,
}

/// Query parameters for tags list pagination
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TagsQuery {
    /// Number of tags to return (default: 100)
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

/// Maximum page size to prevent excessive resource usage
const MAX_PAGE_SIZE: u32 = 10_000;

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

    // Resolve project UUID for stable storage paths
    let project_uuid = resolve_project_uuid(context, &path.name).await?;

    // Get storage
    let storage = context.oci_storage();

    // Determine page size, clamped to [1, MAX_PAGE_SIZE]
    let page_size = query.n.unwrap_or(DEFAULT_PAGE_SIZE).clamp(1, MAX_PAGE_SIZE) as usize;

    // Validate the `last` cursor if provided
    let last_tag = if let Some(last) = &query.last {
        let _tag: bencher_oci_storage::Tag = last.parse().map_err(|_err| {
            crate::error::into_http_error(OciError::TagInvalid { tag: last.clone() })
        })?;
        Some(last.as_str())
    } else {
        None
    };

    // List tags with pagination handled at storage layer
    let result = storage
        .list_tags(&project_uuid, Some(page_size), last_tag)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Record metric
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OciTagsList);

    let tags = result.tags;
    let has_more = result.has_more;

    // Check if we need to add Link header for pagination
    let link_header = if has_more {
        tags.last().map(|last_tag| {
            let n = query.n.unwrap_or(DEFAULT_PAGE_SIZE);
            format!(
                "</v2/{}/tags/list?n={}&last={}>; rel=\"next\"",
                name_str,
                n,
                urlencoding::encode(last_tag)
            )
        })
    } else {
        None
    };

    // Build response body
    let response_body = TagsListResponse {
        name: name_str,
        tags,
    };

    let body = serde_json::to_vec(&response_body)
        .map_err(|e| HttpError::for_internal_error(format!("Failed to serialize tags: {e}")))?;

    // Build response with OCI-compliant headers
    let mut builder = oci_cors_headers(
        Response::builder()
            .status(http::StatusCode::OK)
            .header(http::header::CONTENT_TYPE, "application/json")
            .header(http::header::CONTENT_LENGTH, body.len()),
        &[http::Method::GET],
    );

    // Add Link header for pagination if there are more results
    if let Some(link) = link_header {
        builder = builder.header(http::header::LINK, link);
        builder = builder.header(http::header::ACCESS_CONTROL_EXPOSE_HEADERS, "Link");
    }

    let response = builder
        .body(Body::from(body))
        .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}
