//! OCI Blob Endpoints
//!
//! - HEAD /v2/<name>/blobs/<digest> - Check blob existence
//! - GET /v2/<name>/blobs/<digest> - Download blob
//! - DELETE /v2/<name>/blobs/<digest> - Delete blob

use bencher_endpoint::{CorsResponse, Delete, Endpoint, Get, ResponseDeleted};
use bencher_schema::context::ApiContext;
use dropshot::{Body, HttpError, Path, RequestContext, endpoint};
use http::Response;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::context::storage;
use crate::error::OciError;
use crate::types::{Digest, RepositoryName};

/// Path parameters for blob endpoints
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BlobPath {
    /// Repository name (e.g., "library/ubuntu")
    pub name: String,
    /// Content-addressable digest (e.g., "sha256:abc123...")
    pub digest: String,
}

/// CORS preflight for blob endpoints
#[endpoint {
    method = OPTIONS,
    path = "/v2/{name}/blobs/{digest}",
    tags = ["oci"],
}]
pub async fn oci_blob_options(
    _rqctx: RequestContext<ApiContext>,
    _path: Path<BlobPath>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Delete.into()]))
}

/// Check if a blob exists
///
/// Returns 200 OK with Content-Length header if the blob exists.
/// Returns 404 Not Found if the blob does not exist.
#[endpoint {
    method = HEAD,
    path = "/v2/{name}/blobs/{digest}",
    tags = ["oci"],
}]
pub async fn oci_blob_exists(
    _rqctx: RequestContext<ApiContext>,
    path: Path<BlobPath>,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();

    // Parse and validate inputs
    let repository: RepositoryName = path
        .name
        .parse()
        .map_err(|_err| HttpError::from(OciError::NameInvalid { name: path.name.clone() }))?;
    let digest: Digest = path
        .digest
        .parse()
        .map_err(|_err| HttpError::from(OciError::DigestInvalid { digest: path.digest.clone() }))?;

    // Get storage
    let storage = storage().map_err(|e| HttpError::from(OciError::from(e)))?;

    // Check if blob exists and get size
    let size = storage
        .get_blob_size(&repository, &digest)
        .await
        .map_err(|e| HttpError::from(OciError::from(e)))?;

    // Build response with OCI-compliant headers (no body for HEAD)
    let response = Response::builder()
        .status(http::StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "application/octet-stream")
        .header(http::header::CONTENT_LENGTH, size)
        .header("Docker-Content-Digest", digest.to_string())
        // CORS headers
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(http::header::ACCESS_CONTROL_ALLOW_METHODS, "HEAD, GET")
        .header(http::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type")
        .body(Body::empty())
        .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}

/// Download a blob
///
/// Returns the blob content with appropriate headers.
#[endpoint {
    method = GET,
    path = "/v2/{name}/blobs/{digest}",
    tags = ["oci"],
}]
pub async fn oci_blob_get(
    _rqctx: RequestContext<ApiContext>,
    path: Path<BlobPath>,
) -> Result<Response<Body>, HttpError> {
    let path = path.into_inner();

    // Parse and validate inputs
    let repository: RepositoryName = path
        .name
        .parse()
        .map_err(|_err| HttpError::from(OciError::NameInvalid { name: path.name.clone() }))?;
    let digest: Digest = path
        .digest
        .parse()
        .map_err(|_err| HttpError::from(OciError::DigestInvalid { digest: path.digest.clone() }))?;

    // Get storage
    let storage = storage().map_err(|e| HttpError::from(OciError::from(e)))?;

    // Get blob content
    let (data, size) = storage
        .get_blob(&repository, &digest)
        .await
        .map_err(|e| HttpError::from(OciError::from(e)))?;

    // Record metric
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OciBlobPull);

    // Build response with OCI-compliant headers
    let response = Response::builder()
        .status(http::StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "application/octet-stream")
        .header(http::header::CONTENT_LENGTH, size)
        .header("Docker-Content-Digest", digest.to_string())
        // CORS headers
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(http::header::ACCESS_CONTROL_ALLOW_METHODS, "GET")
        .header(http::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type")
        .body(Body::from(data))
        .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}

/// Delete a blob
///
/// Deletes the blob from the repository.
#[endpoint {
    method = DELETE,
    path = "/v2/{name}/blobs/{digest}",
    tags = ["oci"],
}]
pub async fn oci_blob_delete(
    _rqctx: RequestContext<ApiContext>,
    path: Path<BlobPath>,
) -> Result<ResponseDeleted, HttpError> {
    let path = path.into_inner();

    // Parse and validate inputs
    let repository: RepositoryName = path
        .name
        .parse()
        .map_err(|_err| HttpError::from(OciError::NameInvalid { name: path.name.clone() }))?;
    let digest: Digest = path
        .digest
        .parse()
        .map_err(|_err| HttpError::from(OciError::DigestInvalid { digest: path.digest.clone() }))?;

    // Get storage
    let storage = storage().map_err(|e| HttpError::from(OciError::from(e)))?;

    // Delete the blob
    storage
        .delete_blob(&repository, &digest)
        .await
        .map_err(|e| HttpError::from(OciError::from(e)))?;

    Ok(Delete::auth_response_deleted())
}
