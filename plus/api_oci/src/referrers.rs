//! OCI Referrers API Endpoint
//!
//! - GET /v2/<name>/referrers/<digest> - List referrers to a manifest
//!
//! Returns an image index containing descriptors of manifests that reference
//! the specified digest via their `subject` field.

use bencher_endpoint::{CorsResponse, Endpoint, Get};
use bencher_oci_storage::{Digest, OciError, RepositoryName};
use bencher_schema::context::ApiContext;
use dropshot::{Body, HttpError, Path, Query, RequestContext, endpoint};
use http::Response;
use schemars::JsonSchema;
use serde::Deserialize;

/// Path parameters for referrers endpoint
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReferrersPath {
    /// Repository name (e.g., "library/ubuntu")
    pub name: String,
    /// Digest of the manifest to find referrers for
    pub digest: String,
}

/// Query parameters for referrers endpoint
#[derive(Debug, Deserialize, JsonSchema)]
pub struct ReferrersQuery {
    /// Filter by artifact type
    #[serde(rename = "artifactType")]
    pub artifact_type: Option<String>,
}

/// CORS preflight for referrers endpoint
#[endpoint {
    method = OPTIONS,
    path = "/v2/{name}/referrers/{digest}",
    tags = ["oci"],
}]
pub async fn oci_referrers_options(
    _rqctx: RequestContext<ApiContext>,
    _path: Path<ReferrersPath>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into()]))
}

/// List referrers to a manifest
///
/// Returns an image index containing descriptors of all manifests that
/// reference the specified digest via their `subject` field.
#[endpoint {
    method = GET,
    path = "/v2/{name}/referrers/{digest}",
    tags = ["oci"],
}]
pub async fn oci_referrers_list(
    rqctx: RequestContext<ApiContext>,
    path: Path<ReferrersPath>,
    query: Query<ReferrersQuery>,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();
    let query = query.into_inner();

    // Parse and validate inputs
    let repository: RepositoryName = path
        .name
        .parse()
        .map_err(|_err| crate::error::into_http_error(OciError::NameInvalid { name: path.name.clone() }))?;
    let digest: Digest = path
        .digest
        .parse()
        .map_err(|_err| crate::error::into_http_error(OciError::DigestInvalid { digest: path.digest.clone() }))?;

    // Get storage
    let storage = rqctx.context().oci_storage()?;

    // Get referrers from storage
    let referrers = storage
        .list_referrers(&repository, &digest, query.artifact_type.as_deref())
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Build an OCI image index response
    // Per spec: returns application/vnd.oci.image.index.v1+json
    let index = serde_json::json!({
        "schemaVersion": 2,
        "mediaType": "application/vnd.oci.image.index.v1+json",
        "manifests": referrers
    });

    let body = serde_json::to_vec(&index)
        .map_err(|e| HttpError::for_internal_error(format!("Failed to serialize index: {e}")))?;

    // Build response with OCI-compliant headers
    let response = Response::builder()
        .status(http::StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "application/vnd.oci.image.index.v1+json")
        .header(http::header::CONTENT_LENGTH, body.len())
        // OCI-Filters-Applied header indicates which filters were applied
        .header("OCI-Filters-Applied", if query.artifact_type.is_some() { "artifactType" } else { "" })
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Body::from(body))
        .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}
