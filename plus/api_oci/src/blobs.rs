//! OCI Blob Endpoints
//!
//! Handles both blob operations and upload start using a unified path structure.
//! The path `/v2/{name}/blobs/{ref}` handles:
//! - GET/HEAD/DELETE when `ref` is a digest (e.g., "sha256:abc...") - blob operations
//! - POST/PUT when `ref` is "uploads" - upload start operations
//!
//! This unified structure avoids Dropshot router conflicts between literal
//! and variable path segments while maintaining OCI spec compliance.

use bencher_endpoint::{CorsResponse, Delete, Endpoint, Get, Post, Put};
use bencher_json::ProjectResourceId;
use bencher_oci_storage::{Digest, OciError};
use bencher_schema::context::ApiContext;
use dropshot::{Body, ClientErrorStatusCode, HttpError, Path, Query, RequestContext, UntypedBody, endpoint};
use http::Response;
use schemars::JsonSchema;
use serde::Deserialize;

use crate::auth::{extract_oci_bearer_token, unauthorized_with_www_authenticate, validate_oci_access};

/// Path parameters for blob/upload-start endpoints
#[derive(Debug, Deserialize, JsonSchema)]
pub struct BlobPath {
    /// Project resource ID (UUID or slug)
    pub name: ProjectResourceId,
    /// Reference - either a digest (e.g., "sha256:abc123...") or "uploads"
    #[serde(rename = "ref")]
    pub reference: String,
}

/// Query parameters for upload start (optional mount)
#[derive(Debug, Deserialize, JsonSchema)]
pub struct UploadStartQuery {
    /// Digest of blob to mount from another repository
    pub digest: Option<String>,
    /// Source repository for cross-repo mount
    pub from: Option<String>,
}

/// Query parameters for monolithic upload
#[derive(Debug, Deserialize, JsonSchema)]
pub struct MonolithicUploadQuery {
    /// Expected digest of the complete blob
    pub digest: String,
}

/// CORS preflight for blob/upload endpoints
#[endpoint {
    method = OPTIONS,
    path = "/v2/{name}/blobs/{ref}",
    tags = ["oci"],
}]
pub async fn oci_blob_options(
    _rqctx: RequestContext<ApiContext>,
    _path: Path<BlobPath>,
) -> Result<CorsResponse, HttpError> {
    Ok(Endpoint::cors(&[Get.into(), Delete.into(), Post.into(), Put.into()]))
}

/// Check if a blob exists (HEAD) or start upload check
#[expect(
    clippy::map_err_ignore,
    reason = "Intentionally discarding auth errors for security"
)]
#[endpoint {
    method = HEAD,
    path = "/v2/{name}/blobs/{ref}",
    tags = ["oci"],
}]
pub async fn oci_blob_exists(
    rqctx: RequestContext<ApiContext>,
    path: Path<BlobPath>,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let path = path.into_inner();

    // "uploads" is not a valid digest - return appropriate error
    if path.reference == "uploads" {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::METHOD_NOT_ALLOWED,
            "HEAD not supported for uploads endpoint".to_owned(),
        ));
    }

    // Authenticate
    let name_str = path.name.to_string();
    let scope = format!("repository:{name_str}:pull");
    let token = extract_oci_bearer_token(&rqctx)
        .map_err(|_| unauthorized_with_www_authenticate(&rqctx, Some(&scope)))?;
    validate_oci_access(context, &token, &name_str, "pull")
        .map_err(|_| unauthorized_with_www_authenticate(&rqctx, Some(&scope)))?;

    // Parse digest
    let digest: Digest = path
        .reference
        .parse()
        .map_err(|_err| crate::error::into_http_error(OciError::DigestInvalid { digest: path.reference.clone() }))?;

    // Get storage
    let storage = context.oci_storage()?;

    // Check if blob exists and get size
    let size = storage
        .get_blob_size(&path.name, &digest)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Build response with OCI-compliant headers (no body for HEAD)
    let response = Response::builder()
        .status(http::StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "application/octet-stream")
        .header(http::header::CONTENT_LENGTH, size)
        .header("Docker-Content-Digest", digest.to_string())
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(http::header::ACCESS_CONTROL_ALLOW_METHODS, "HEAD, GET")
        .header(http::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type")
        .body(Body::empty())
        .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}

/// Download a blob (GET)
#[expect(
    clippy::map_err_ignore,
    reason = "Intentionally discarding auth errors for security"
)]
#[endpoint {
    method = GET,
    path = "/v2/{name}/blobs/{ref}",
    tags = ["oci"],
}]
pub async fn oci_blob_get(
    rqctx: RequestContext<ApiContext>,
    path: Path<BlobPath>,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let path = path.into_inner();

    // "uploads" is not a valid digest - return appropriate error
    if path.reference == "uploads" {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::METHOD_NOT_ALLOWED,
            "GET not supported for uploads endpoint without session ID".to_owned(),
        ));
    }

    // Authenticate
    let name_str = path.name.to_string();
    let scope = format!("repository:{name_str}:pull");
    let token = extract_oci_bearer_token(&rqctx)
        .map_err(|_| unauthorized_with_www_authenticate(&rqctx, Some(&scope)))?;
    validate_oci_access(context, &token, &name_str, "pull")
        .map_err(|_| unauthorized_with_www_authenticate(&rqctx, Some(&scope)))?;

    // Parse digest
    let digest: Digest = path
        .reference
        .parse()
        .map_err(|_err| crate::error::into_http_error(OciError::DigestInvalid { digest: path.reference.clone() }))?;

    // Get storage
    let storage = context.oci_storage()?;

    // Get blob content
    let (data, size) = storage
        .get_blob(&path.name, &digest)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Record metric
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OciBlobPull);

    // Build response with OCI-compliant headers
    let response = Response::builder()
        .status(http::StatusCode::OK)
        .header(http::header::CONTENT_TYPE, "application/octet-stream")
        .header(http::header::CONTENT_LENGTH, size)
        .header("Docker-Content-Digest", digest.to_string())
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .header(http::header::ACCESS_CONTROL_ALLOW_METHODS, "GET")
        .header(http::header::ACCESS_CONTROL_ALLOW_HEADERS, "Content-Type")
        .body(Body::from(data))
        .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}

/// Delete a blob (DELETE)
#[expect(
    clippy::map_err_ignore,
    reason = "Intentionally discarding auth errors for security"
)]
#[endpoint {
    method = DELETE,
    path = "/v2/{name}/blobs/{ref}",
    tags = ["oci"],
}]
pub async fn oci_blob_delete(
    rqctx: RequestContext<ApiContext>,
    path: Path<BlobPath>,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let path = path.into_inner();

    // "uploads" is not a valid digest - return appropriate error
    if path.reference == "uploads" {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::METHOD_NOT_ALLOWED,
            "DELETE not supported for uploads endpoint without session ID".to_owned(),
        ));
    }

    // Authenticate (delete requires push permission)
    let name_str = path.name.to_string();
    let scope = format!("repository:{name_str}:push");
    let token = extract_oci_bearer_token(&rqctx)
        .map_err(|_| unauthorized_with_www_authenticate(&rqctx, Some(&scope)))?;
    validate_oci_access(context, &token, &name_str, "push")
        .map_err(|_| unauthorized_with_www_authenticate(&rqctx, Some(&scope)))?;

    // Parse digest
    let digest: Digest = path
        .reference
        .parse()
        .map_err(|_err| crate::error::into_http_error(OciError::DigestInvalid { digest: path.reference.clone() }))?;

    // Get storage
    let storage = context.oci_storage()?;

    // Delete the blob
    storage
        .delete_blob(&path.name, &digest)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // OCI spec requires 202 Accepted for DELETE
    let response = Response::builder()
        .status(http::StatusCode::ACCEPTED)
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Body::empty())
        .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}

/// Start a new blob upload (POST to /v2/{name}/blobs/uploads)
#[expect(
    clippy::map_err_ignore,
    reason = "Intentionally discarding auth errors for security"
)]
#[endpoint {
    method = POST,
    path = "/v2/{name}/blobs/{ref}",
    tags = ["oci"],
}]
pub async fn oci_upload_start(
    rqctx: RequestContext<ApiContext>,
    path: Path<BlobPath>,
    query: Query<UploadStartQuery>,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let path = path.into_inner();
    let query = query.into_inner();

    // POST is only valid when ref is "uploads"
    if path.reference != "uploads" {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::METHOD_NOT_ALLOWED,
            "POST only supported for uploads endpoint".to_owned(),
        ));
    }

    // Authenticate
    let name_str = path.name.to_string();
    let scope = format!("repository:{name_str}:push");
    let token = extract_oci_bearer_token(&rqctx)
        .map_err(|_| unauthorized_with_www_authenticate(&rqctx, Some(&scope)))?;
    validate_oci_access(context, &token, &name_str, "push")
        .map_err(|_| unauthorized_with_www_authenticate(&rqctx, Some(&scope)))?;

    // Get storage
    let storage = context.oci_storage()?;

    // Handle cross-repository mount if requested
    if let (Some(digest_str), Some(from_name)) = (&query.digest, &query.from) {
        let digest: Digest = digest_str
            .parse()
            .map_err(|_err| crate::error::into_http_error(OciError::DigestInvalid { digest: digest_str.clone() }))?;
        let from_repo: ProjectResourceId = from_name
            .parse()
            .map_err(|_err| crate::error::into_http_error(OciError::NameInvalid { name: from_name.clone() }))?;

        // Try to mount the blob
        let mounted = storage
            .mount_blob(&from_repo, &path.name, &digest)
            .await
            .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

        if mounted {
            // Mount successful - return 201 Created
            let location = format!("/v2/{}/blobs/{digest}", path.name);
            let response = Response::builder()
                .status(http::StatusCode::CREATED)
                .header(http::header::LOCATION, location)
                .header("Docker-Content-Digest", digest.to_string())
                .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
                .body(Body::empty())
                .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;
            return Ok(response);
        }
    }

    // Start a new upload session
    let upload_id = storage
        .start_upload(&path.name)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Build 202 Accepted response
    let location = format!("/v2/{}/blobs/uploads/{upload_id}", path.name);
    let response = Response::builder()
        .status(http::StatusCode::ACCEPTED)
        .header(http::header::LOCATION, location)
        .header("Range", "0-0")
        .header("Docker-Upload-UUID", upload_id.to_string())
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Body::empty())
        .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}

/// Monolithic upload (PUT to /v2/{name}/blobs/uploads?digest=...)
#[expect(
    clippy::map_err_ignore,
    reason = "Intentionally discarding auth errors for security"
)]
#[endpoint {
    method = PUT,
    path = "/v2/{name}/blobs/{ref}",
    tags = ["oci"],
}]
pub async fn oci_upload_monolithic(
    rqctx: RequestContext<ApiContext>,
    path: Path<BlobPath>,
    query: Query<MonolithicUploadQuery>,
    body: UntypedBody,
) -> Result<Response<Body>, HttpError> {
    let context = rqctx.context();
    let path = path.into_inner();
    let query = query.into_inner();
    let data = body.as_bytes();

    // PUT with digest query param is only valid when ref is "uploads"
    if path.reference != "uploads" {
        return Err(HttpError::for_client_error(
            None,
            ClientErrorStatusCode::METHOD_NOT_ALLOWED,
            "PUT only supported for uploads endpoint".to_owned(),
        ));
    }

    // Authenticate
    let name_str = path.name.to_string();
    let scope = format!("repository:{name_str}:push");
    let token = extract_oci_bearer_token(&rqctx)
        .map_err(|_| unauthorized_with_www_authenticate(&rqctx, Some(&scope)))?;
    validate_oci_access(context, &token, &name_str, "push")
        .map_err(|_| unauthorized_with_www_authenticate(&rqctx, Some(&scope)))?;

    // Parse digest
    let expected_digest: Digest = query
        .digest
        .parse()
        .map_err(|_err| crate::error::into_http_error(OciError::DigestInvalid { digest: query.digest.clone() }))?;

    // Get storage
    let storage = context.oci_storage()?;

    // Start upload, append data, and complete in one operation
    let upload_id = storage
        .start_upload(&path.name)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    storage
        .append_upload(&upload_id, bytes::Bytes::copy_from_slice(data))
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    let actual_digest = storage
        .complete_upload(&upload_id, &expected_digest)
        .await
        .map_err(|e| crate::error::into_http_error(OciError::from(e)))?;

    // Record metric
    #[cfg(feature = "otel")]
    bencher_otel::ApiMeter::increment(bencher_otel::ApiCounter::OciBlobPush);

    // Build 201 Created response
    let location = format!("/v2/{}/blobs/{actual_digest}", path.name);
    let response = Response::builder()
        .status(http::StatusCode::CREATED)
        .header(http::header::LOCATION, location)
        .header("Docker-Content-Digest", actual_digest.to_string())
        .header(http::header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
        .body(Body::empty())
        .map_err(|e| HttpError::for_internal_error(format!("Failed to build response: {e}")))?;

    Ok(response)
}
