//! OCI Tags Endpoint
//!
//! - GET /v2/<name>/tags/list - List tags for a repository

use bencher_endpoint::{CorsResponse, Endpoint, Get, ResponseOk};
use bencher_json::ProjectResourceId;
use bencher_oci_storage::OciError;
use bencher_schema::context::ApiContext;
use dropshot::{HttpError, Path, Query, RequestContext, endpoint};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[cfg(feature = "plus")]
use crate::auth::apply_auth_rate_limit;
use crate::auth::{
    extract_oci_bearer_token, unauthorized_with_www_authenticate, validate_oci_access,
};

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
}]
pub async fn oci_tags_options(
    _rqctx: RequestContext<ApiContext>,
    _path: Path<TagsPath>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// List tags for a repository
#[expect(
    clippy::map_err_ignore,
    reason = "Intentionally discarding auth errors for security"
)]
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
    let context = rqctx.context();
    let path = path.into_inner();
    let query = query.into_inner();

    // Authenticate
    let name_str = path.name.to_string();
    let scope = format!("repository:{name_str}:pull");
    let token = extract_oci_bearer_token(&rqctx)
        .map_err(|_| unauthorized_with_www_authenticate(&rqctx, Some(&scope)))?;
    let claims = validate_oci_access(context, &token, &name_str, "pull")
        .map_err(|_| unauthorized_with_www_authenticate(&rqctx, Some(&scope)))?;

    // Apply rate limiting
    #[cfg(feature = "plus")]
    apply_auth_rate_limit(&rqctx.log, context, &claims).await?;

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
        name: name_str,
        tags,
    }))
}
