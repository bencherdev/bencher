//! OCI Tags Endpoint
//!
//! - GET /v2/<name>/tags/list - List tags for a repository

use bencher_endpoint::{CorsResponse, Endpoint, Get};
use bencher_json::ProjectResourceId;
use bencher_oci_storage::OciError;
use bencher_schema::context::ApiContext;
use dropshot::{
    HttpError, HttpResponseHeaders, HttpResponseOk, Path, Query, RequestContext, endpoint,
};
use http::header::HeaderValue;
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
    /// Number of tags to return (default: 100)
    pub n: Option<u32>,
    /// Last tag from previous response (for pagination)
    pub last: Option<String>,
}

/// Headers for OCI tags list response
#[derive(Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub struct OciTagsHeaders {
    /// CORS: Allow all origins
    pub access_control_allow_origin: String,
    /// CORS: Expose headers to client (includes Link when pagination is present)
    pub access_control_expose_headers: String,
}

impl OciTagsHeaders {
    pub fn new(expose_link: bool) -> Self {
        let expose = if expose_link { "Link" } else { "" };
        Self {
            access_control_allow_origin: "*".to_owned(),
            access_control_expose_headers: expose.to_owned(),
        }
    }
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
) -> Result<HttpResponseHeaders<HttpResponseOk<TagsListResponse>, OciTagsHeaders>, HttpError> {
    let context = rqctx.context();
    let path = path.into_inner();
    let query = query.into_inner();

    // Authenticate and apply rate limiting
    let name_str = path.name.to_string();
    let _access = require_pull_access(&rqctx, &name_str).await?;

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
        .list_tags(&path.name, Some(page_size), last_tag)
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

    // Build response with OCI-compliant headers
    let mut response = HttpResponseHeaders::new(
        HttpResponseOk(response_body),
        OciTagsHeaders::new(link_header.is_some()),
    );

    // Add Link header for pagination if there are more results
    if let Some(link) = link_header {
        response.headers_mut().insert(
            http::header::LINK,
            HeaderValue::from_str(&link)
                .map_err(|e| HttpError::for_internal_error(format!("Invalid Link header: {e}")))?,
        );
    }

    Ok(response)
}
