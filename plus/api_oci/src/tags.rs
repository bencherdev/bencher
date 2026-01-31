//! OCI Tags Endpoint
//!
//! - GET /v2/<name>/tags/list - List tags for a repository

use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
use bencher_oci_storage::{OciError, RepositoryName};
use bencher_schema::context::ApiContext;
use dropshot::{HttpError, Path, Query, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Path parameters for tags list
#[derive(Debug, Deserialize, JsonSchema)]
pub struct TagsPath {
    /// Repository name (e.g., "library/ubuntu")
    pub name: String,
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
}]
pub async fn oci_tags_options(
    _rqctx: RequestContext<ApiContext>,
    _path: Path<TagsPath>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// List tags for a repository
#[endpoint {
    method = GET,
    path = "/v2/{name}/tags/list",
    tags = ["oci"],
}]
pub async fn oci_tags_list(
    rqctx: RequestContext<ApiContext>,
    path: Path<TagsPath>,
    query: Query<TagsQuery>,
) -> Result<ResponseOk<TagsListResponse>, HttpError> {
    let path = path.into_inner();
    let query = query.into_inner();

    // Parse and validate inputs
    let repository: RepositoryName = path
        .name
        .parse()
        .map_err(|_err| crate::error::into_http_error(OciError::NameInvalid { name: path.name.clone() }))?;

    // Get storage
    let storage = rqctx.context().oci_storage()?;

    // List tags
    let mut tags = storage
        .list_tags(&repository)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Record metric
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OciTagsList);

    // Sort tags for consistent ordering
    tags.sort();

    // Apply pagination
    if let Some(last) = &query.last {
        // Find the position of the last tag and skip to after it
        if let Some(pos) = tags.iter().position(|t| t == last) {
            tags = tags.into_iter().skip(pos + 1).collect();
        }
    }

    // Limit number of results
    if let Some(n) = query.n {
        tags.truncate(n as usize);
    }

    // TODO: Add Link header for pagination if there are more results

    Ok(Get::pub_response_ok(TagsListResponse {
        name: repository.to_string(),
        tags,
    }))
}
